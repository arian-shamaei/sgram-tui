mod app;
mod colors;
mod config;
mod dsp;
mod export;
mod font;
mod input;
mod ui;
mod view;

use anyhow::Result;
use clap::{ArgAction, Parser, ValueEnum};

use app::{AnimationStyle, App, BinsMode, ColorPalette, Settings};
use app::FreqScale;

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum RenderArg { Cell, Half, Quad }

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum ResolutionArg { Low, Medium, High, Ultra }

#[derive(Parser, Debug)]
#[command(name = "sgram-tui", version, about = "Terminal spectrogram viewer", long_about = None)]
struct Cli {
    /// Input source: mic | wav | render | FILE
    #[arg(value_name = "SOURCE", help = "mic | wav | render (headless PNG/CSV export) | FILE (wav/mp3/flac/ogg path)", required = false)]
    source: Option<String>,

    /// Audio file path when SOURCE is 'wav', 'file', or 'render'
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

    /// History length (frames) for time resolution; default set by --resolution
    #[arg(long)]
    history: Option<usize>,

    /// Renderer: quad (2x2 sub-pixels, default), half (1x2), or cell (1x1)
    #[arg(long, value_enum)]
    render: Option<RenderArg>,

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

    /// Analysis window function
    #[arg(long, value_enum, default_value_t = WindowArg::Hann)]
    window: WindowArg,

    /// Bin display: all bins, or only local spectral maxima
    #[arg(long, value_enum, default_value_t = BinsArg::All)]
    bins: BinsArg,

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

    /// Clamp DSP output to dB floor before rendering
    #[arg(long, default_value_t = false)]
    clamp_floor: bool,

    /// Normalize each DSP frame to its max (peak=0 dB)
    #[arg(long, default_value_t = false)]
    normalize: bool,

    /// Disable microphone feature fallback check
    #[arg(long, action=ArgAction::SetTrue)]
    no_mic: bool,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum PaletteArg { Grayscale, Heat, Viridis, Jet, Inferno, Magma, Plasma, Purplefire }

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum AnimArg { Horizontal, Waterfall, Spectrum }

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum BinsArg { All, Peaks }

impl From<BinsArg> for BinsMode {
    fn from(v: BinsArg) -> Self {
        match v { BinsArg::All => Self::All, BinsArg::Peaks => Self::Peaks }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum FreqArg { Linear, Log, Mel }

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum WindowArg { Hann, Hamming, Blackman }

impl From<WindowArg> for dsp::WindowType {
    fn from(v: WindowArg) -> Self {
        match v {
            WindowArg::Hann => Self::Hann,
            WindowArg::Hamming => Self::Hamming,
            WindowArg::Blackman => Self::Blackman,
        }
    }
}

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
    fn from(v: AnimArg) -> Self {
        match v {
            AnimArg::Horizontal => Self::Horizontal,
            AnimArg::Waterfall => Self::Waterfall,
            AnimArg::Spectrum => Self::Spectrum,
        }
    }
}

impl From<RenderArg> for app::RenderMode {
    fn from(v: RenderArg) -> Self {
        match v {
            RenderArg::Cell => app::RenderMode::Cell,
            RenderArg::Half => app::RenderMode::Half,
            RenderArg::Quad => app::RenderMode::Quad,
        }
    }
}

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
        // Explicit --history always wins; otherwise the resolution preset decides
        history: cli.history.unwrap_or(match cli.resolution {
            ResolutionArg::Low => 256,
            ResolutionArg::Medium => 512,
            ResolutionArg::High => 1024,
            ResolutionArg::Ultra => 2048,
        }),
        render_mode: cli.render.map(Into::into).unwrap_or(app::RenderMode::Quad),
        freq_scale: cli.freq_scale.into(),
        alpha: if cli.alpha == 2 { 2 } else { 1 },
        pre_emphasis: cli.pre_emphasis,
        overview: cli.overview,
        realtime: cli.realtime,
        clamp_floor: cli.clamp_floor,
        normalize: cli.normalize,
        window: cli.window.into(),
        bins_mode: cli.bins.into(),
    };

    // Low preset drops to the cheap renderer unless --render was given
    if cli.resolution == ResolutionArg::Low && cli.render.is_none() {
        settings.render_mode = app::RenderMode::Cell;
    }

    // Resolve input per simplified usage: [mic|wav|render|FILE] [FILE]
    let mut headless = false;
    let input_arg = if let Some(src) = &cli.source {
        let s = src.to_lowercase();
        if s == "mic" { "mic".to_string() }
        else if s == "render" {
            if cli.file.is_none() && std::path::Path::new(src).exists() {
                // A file literally named "render" — treat as a direct path
                src.clone()
            } else {
                headless = true;
                cli.file.clone().ok_or_else(|| anyhow::anyhow!("Missing FILE after 'render'"))?
            }
        }
        else if s == "wav" || s == "file" {
            cli.file.clone().ok_or_else(|| anyhow::anyhow!("Missing FILE after 'wav'"))?
        } else {
            src.clone()
        }
    } else {
        return Err(anyhow::anyhow!("Usage: sgram-tui [mic|wav|render|FILE] [FILE] [flags]"));
    };

    if headless {
        // Offline figure export: process the whole file, no TUI.
        settings.history = 1_000_000;
        settings.realtime = false;
        return render_offline(&input_arg, settings, cli.png_path, cli.csv_path);
    }

    let device = cli.device.or_else(|| cfg.as_ref().and_then(|c| c.device.clone()));
    let mut app = App::new(input_arg, settings, cli.no_mic, device)?;

    if let Some(p) = cli.png_path.or_else(|| cfg.as_ref().and_then(|c| c.png_path.clone())) { app.export_png_path = Some(p.into()); }
    if let Some(p) = cli.csv_path.or_else(|| cfg.as_ref().and_then(|c| c.csv_path.clone())) { app.export_csv_path = Some(p.into()); }
    ui::run(&mut app)
}

fn render_offline(
    input: &str,
    settings: Settings,
    png_path: Option<String>,
    csv_path: Option<String>,
) -> Result<()> {
    use std::path::{Path, PathBuf};
    let mut app = App::new(input.to_string(), settings, true, None)?;
    // Drain the DSP pipeline until the decoder thread finishes and drops its sender
    while let Ok(row) = app.spectrogram_rx.recv() {
        app.push_row(row);
        app.total_rows = app.total_rows.saturating_add(1);
    }
    if app.buffer.is_empty() {
        // The decoder thread drops its sender (ending the recv loop) just
        // before it records the failure; give it a moment to land.
        let mut cause = None;
        for _ in 0..50 {
            if let Some(e) = app.pipeline_error.lock().unwrap().take() {
                cause = Some(e);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let cause = cause.map(|e| format!(": {e}")).unwrap_or_default();
        return Err(anyhow::anyhow!("no audio frames decoded from {input}{cause}"));
    }
    // Use the normalized settings: the DSP hop may have been clamped
    let hop = app.settings.hop_size;
    let sr = app.settings.sample_rate as f32;
    let bins = app.buffer.front().map(|r| r.len()).unwrap_or(0);
    let seconds = (app.total_rows as f32) * (hop as f32) / sr;
    let stem = Path::new(input)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "sgram".to_string());
    let png: PathBuf = png_path.map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("{stem}_sgram.png")));
    let (w, h) = app.png_content_dims();
    app.save_png(png.clone(), w, h)?;
    if app.total_rows > app.buffer.len() {
        let kept = (app.buffer.len() as f32) * (hop as f32) / sr;
        eprintln!(
            "warning: history capped at {} rows; figure shows only the last {:.2}s of {:.2}s",
            app.buffer.len(), kept, seconds
        );
    }
    println!(
        "wrote {} ({} rows x {} bins, {:.2}s of audio)",
        png.display(), app.total_rows, bins, seconds
    );
    if let Some(csv) = csv_path {
        let csv = PathBuf::from(csv);
        app.save_csv(csv.clone())?;
        println!("wrote {}", csv.display());
    }
    Ok(())
}
