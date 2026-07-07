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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AnimationStyle {
    Horizontal,
    Waterfall,
    Spectrum,
}

#[derive(Copy, Clone, Debug)]
pub enum FreqScale {
    Linear,
    Log,
    Mel,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BinsMode {
    All,
    Peaks,
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
    pub window: WindowType,
    pub bins_mode: BinsMode,
}

pub struct App {
    pub settings: Settings,
    pub running: bool,
    pub paused: bool,
    pub palette: Palette,
    pub style: AnimationStyle,
    pub zoom: f32,
    pub db_floor: f32,
    pub db_ceiling: f32,
    pub buffer: VecDeque<Vec<f32>>, // normalized 0..1 rows (bins)
    pub max_history: usize,
    pub spectrogram_rx: Receiver<Vec<f32>>,
    pub input_desc: String,
    pub detailed: bool,
    pub fullscreen: bool,
    pub export_png_path: Option<PathBuf>,
    pub export_csv_path: Option<PathBuf>,
    pub render_mode: RenderMode,
    pub show_help: bool,
    pub freq_scale: FreqScale,
    pub overview: bool,
    pub realtime: bool,
    pub stats_rows_sec: f32,
    pub stats_rows_count: usize,
    pub stats_last_instant: Instant,
    pub total_rows: usize,
    pub status_msg: Option<(String, Instant)>,
    pub bins_mode: BinsMode,
    pub hover: Option<(u16, u16)>,
    pub hover_at: Instant,
    pub pipeline_error: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    /// Sticky input error shown in the status bar (unlike status_msg, no expiry)
    pub error: Option<String>,
}

impl App {
    pub fn new(
        input: String,
        settings: Settings,
        no_mic: bool,
        mic_device: Option<String>,
    ) -> Result<Self> {
        // Normalize analysis parameters once so every consumer (DSP, status
        // bar, hover readout, PNG time axis, render summaries) agrees on the
        // effective values, not the raw CLI ones.
        let mut settings = settings;
        settings.fft_size = settings.fft_size.max(16);
        settings.window_len = settings.window_len.min(settings.fft_size).max(16);
        settings.hop_size = settings.hop_size.min(settings.window_len).max(1);

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
        let frame_len = settings.window_len;
        let hop = settings.hop_size;
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

        let pipeline_error = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
        let thread_error = pipeline_error.clone();
        let thread_kind = input_kind.clone();
        std::thread::spawn(move || {
            let mut spec = SpectrogramBuilder::new(fft_size, frame_len, hop)
                .window(settings.window)
                .db_floor(floor)
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
                // Surfaced by the UI (or the headless render path); eprintln
                // alone would be swallowed by the alternate screen.
                *thread_error.lock().unwrap() = Some(e.to_string());
            }
        });

        Ok(Self {
            settings,
            running: true,
            paused: false,
            palette: settings.palette.palette(),
            style: settings.style,
            zoom: settings.zoom,
            db_floor: settings.db_floor,
            db_ceiling: settings.db_ceiling,
            buffer: VecDeque::new(),
            max_history: settings.history.max(16),
            spectrogram_rx,
            input_desc,
            detailed: settings.detailed,
            fullscreen: settings.fullscreen,
            export_png_path: None,
            export_csv_path: None,
            render_mode: settings.render_mode,
            show_help: false,
            freq_scale: settings.freq_scale,
            overview: settings.overview,
            realtime: settings.realtime,
            stats_rows_sec: 0.0,
            stats_rows_count: 0,
            stats_last_instant: Instant::now(),
            total_rows: 0,
            status_msg: None,
            bins_mode: settings.bins_mode,
            hover: None,
            hover_at: Instant::now(),
            pipeline_error,
            error: None,
        })
    }

    pub fn tick_rate(&self) -> Duration {
        Duration::from_millis(1000 / self.settings.fps.max(1))
    }

    pub fn push_row(&mut self, row: Vec<f32>) {
        // Store full-resolution rows; zoom is applied at render time so it is
        // reversible and history stays uniform when zoom changes mid-run.
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
            AnimationStyle::Waterfall => AnimationStyle::Spectrum,
            AnimationStyle::Spectrum => AnimationStyle::Horizontal,
        };
    }

    pub fn toggle_bins_mode(&mut self) {
        self.bins_mode = match self.bins_mode {
            BinsMode::All => BinsMode::Peaks,
            BinsMode::Peaks => BinsMode::All,
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

    /// Hover position, if the mouse moved recently (readouts fade after 3s).
    pub fn active_hover(&self) -> Option<(u16, u16)> {
        if self.hover_at.elapsed() < Duration::from_secs(3) {
            self.hover
        } else {
            None
        }
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_msg = Some((msg.into(), Instant::now()));
    }

    pub fn current_status(&self) -> Option<&str> {
        match &self.status_msg {
            Some((msg, at)) if at.elapsed() < Duration::from_secs(4) => Some(msg.as_str()),
            _ => None,
        }
    }

    /// Native export size: one pixel per (zoomed) frequency bin and history
    /// row, doubled for crispness. The exporter adds axis margins on top.
    pub fn png_content_dims(&self) -> (u32, u32) {
        const MAX_DIM: u32 = 4096;
        const MIN_DIM: u32 = 480;
        let full_bins = self.buffer.front().map(|r| r.len()).unwrap_or(1).max(1) as f32;
        let bins = ((full_bins / self.zoom.max(1.0)).round().max(1.0) as u32) * 2;
        let rows = (self.buffer.len().max(1) as u32) * 2;
        let (w, h) = match self.export_style() {
            AnimationStyle::Horizontal => (rows, bins),
            _ => (bins, rows),
        };
        (w.clamp(MIN_DIM, MAX_DIM), h.clamp(MIN_DIM, MAX_DIM))
    }

    /// Spectrum view has no 2D export; fall back to waterfall.
    fn export_style(&self) -> AnimationStyle {
        if self.style == AnimationStyle::Spectrum { AnimationStyle::Waterfall } else { self.style }
    }

    pub fn save_png(&self, path: PathBuf, width: u32, height: u32) -> Result<()> {
        let s = &self.settings;
        export::save_png(
            &export::PngRequest {
                buffer: &self.buffer,
                palette: &self.palette,
                db_floor: self.db_floor,
                db_ceiling: self.db_ceiling,
                width,
                height,
                style: self.export_style(),
                freq_scale: self.freq_scale,
                sample_rate: s.sample_rate,
                zoom: self.zoom,
                bins_mode: self.bins_mode,
                hop: s.hop_size,
                title: Some(format!(
                    "fs={}Hz N={} L={} H={} floor={} ceil={}",
                    s.sample_rate, s.fft_size, s.window_len, s.hop_size,
                    self.db_floor as i32, self.db_ceiling as i32
                )),
            },
            path,
        )
    }

    pub fn save_csv(&self, path: PathBuf) -> Result<()> {
        export::save_csv(&self.buffer, path)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RenderMode {
    /// One colored block per terminal cell (1x1)
    Cell,
    /// Two vertical sub-pixels per cell via '▀' (1x2)
    Half,
    /// Four sub-pixels per cell via quadrant glyphs (2x2)
    Quad,
}
