use anyhow::{anyhow, Context, Result};
use crossbeam_channel::bounded;
use std::path::PathBuf;

#[derive(Clone)]
pub enum AudioInputKind { Mic { device: Option<String> }, Wav(PathBuf) }

pub fn run_input_pipeline<F: FnMut(&[f32]) + Send + 'static>(kind: AudioInputKind, target_sr: u32, realtime: bool, on_block: F) -> Result<()> {
    match kind {
        AudioInputKind::Wav(path) => run_wav(path, target_sr, realtime, on_block),
        AudioInputKind::Mic { device } => run_mic(target_sr, device, on_block),
    }
}

fn run_wav<F: FnMut(&[f32]) + Send + 'static>(path: PathBuf, target_sr: u32, realtime: bool, mut on_block: F) -> Result<()> {
    let mut reader = hound::WavReader::open(&path).with_context(|| format!("Opening {}", path.display()))?;
    let spec = reader.spec();
    let src_sr = spec.sample_rate as f32;
    let dst_sr = target_sr as f32;
    let channels = spec.channels.max(1) as usize;
    let ratio = dst_sr / src_sr;

    // Streaming downmix + linear resampler state
    let mut sum = 0.0f32;
    let mut cnt = 0usize;
    let mut src_buf: Vec<f32> = Vec::with_capacity(8192);
    let mut src_pos = 0.0f32; // fractional index into src_buf
    let mut out_buf: Vec<f32> = Vec::with_capacity(8192);
    let block = 1024usize; // smaller block for lower latency

    let start = std::time::Instant::now();
    let mut emitted_samples: usize = 0;
    match spec.sample_format {
        hound::SampleFormat::Float => {
            for s in reader.samples::<f32>() {
                let v = s?;
                sum += v; cnt += 1;
                if cnt == channels { src_buf.push(sum / (channels as f32)); sum = 0.0; cnt = 0; }

                // Resample when enough source is buffered
                resample_drain(ratio, &mut src_buf, &mut src_pos, &mut out_buf);
                while out_buf.len() >= block {
                    let chunk = &out_buf[..block];
                    on_block(chunk);
                    if realtime { throttle_realtime(chunk.len(), target_sr, start, &mut emitted_samples); }
                    out_buf.drain(0..block);
                }
            }
        }
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            for s in reader.samples::<i32>() {
                let v = s? as f32 / max;
                sum += v; cnt += 1;
                if cnt == channels { src_buf.push(sum / (channels as f32)); sum = 0.0; cnt = 0; }

                resample_drain(ratio, &mut src_buf, &mut src_pos, &mut out_buf);
                while out_buf.len() >= block {
                    let chunk = &out_buf[..block];
                    on_block(chunk);
                    if realtime { throttle_realtime(chunk.len(), target_sr, start, &mut emitted_samples); }
                    out_buf.drain(0..block);
                }
            }
        }
    }
    // Flush remaining
    resample_drain(ratio, &mut src_buf, &mut src_pos, &mut out_buf);
    while !out_buf.is_empty() {
        let n = out_buf.len().min(block);
        let chunk = &out_buf[..n];
        on_block(chunk);
        if realtime { throttle_realtime(chunk.len(), target_sr, start, &mut emitted_samples); }
        out_buf.drain(0..n);
    }
    Ok(())
}

fn resample_drain(ratio: f32, src_buf: &mut Vec<f32>, src_pos: &mut f32, out_buf: &mut Vec<f32>) {
    if src_buf.len() < 2 { return; }
    while *src_pos + 1.0 < src_buf.len() as f32 {
        let i0 = (*src_pos).floor() as usize;
        let frac = *src_pos - (i0 as f32);
        let y = src_buf[i0] * (1.0 - frac) + src_buf[i0 + 1] * frac;
        out_buf.push(y);
        *src_pos += ratio;
    }
    // Drop consumed samples to avoid unbounded growth, keep one sample for interpolation
    let consumed = (*src_pos).floor() as usize;
    if consumed > 0 && consumed < src_buf.len() {
        src_buf.drain(0..consumed);
        *src_pos -= consumed as f32;
    }
}

fn throttle_realtime(emitted_now: usize, sr: u32, start: std::time::Instant, emitted_total: &mut usize) {
    *emitted_total += emitted_now;
    let target = std::time::Duration::from_secs_f32((*emitted_total as f32) / (sr as f32));
    let elapsed = start.elapsed();
    if target > elapsed {
        let sleep_dur = target - elapsed;
        // Cap sleep to avoid long stalls in case of hiccups
        let cap = std::time::Duration::from_millis(50);
        std::thread::sleep(sleep_dur.min(cap));
    }
}

#[cfg(feature = "mic")]
fn run_mic<F: FnMut(&[f32]) + Send + 'static>(_target_sr: u32, device_name: Option<String>, mut on_block: F) -> Result<()> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let host = cpal::default_host();
    let device = if let Some(name) = device_name {
        let mut found = None;
        if let Ok(devices) = host.input_devices() {
            for d in devices {
                if let Ok(n) = d.name() { if n.contains(&name) { found = Some(d); break; } }
            }
        }
        found.ok_or_else(|| anyhow!("Input device '{name}' not found"))?
    } else {
        host.default_input_device().ok_or_else(|| anyhow!("No default input device"))?
    };

    let default_config = device.default_input_config().context("default input config")?;
    let sample_format = default_config.sample_format();
    let config: cpal::StreamConfig = default_config.config();

    let (tx, rx) = bounded::<Vec<f32>>(64);
    let channels = config.channels as usize;
    let err_fn = |err| eprintln!("Stream error: {err}");

    let stream = match sample_format {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config,
            move |data: &[f32], _| {
                let mut mono = Vec::with_capacity(data.len() / channels);
                for frame in data.chunks_exact(channels) {
                    mono.push(frame.iter().copied().sum::<f32>() / (channels as f32));
                }
                let _ = tx.send(mono);
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config,
            move |data: &[i16], _| {
                let mut mono = Vec::with_capacity(data.len() / channels);
                for frame in data.chunks_exact(channels) {
                    let sum: i32 = frame.iter().map(|&v| v as i32).sum();
                    mono.push((sum as f32) / (channels as f32) / (i16::MAX as f32));
                }
                let _ = tx.send(mono);
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::U16 => device.build_input_stream(
            &config,
            move |data: &[u16], _| {
                let mut mono = Vec::with_capacity(data.len() / channels);
                for frame in data.chunks_exact(channels) {
                    let sum: u32 = frame.iter().map(|&v| v as u32).sum();
                    // Center around 0
                    let avg = (sum as f32) / (channels as f32);
                    let centered = (avg - (u16::MAX as f32) / 2.0) / ((u16::MAX as f32) / 2.0);
                    mono.push(centered);
                }
                let _ = tx.send(mono);
            },
            err_fn,
            None,
        )?,
        _ => return Err(anyhow!("Unsupported sample format")),
    };

    stream.play()?;
    while let Ok(buf) = rx.recv() { on_block(&buf); }
    Ok(())
}

#[cfg(not(feature = "mic"))]
fn run_mic<F: FnMut(&[f32]) + Send + 'static>(_target_sr: u32, _device_name: Option<String>, _on_block: F) -> Result<()> {
    Err(anyhow!("Binary built without 'mic' feature"))
}
