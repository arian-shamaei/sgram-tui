use crate::colors::Palette;
use crate::app::{AnimationStyle, RenderMode, FreqScale};
use anyhow::Result;
use image::{ImageBuffer, Rgb};
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;

pub fn save_png(
    buffer: &VecDeque<Vec<f32>>,
    palette: &Palette,
    db_floor: f32,
    db_ceiling: f32,
    width: u32,
    height: u32,
    style: AnimationStyle,
    render: RenderMode,
    freq_scale: FreqScale,
    sample_rate: u32,
    zoom: f32,
    overview: bool,
    path: PathBuf,
) -> Result<()> {
    if buffer.is_empty() { return Ok(()); }
    let w = width.max(1);
    let h = height.max(1);
    let mut img = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(w, h);
    let bins = buffer.front().map(|r| r.len()).unwrap_or(1).max(1);
    let total = buffer.len();

    // Helpers to map frequency fraction to bin index using the selected scale
    let map_frac_to_freq = |t: f32| -> f32 {
        let fs = sample_rate as f32;
        let fmax = fs / 2.0 / zoom.max(1.0);
        let fmin = match freq_scale { FreqScale::Linear => 0.0, _ => 20.0 };
        match freq_scale {
            FreqScale::Linear => t * fmax,
            FreqScale::Log => {
                let a = (fmax / fmin).max(1.01);
                fmin * a.powf(t)
            }
            FreqScale::Mel => {
                let mel = |hz: f32| 2595.0 * (1.0 + hz / 700.0).log10();
                let inv_mel = |m: f32| 700.0 * (10f32.powf(m / 2595.0) - 1.0);
                let mmin = mel(fmin); let mmax = mel(fmax);
                inv_mel(mmin + t * (mmax - mmin))
            }
        }
    };
    let map_t_to_bin = |t: f32| -> usize {
        let fs = sample_rate as f32;
        let fmax = fs / 2.0 / zoom.max(1.0);
        let f = map_frac_to_freq(t);
        let hz_per_bin = fmax / (bins as f32);
        let idx = if hz_per_bin > 0.0 { (f / hz_per_bin).floor() as usize } else { 0 };
        idx.min(bins.saturating_sub(1))
    };

    match style {
        AnimationStyle::Waterfall => {
            // y = time (newest at top), x = frequency (low->high)
            for y in 0..h {
                let src = if overview {
                    let frac = 1.0 - ((y as f32) + 0.5) / (h as f32);
                    let idx = ((total as f32 - 1.0) * frac).round() as usize;
                    &buffer[total - 1 - idx.min(total - 1)]
                } else {
                    let idx = y as usize;
                    buffer.get(idx).unwrap_or_else(|| buffer.back().unwrap())
                };
                for x in 0..w {
                    let t = (x as f32) / (w as f32);
                    let bin_idx = map_t_to_bin(t);
                    let db = *src.get(bin_idx).unwrap_or(&db_floor);
                    let tcol = ((db - db_floor) / (db_ceiling - db_floor).max(1.0)).clamp(0.0, 1.0);
                    let (r, g, b) = match palette.color_at(tcol) { ratatui::style::Color::Rgb(r,g,b) => (r,g,b), _ => (0,0,0) };
                    img.put_pixel(x, y, Rgb([r, g, b]));
                }
            }
        }
        AnimationStyle::Horizontal => {
            // x = time (oldest->newest), y = frequency (low at bottom)
            for y in 0..h {
                let tfrac = 1.0 - ((y as f32) + 0.5) / (h as f32);
                let bin_idx = map_t_to_bin(tfrac);
                for x in 0..w {
                    let t = (x as f32) / (w as f32);
                    // Map full history horizontally
                    let tidx = ((total as f32 - 1.0) * t).round() as usize;
                    if let Some(row) = buffer.get(total - 1 - tidx.min(total - 1)) {
                        let db = *row.get(bin_idx).unwrap_or(&db_floor);
                        let tcol = ((db - db_floor) / (db_ceiling - db_floor).max(1.0)).clamp(0.0, 1.0);
                        let (r, g, b) = match palette.color_at(tcol) { ratatui::style::Color::Rgb(r,g,b) => (r,g,b), _ => (0,0,0) };
                        img.put_pixel(x, y, Rgb([r, g, b]));
                    }
                }
            }
        }
    }
    if let Some(parent) = path.parent() { if !parent.as_os_str().is_empty() { let _ = fs::create_dir_all(parent); } }
    img.save(path)?;
    Ok(())
}

pub fn save_csv(buffer: &VecDeque<Vec<f32>>, path: PathBuf) -> Result<()> {
    if let Some(parent) = path.parent() { if !parent.as_os_str().is_empty() { let _ = fs::create_dir_all(parent); } }
    let mut wtr = csv::Writer::from_path(path)?;
    for row in buffer.iter().rev() { // oldest to newest
        wtr.write_record(row.iter().map(|v| format!("{:.6}", v)))?;
    }
    wtr.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("sgram_tui_test_{}_{}.tmp", name, std::process::id()));
        p
    }

    #[test]
    fn csv_writes_rows_oldest_first() {
        let mut buf: VecDeque<Vec<f32>> = VecDeque::new();
        // push newest first (front), oldest last (back)
        buf.push_front(vec![-20.0, -30.0]);
        buf.push_front(vec![0.0, -10.0]);
        let path = tmp_path("csv");
        save_csv(&buf, path.clone()).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.trim().split('\n').collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "-20.000000,-30.000000");
        assert_eq!(lines[1], "0.000000,-10.000000");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn png_creates_nonempty_file() {
        let mut buf: VecDeque<Vec<f32>> = VecDeque::new();
        buf.push_front(vec![-80.0, 0.0]);
        let path = tmp_path("png").with_extension("png");
        let palette = Palette::viridis();
        save_png(&buf, &palette, -80.0, 0.0, 64, 32, AnimationStyle::Waterfall, RenderMode::Cell, FreqScale::Linear, 48000, 1.0, true, path.clone()).unwrap();
        let meta = std::fs::metadata(&path).unwrap();
        assert!(meta.len() > 0);
        let _ = std::fs::remove_file(path);
    }
}
