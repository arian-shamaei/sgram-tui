use crate::colors::Palette;
use crate::dsp::{SpectrogramBuilder, WindowType};
use crate::export;
use crate::input::{self, AudioInputKind};
use anyhow::{anyhow, Result};
use crossbeam_channel::{bounded, Receiver};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Copy, Clone, Debug)]
pub enum ColorPalette {
    Grayscale,
    Heat,
    Viridis,
    Jet,
    Inferno,
    Magma,
    Plasma,
    PurpleFire,
}

impl ColorPalette {
    pub fn palette(&self) -> Palette {
        match self {
            ColorPalette::Grayscale => Palette::grayscale(),
            ColorPalette::Heat => Palette::heat(),
            ColorPalette::Viridis => Palette::viridis(),
            ColorPalette::Jet => Palette::jet(),
            ColorPalette::Inferno => Palette::inferno(),
            ColorPalette::Magma => Palette::magma(),
            ColorPalette::Plasma => Palette::plasma(),
            ColorPalette::PurpleFire => Palette::purple_fire(),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum AnimationStyle {
    Horizontal,
    Waterfall,
}

#[derive(Copy, Clone, Debug)]
pub enum FreqScale {
    Linear,
    Log,
    Mel,
}

#[derive(Copy, Clone, Debug)]
pub struct Settings {
    pub fft_size: usize,
    pub hop_size: usize,
    pub window_len: usize,
    pub sample_rate: u32,
    pub db_floor: f32,
    pub db_ceiling: f32,
    pub fps: u64,
    pub zoom: f32,
    pub palette: ColorPalette,
    pub style: AnimationStyle,
    pub detailed: bool,
    pub fullscreen: bool,
    pub history: usize,
    pub render_mode: RenderMode,
    pub freq_scale: FreqScale,
    pub alpha: u8,
    pub pre_emphasis: Option<f32>,
    pub overview: bool,
    pub realtime: bool,
    pub clamp_floor: bool,
    pub normalize: bool,
}

pub struct App {
    pub settings: Settings,
    pub running: bool,
    pub paused: bool,
    pub last_tick: Instant,
    pub palette: Palette,
    pub style: AnimationStyle,
    pub zoom: f32,
    pub db_floor: f32,
    pub db_ceiling: f32,
    pub buffer: VecDeque<Vec<f32>>, // normalized 0..1 rows (bins)
    pub max_history: usize,
    pub spectrogram_rx: Receiver<Vec<f32>>,
    pub input_kind: AudioInputKind,
    pub input_desc: String,
    pub detailed: bool,
    pub fullscreen: bool,
    pub export_png_path: Option<PathBuf>,
    pub export_csv_path: Option<PathBuf>,
    pub render_mode: RenderMode,
    pub history: usize,
    pub show_help: bool,
    pub freq_scale: FreqScale,
    pub overview: bool,
    pub realtime: bool,
    pub stats_rows_sec: f32,
    pub stats_rows_count: usize,
    pub stats_last_instant: Instant,
    pub total_rows: usize,
}

impl App {
    pub fn new(
        input: String,
        settings: Settings,
        no_mic: bool,
        mic_device: Option<String>,
    ) -> Result<Self> {
        let input_kind = if input.to_lowercase() == "mic" {
            if cfg!(feature = "mic") && !no_mic {
                AudioInputKind::Mic { device: mic_device }
            } else {
                return Err(anyhow!("Mic feature not enabled at compile time. Rebuild with --features mic or provide a WAV file."));
            }
        } else {
            AudioInputKind::Wav(PathBuf::from(input))
        };

        let (spectrogram_tx, spectrogram_rx) = bounded::<Vec<f32>>(64);

        // Start input + DSP thread
        let sr = settings.sample_rate;
        let fft_size = settings.fft_size;
        let frame_len = settings.window_len.min(fft_size).max(16);
        let hop = settings.hop_size.min(frame_len).max(1);
        let floor = settings.db_floor;
        let alpha = settings.alpha;
        let pre_emph = settings.pre_emphasis;
        let input_desc = match &input_kind {
            AudioInputKind::Mic { device } => match &device {
                Some(d) => format!("Microphone: {d}"),
                None => "Microphone (default)".to_string(),
            },
            AudioInputKind::Wav(p) => format!("WAV: {}", p.display()),
        };

        let thread_kind = input_kind.clone();
        std::thread::spawn(move || {
            let mut spec = SpectrogramBuilder::new(fft_size, frame_len, hop)
                .window(WindowType::Hann)
                .db_floor(floor)
                .sample_rate(sr)
                .alpha(alpha)
                .pre_emphasis(pre_emph)
                .clamp_floor(settings.clamp_floor)
                .normalize(settings.normalize)
                .build();
            if let Err(e) =
                input::run_input_pipeline(thread_kind, sr, settings.realtime, move |samples| {
                    let rows = spec.process_samples(samples);
                    for row in rows {
                        let _ = spectrogram_tx.send(row);
                    }
                })
            {
                eprintln!("Input pipeline error: {e}");
            }
        });

        Ok(Self {
            settings,
            running: true,
            paused: false,
            last_tick: Instant::now(),
            palette: settings.palette.palette(),
            style: settings.style,
            zoom: settings.zoom,
            db_floor: settings.db_floor,
            db_ceiling: settings.db_ceiling,
            buffer: VecDeque::new(),
            max_history: settings.history.max(16),
            spectrogram_rx,
            input_kind,
            input_desc,
            detailed: settings.detailed,
            fullscreen: settings.fullscreen,
            export_png_path: None,
            export_csv_path: None,
            render_mode: settings.render_mode,
            history: settings.history.max(16),
            show_help: false,
            freq_scale: settings.freq_scale,
            overview: settings.overview,
            realtime: settings.realtime,
            stats_rows_sec: 0.0,
            stats_rows_count: 0,
            stats_last_instant: Instant::now(),
            total_rows: 0,
        })
    }

    pub fn tick_rate(&self) -> Duration {
        Duration::from_millis((1000 / self.settings.fps.max(1)) as u64)
    }

    pub fn push_row(&mut self, mut row: Vec<f32>, bins: usize) {
        // Apply zoom: take lower frequency bins proportionally (in dB domain)
        let take = ((bins as f32) / self.zoom).round() as usize;
        row.truncate(take.max(1));
        self.buffer.push_front(row);
        while self.buffer.len() > self.max_history {
            self.buffer.pop_back();
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    pub fn toggle_style(&mut self) {
        self.style = match self.style {
            AnimationStyle::Horizontal => AnimationStyle::Waterfall,
            AnimationStyle::Waterfall => AnimationStyle::Horizontal,
        };
    }

    pub fn next_palette(&mut self) {
        self.palette = self.palette.next();
    }

    pub fn prev_palette(&mut self) {
        self.palette = self.palette.prev();
    }

    pub fn adjust_zoom(&mut self, delta: f32) {
        self.zoom = (self.zoom + delta).clamp(1.0, 64.0);
    }

    pub fn adjust_floor(&mut self, delta: f32) {
        self.db_floor = (self.db_floor + delta).clamp(-140.0, -10.0);
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    pub fn save_png(&self, path: PathBuf, width: u32, height: u32) -> Result<()> {
        export::save_png(
            &self.buffer,
            &self.palette,
            self.db_floor,
            self.db_ceiling,
            width,
            height,
            self.style,
            self.render_mode,
            self.freq_scale,
            self.settings.sample_rate,
            self.zoom,
            self.overview,
            path,
        )
    }

    pub fn save_csv(&self, path: PathBuf) -> Result<()> {
        export::save_csv(&self.buffer, path)
    }
}

#[derive(Copy, Clone, Debug)]
pub enum RenderMode {
    Cell,
    Half,
}
