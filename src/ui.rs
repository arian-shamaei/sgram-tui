use crate::app::{AnimationStyle, App};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use crate::app::{FreqScale};

enum UiMode {
    Normal,
    PromptSave { kind: SaveKind, input: String },
}

enum SaveKind { Png, Csv }

pub fn run(app: &mut App) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen, crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let tick_rate = app.tick_rate();
    let mut last_tick = Instant::now();
    let mut mode = UiMode::Normal;

    while app.running {
        terminal.draw(|f| draw(f, app, &mode)).ok();

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::from_millis(0));
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? { handle_key(app, key, &mut mode)?; }
        }
        if last_tick.elapsed() >= tick_rate {
            // Drain any available rows to minimize latency
            if !app.paused {
                let mut drained = 0usize;
                while let Ok(row) = app.spectrogram_rx.try_recv() {
                    let bins = row.len();
                    app.push_row(row, bins);
                    drained += 1;
                    app.stats_rows_count += 1;
                    app.total_rows = app.total_rows.saturating_add(1);
                    if drained > 1024 { break; }
                }
                let now = Instant::now();
                if now.duration_since(app.stats_last_instant) >= Duration::from_secs(1) {
                    app.stats_rows_sec = app.stats_rows_count as f32 / now.duration_since(app.stats_last_instant).as_secs_f32();
                    app.stats_rows_count = 0;
                    app.stats_last_instant = now;
                }
            }
            last_tick = Instant::now();
        }
    }

    disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), crossterm::event::DisableMouseCapture, crossterm::terminal::LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn handle_key(app: &mut App, key: KeyEvent, mode: &mut UiMode) -> Result<()> {
    match mode {
        UiMode::PromptSave { kind, input } => {
            match key.code {
                KeyCode::Esc => { *mode = UiMode::Normal; }
                KeyCode::Enter => {
                    let path = PathBuf::from(input.clone());
                    match kind {
                        SaveKind::Png => { app.save_png(path, 800, 600)?; }
                        SaveKind::Csv => { app.save_csv(path)?; }
                    }
                    *mode = UiMode::Normal;
                }
                KeyCode::Backspace => { input.pop(); }
                KeyCode::Char(c) => { input.push(c); }
                _ => {}
            }
            return Ok(());
        }
        UiMode::Normal => {}
    }

    let KeyEvent { code, modifiers, .. } = key;
    match (code, modifiers) {
        (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => app.running = false,
        (KeyCode::Char('p'), _) => app.toggle_pause(),
        (KeyCode::Char('a'), _) => app.toggle_style(),
        (KeyCode::Char('+'), _) | (KeyCode::Char('='), _) => app.adjust_zoom(0.25),
        (KeyCode::Char('-'), _) => app.adjust_zoom(-0.25),
        (KeyCode::Char('['), _) => app.adjust_floor(-2.0),
        (KeyCode::Char(']'), _) => app.adjust_floor(2.0),
        (KeyCode::Char('c'), _) => app.next_palette(),
        (KeyCode::Char('C'), _) => app.prev_palette(),
        (KeyCode::Char('s'), _) => save_png_default(app)?,
        (KeyCode::Char('w'), _) => save_csv_default(app)?,
        (KeyCode::Char('S'), _) => { *mode = UiMode::PromptSave { kind: SaveKind::Png, input: String::new() }; }
        (KeyCode::Char('W'), _) => { *mode = UiMode::PromptSave { kind: SaveKind::Csv, input: String::new() }; }
        (KeyCode::Char('f'), _) => { app.fullscreen = !app.fullscreen; }
        (KeyCode::Char('d'), _) => { app.detailed = !app.detailed; }
        (KeyCode::Char('o'), _) => { app.overview = !app.overview; }
        (KeyCode::Char('h'), _) | (KeyCode::F(1), _) => { app.toggle_help(); },
        _ => {}
    }
    Ok(())
}

fn ensure_saved_dir(path: PathBuf) -> PathBuf {
    if path.parent().map(|p| p.as_os_str().is_empty()).unwrap_or(true) {
        PathBuf::from("saved").join(path)
    } else { path }
}

fn save_png_default(app: &App) -> Result<()> {
    let base: PathBuf = app
        .export_png_path
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("sgram_{}.png", chrono_like_ts())));
    let path = ensure_saved_dir(base);
    // Approximate terminal pixel dims by cell count * 8x16
    let width = 800;
    let height = 600;
    app.save_png(path, width, height)?;
    Ok(())
}

fn save_csv_default(app: &App) -> Result<()> {
    let base: PathBuf = app
        .export_csv_path
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("sgram_{}.csv", chrono_like_ts())));
    let path = ensure_saved_dir(base);
    app.save_csv(path)?;
    Ok(())
}

fn chrono_like_ts() -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap();
    format!("{}", now.as_secs())
}

fn draw(f: &mut ratatui::Frame, app: &mut App, mode: &UiMode) {
    if app.fullscreen {
        let full = f.size();
        draw_spectrogram(f, full, app, mode);
        if app.show_help { draw_help(f, full); }
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3)].as_ref())
        .split(f.size());

    draw_spectrogram(f, chunks[0], app, mode);
    draw_status(f, chunks[1], app, mode);
    if app.show_help { draw_help(f, chunks[0]); }
}

fn draw_spectrogram(f: &mut ratatui::Frame, area: Rect, app: &mut App, mode: &UiMode) {
    let inner = if app.fullscreen {
        area
    } else {
        let block = Block::default().borders(Borders::ALL).title("sgram-tui");
        f.render_widget(block, area);
        if area.width < 4 || area.height < 4 { return; }
        Rect { x: area.x + 1, y: area.y + 1, width: area.width - 2, height: area.height - 2 }
    };

    match app.style {
        AnimationStyle::Waterfall => draw_waterfall(f, inner, app),
        AnimationStyle::Horizontal => draw_horizontal(f, inner, app),
    }
    if app.detailed { draw_overlay(f, inner, app, mode); }
}

fn draw_waterfall(f: &mut ratatui::Frame, area: Rect, app: &mut App) {
    let h = area.height as usize;
    let w = area.width as usize;
    let total = app.buffer.len();
    let rows = total.min(h);
    let bins = app.buffer.front().map(|r| r.len()).unwrap_or(1).max(1);
    match app.render_mode {
        crate::app::RenderMode::Cell => {
            for y in 0..rows {
                let src = if app.overview {
                    // Map y evenly across entire buffer (oldest at bottom)
                    let frac = 1.0 - (y as f32 + 0.5) / (h as f32);
                    let idx = ((total as f32 - 1.0) * frac).round() as usize;
                    &app.buffer[total - 1 - idx]
                } else {
                    &app.buffer[y]
                };
                let row_max = app.db_ceiling;
                let mut spans = Vec::with_capacity(w);
                for x in 0..w {
                    let idx = sample_bin_x(x, w, bins, app);
                    let db = *src.get(idx).unwrap_or(&app.db_floor);
                    let t = ((db - app.db_floor) / (row_max - app.db_floor).max(1.0)).clamp(0.0, 1.0);
                    spans.push(Span::styled(" ", Style::default().bg(app.palette.color_at(t))));
                }
                let line = Line::from(spans);
                let p = Paragraph::new(line);
                let r = Rect { x: area.x, y: area.y + y as u16, width: area.width, height: 1 };
                f.render_widget(p, r);
            }
        }
        crate::app::RenderMode::Half => {
            // Each terminal row shows two time rows (top newer, bottom older) using '▀' with fg/bg
            let half_rows = h.min((total + 1) / 2);
            for y in 0..half_rows {
                let (top, bot) = if app.overview {
                    let frac_top = 1.0 - ((y * 2) as f32 + 0.5) / (h as f32);
                    let frac_bot = 1.0 - ((y * 2 + 1) as f32 + 0.5) / (h as f32);
                    let idx_top = ((total as f32 - 1.0) * frac_top).round() as usize;
                    let idx_bot = ((total as f32 - 1.0) * frac_bot).round() as usize;
                    (&app.buffer[total - 1 - idx_top], &app.buffer[total - 1 - idx_bot])
                } else {
                    let top_idx = y * 2;
                    let bot_idx = (y * 2 + 1).min(total.saturating_sub(1));
                    (&app.buffer[top_idx], &app.buffer[bot_idx])
                };
                let row_max_top = top.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                let row_max_bot = bot.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                let mut spans = Vec::with_capacity(w);
                for x in 0..w {
                    let idx = sample_bin_x(x, w, bins, app);
                    let db_top = *top.get(idx).unwrap_or(&app.db_floor);
                    let db_bot = *bot.get(idx).unwrap_or(&app.db_floor);
                    let t_top = ((db_top - app.db_floor) / (row_max_top - app.db_floor).max(1.0)).clamp(0.0, 1.0);
                    let t_bot = ((db_bot - app.db_floor) / (row_max_bot - app.db_floor).max(1.0)).clamp(0.0, 1.0);
                    let style = Style::default().fg(app.palette.color_at(t_top)).bg(app.palette.color_at(t_bot));
                    spans.push(Span::styled("▀", style));
                }
                let p = Paragraph::new(Line::from(spans));
                let r = Rect { x: area.x, y: area.y + y as u16, width: area.width, height: 1 };
                f.render_widget(p, r);
            }
        }
    }
}

fn draw_horizontal(f: &mut ratatui::Frame, area: Rect, app: &mut App) {
    // Time runs left->right (newest on right), frequency low->high is bottom->top
    let w = area.width as usize;
    let h = area.height as usize;
    let time_len = app.buffer.len().max(1);
    let bins = app.buffer.front().map(|r| r.len()).unwrap_or(1).max(1);
    for y in 0..h {
        let mut spans = Vec::with_capacity(w);
        for x in 0..w {
            let t_idx = ((x as f32) / (w as f32) * (time_len as f32)) as usize;
            let t_idx = t_idx.min(time_len.saturating_sub(1));
            if let Some(row) = app.buffer.get(time_len - 1 - t_idx) { // newest on right
                // invert vertical so low freq at bottom
                let bin_idx = sample_bin_y(y, h, bins, app);
                let row_max = app.db_ceiling;
                let db = *row.get(bin_idx).unwrap_or(&app.db_floor);
                let t = ((db - app.db_floor) / (row_max - app.db_floor).max(1.0)).clamp(0.0, 1.0);
                spans.push(Span::styled(" ", Style::default().bg(app.palette.color_at(t))));
            } else {
                spans.push(Span::raw(" "));
            }
        }
        let p = Paragraph::new(Line::from(spans));
        let r = Rect { x: area.x, y: area.y + y as u16, width: area.width, height: 1 };
        f.render_widget(p, r);
    }
}

fn draw_status(f: &mut ratatui::Frame, area: Rect, app: &App, mode: &UiMode) {
    if app.fullscreen { return; }
    let mut lines = vec![
        Line::from(vec![
            Span::raw("[q] quit  [p] pause  [a] style  [+/-] zoom  [[/]] floor  [c/C] palette  [s/S] png  [w/W] csv  [f] fullscreen  [d] details  [o] overview  [h] help"),
        ]),
    ];
    let f_max = (app.settings.sample_rate as f32) / 2.0 / app.zoom;
    let seconds = (app.buffer.len() as f32) * (app.settings.hop_size as f32) / (app.settings.sample_rate as f32);
    lines.push(Line::from(Span::raw(format!(
        "src: {} | style: {:?} | zoom: {:.2} | floor: {:.1} dB ceil: {:.1} | rows: {} | freq: 0..{:.0} Hz | time: 0..{:.2}s | L/H/N: {}/{}/{} | fps: {} | rps: {:.1} | rt: {} | scale: {:?} | render: {:?}",
        app.input_desc,
        app.style,
        app.zoom,
        app.db_floor,
        app.db_ceiling,
        app.buffer.len(),
        f_max,
        seconds,
        app.settings.window_len,
        app.settings.hop_size,
        app.settings.fft_size,
        app.settings.fps,
        app.stats_rows_sec,
        if app.realtime { "on" } else { "off" },
        app.freq_scale,
        app.render_mode
    ))));
    if let UiMode::PromptSave { kind, input } = mode {
        let title = match kind { SaveKind::Png => "PNG path:", SaveKind::Csv => "CSV path:" };
        lines.push(Line::from(Span::raw(format!("{} {}", title, input))));
    }
    let p = Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("status"));
    f.render_widget(p, area);
}

fn draw_overlay(f: &mut ratatui::Frame, area: Rect, app: &App, _mode: &UiMode) {
    // Frequency markers with labels (left side)
    let ticks = 4;
    for i in 0..=ticks {
        let y = area.y + (i as u16) * (area.height.saturating_sub(1)) / (ticks as u16);
        let frac = (ticks - i) as f32 / ticks as f32;
        let freq = map_frac_to_freq(frac, app);
        let label = format!("{:.0}Hz", freq);
        let p = Paragraph::new(label.clone());
        let r = Rect { x: area.x, y, width: label.len() as u16, height: 1 };
        f.render_widget(p, r);
    }
    // Metadata panel (top-right)
    let panel_w = area.width.min(52);
    let panel_h = 6u16;
    let px = area.x + area.width.saturating_sub(panel_w) - 1;
    let py = area.y;
    let df = (app.settings.sample_rate as f32) / (app.settings.fft_size as f32);
    let rps = app.stats_rows_sec;
    let rtf = rps * (app.settings.hop_size as f32) / (app.settings.sample_rate as f32);
    let total_time = (app.total_rows as f32) * (app.settings.hop_size as f32) / (app.settings.sample_rate as f32);
    let meta = vec![
        Line::from(format!("src: {}", app.input_desc)),
        Line::from(format!("fs: {} Hz | L/H/N: {}/{}/{}", app.settings.sample_rate, app.settings.window_len, app.settings.hop_size, app.settings.fft_size)),
        Line::from(format!("bins: {} | df: {:.1} Hz", app.settings.fft_size/2, df)),
        Line::from(format!("floor/ceil: {:.0}/{:.0} dB | zoom: {:.2}", app.db_floor, app.db_ceiling, app.zoom)),
        Line::from(format!("throughput: {:.1} rows/s | RTF: {:.2}x | total: {:.2}s", rps, rtf, total_time)),
        Line::from(format!("scale: {:?} | render: {:?}", app.freq_scale, app.render_mode)),
    ];
    let p = Paragraph::new(meta).block(Block::default().borders(Borders::ALL).title("details"));
    let rect = Rect { x: px, y: py, width: panel_w, height: panel_h };
    f.render_widget(p, rect);
}

fn sample_bin_x(x: usize, w: usize, bins: usize, app: &App) -> usize {
    let t = (x as f32) / (w as f32);
    map_t_to_bin(t, bins, app)
}

fn sample_bin_y(y: usize, h: usize, bins: usize, app: &App) -> usize {
    // invert vertical so low freq at bottom
    let t = 1.0 - (y as f32) / (h as f32);
    map_t_to_bin(t, bins, app)
}

fn map_t_to_bin(t: f32, bins: usize, app: &App) -> usize {
    let fs = app.settings.sample_rate as f32;
    let fmax = fs / 2.0 / app.zoom.max(1.0);
    let fmin = match app.freq_scale { FreqScale::Linear => 0.0, _ => 20.0 };
    let f = map_frac_to_freq(t, app);
    let hz_per_bin = fmax / (bins as f32);
    let idx = if hz_per_bin > 0.0 { (f / hz_per_bin).floor() as usize } else { 0 };
    idx.min(bins.saturating_sub(1))
}

fn map_frac_to_freq(t: f32, app: &App) -> f32 {
    let fs = app.settings.sample_rate as f32;
    let fmax = fs / 2.0 / app.zoom.max(1.0);
    let fmin = match app.freq_scale { FreqScale::Linear => 0.0, _ => 20.0 };
    match app.freq_scale {
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
}

// help overlay
fn draw_help(f: &mut ratatui::Frame, area: Rect) {
    let lines = vec![
        Line::from("Usage: sgram-tui [mic|wav|FILE] [FILE] [flags]"),
        Line::from("Examples: sgram-tui wav song.wav  |  sgram-tui mic  |  sgram-tui song.wav"),
        Line::from("Keys: q/Esc quit, p pause, a style, +/- zoom, [[/]] floor, c/C palette, f fullscreen, o overview, d details, s/S png, w/W csv, h help"),
    ];
    let p = Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Help"));
    let w = area.width.min(100);
    let h = 5;
    let x = area.x + (area.width - w) / 2;
    let y = area.y + (area.height - h) / 2;
    f.render_widget(p, Rect { x, y, width: w, height: h });
}
