mod app;
mod colors;
mod config;
mod dsp;
mod export;
mod input;
mod ui;

use anyhow::Result;
use clap::{ArgAction, Parser, ValueEnum};

use app::{AnimationStyle, App, ColorPalette, Settings};
use app::FreqScale;

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum RenderArg { Cell, Half }

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum ResolutionArg { Low, Medium, High, Ultra }

#[derive(Parser, Debug)]
#[command(name = "sgram-tui", version, about = "Terminal spectrogram viewer", long_about = None)]
struct Cli {
    /// Input source: mic | wav | FILE
    #[arg(value_name = "SOURCE", help = "mic | wav | FILE (direct path)", required = false)]
    source: Option<String>,

    /// Audio file path when SOURCE is 'wav' or 'file'
    #[arg(value_name = "FILE", required = false)]
    file: Option<String>,

    /// FFT size (power of two), controls frequency resolution
    #[arg(long, default_value_t = 1024)]
    fft: usize,

    /// Window length (frame size) L in samples (<= fft); zero-pad if smaller than FFT
    #[arg(long)]
    win: Option<usize>,

    /// Hop size between FFT windows (<= fft)
    #[arg(long, default_value_t = 256)]
    hop: usize,

    /// Target sample rate for processing
    #[arg(long, default_value_t = 48000)]
    sample_rate: u32,

    /// Minimum dB floor (lower = more dynamic range)
    #[arg(long, default_value_t = -80.0, allow_negative_numbers = true)]
    floor: f32,

    /// dB ceiling (upper bound, typically 0 dB)
    #[arg(long, default_value_t = 0.0, allow_negative_numbers = true)]
    ceil: f32,

    /// Frames per second for UI updates
    #[arg(long, default_value_t = 30)]
    fps: u64,

    /// Initial zoom (>1 zooms into low frequencies)
    #[arg(long, default_value_t = 1.0)]
    zoom: f32,

    /// Initial palette
    #[arg(long, value_enum, default_value_t = PaletteArg::Viridis)]
    palette: PaletteArg,

    /// Animation style (horizontal sweep or vertical waterfall)
    #[arg(long, value_enum, default_value_t = AnimArg::Waterfall)]
    style: AnimArg,

    /// Detailed view (show frequency range and scale)
    #[arg(long, default_value_t = false)]
    detailed: bool,

    /// Fullscreen mode (hide borders and status)
    #[arg(long, default_value_t = false)]
    fullscreen: bool,

    /// History length (frames) for time resolution
    #[arg(long, default_value_t = 512)]
    history: usize,

    /// Renderer: cell (1x1) or half (two vertical bins per row)
    #[arg(long, value_enum, default_value_t = RenderArg::Cell)]
    render: RenderArg,

    /// Resolution preset (affects history and renderer if not overridden)
    #[arg(long, value_enum, default_value_t = ResolutionArg::Medium)]
    resolution: ResolutionArg,

    /// PNG export path (default uses timestamp)
    #[arg(long)]
    png_path: Option<String>,

    /// CSV export path (default uses timestamp)
    #[arg(long)]
    csv_path: Option<String>,

    /// Input device name substring (for mic)
    #[arg(long)]
    device: Option<String>,

    /// Frequency scale for display
    #[arg(long, value_enum, default_value_t = FreqArg::Linear)]
    freq_scale: FreqArg,

    /// Magnitude exponent alpha (1=magnitude, 2=power)
    #[arg(long, default_value_t = 1)]
    alpha: u8,

    /// Pre-emphasis beta (0..1), e.g. 0.97; omit to disable
    #[arg(long)]
    pre_emphasis: Option<f32>,

    /// Overview mode: fit entire buffer into view
    #[arg(long, default_value_t = false)]
    overview: bool,

    /// Realtime sync for WAV input (sleep to emulate real time)
    #[arg(long, default_value_t = false)]
    realtime: bool,

    /// Disable microphone feature fallback check
    #[arg(long, action=ArgAction::SetTrue)]
    no_mic: bool,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum PaletteArg { Grayscale, Heat, Viridis, Jet, Inferno, Magma, Plasma, Purplefire }

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum AnimArg { Horizontal, Waterfall }

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum FreqArg { Linear, Log, Mel }

impl From<PaletteArg> for ColorPalette {
    fn from(v: PaletteArg) -> Self {
        match v {
            PaletteArg::Grayscale => Self::Grayscale,
            PaletteArg::Heat => Self::Heat,
            PaletteArg::Viridis => Self::Viridis,
            PaletteArg::Jet => Self::Jet,
            PaletteArg::Inferno => Self::Inferno,
            PaletteArg::Magma => Self::Magma,
            PaletteArg::Plasma => Self::Plasma,
            PaletteArg::Purplefire => Self::PurpleFire,
        }
    }
}

impl From<AnimArg> for AnimationStyle {
    fn from(v: AnimArg) -> Self { match v { AnimArg::Horizontal => Self::Horizontal, AnimArg::Waterfall => Self::Waterfall } }
}

impl From<RenderArg> for app::RenderMode { fn from(v: RenderArg) -> Self { match v { RenderArg::Cell => app::RenderMode::Cell, RenderArg::Half => app::RenderMode::Half } } }

impl From<FreqArg> for FreqScale { fn from(v: FreqArg) -> Self { match v { FreqArg::Linear => FreqScale::Linear, FreqArg::Log => FreqScale::Log, FreqArg::Mel => FreqScale::Mel } } }

fn main() -> Result<()> {
    let cli = Cli::parse();
    // Load config defaults
    let cfg = config::load_config();

    let mut settings = Settings {
        fft_size: cli.fft.max(16),
        hop_size: cli.hop.max(1).min(cli.fft.max(16)),
        window_len: cli.win.unwrap_or(cli.fft).min(cli.fft).max(16),
        sample_rate: cli.sample_rate,
        db_floor: cli.floor,
        db_ceiling: cli.ceil,
        fps: cli.fps,
        zoom: cli.zoom.max(1.0),
        palette: cli.palette.into(),
        style: cli.style.into(),
        detailed: cli.detailed || cfg.as_ref().map(|c| c.detailed).unwrap_or(false),
        fullscreen: cli.fullscreen || cfg.as_ref().map(|c| c.fullscreen).unwrap_or(false),
        history: cli.history,
        render_mode: cli.render.into(),
        freq_scale: cli.freq_scale.into(),
        alpha: if cli.alpha == 2 { 2 } else { 1 },
        pre_emphasis: cli.pre_emphasis,
        overview: cli.overview,
        realtime: cli.realtime,
    };

    // Apply resolution preset as a convenience when using defaults
    match cli.resolution {
        ResolutionArg::Low => {
            settings.history = 256;
            if matches!(cli.render, RenderArg::Cell) { settings.render_mode = app::RenderMode::Cell; }
        }
        ResolutionArg::Medium => {}
        ResolutionArg::High => {
            settings.history = settings.history.max(1024);
            if matches!(cli.render, RenderArg::Cell) { settings.render_mode = app::RenderMode::Half; }
        }
        ResolutionArg::Ultra => {
            settings.history = settings.history.max(2048);
            if matches!(cli.render, RenderArg::Cell) { settings.render_mode = app::RenderMode::Half; }
        }
    }

    // Resolve input per simplified usage: [mic|wav|FILE] [FILE]
    let input_arg = if let Some(src) = &cli.source {
        let s = src.to_lowercase();
        if s == "mic" { "mic".to_string() }
        else if s == "wav" || s == "file" {
            cli.file.clone().ok_or_else(|| anyhow::anyhow!("Missing FILE after 'wav'"))?
        } else {
            src.clone()
        }
    } else {
        return Err(anyhow::anyhow!("Usage: sgram-tui [mic|wav|FILE] [FILE] [flags]"));
    };

    let device = cli.device.or_else(|| cfg.as_ref().and_then(|c| c.device.clone()));
    let mut app = App::new(input_arg, settings, cli.no_mic, device)?;

    if let Some(p) = cli.png_path.or_else(|| cfg.as_ref().and_then(|c| c.png_path.clone())) { app.export_png_path = Some(p.into()); }
    if let Some(p) = cli.csv_path.or_else(|| cfg.as_ref().and_then(|c| c.csv_path.clone())) { app.export_csv_path = Some(p.into()); }
    ui::run(&mut app)
}
