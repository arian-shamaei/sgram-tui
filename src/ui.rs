use crate::app::{AnimationStyle, App, BinsMode};
use crate::view::{self, FreqMap};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, MouseEventKind};
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

enum UiMode {
    Normal,
    PromptSave { kind: SaveKind, input: String },
}

enum SaveKind { Png, Csv }

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = crossterm::execute!(
        io::stdout(),
        crossterm::event::DisableMouseCapture,
        crossterm::terminal::LeaveAlternateScreen
    );
}

pub fn run(app: &mut App) -> Result<()> {
    // Restore the terminal even if we panic mid-draw, so the shell is never
    // left in raw mode / the alternate screen.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen, crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = event_loop(app, &mut terminal);

    restore_terminal();
    terminal.show_cursor()?;
    result
}

fn event_loop(app: &mut App, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let tick_rate = app.tick_rate();
    let mut last_tick = Instant::now();
    let mut mode = UiMode::Normal;

    while app.running {
        terminal.draw(|f| draw(f, app, &mode)).ok();

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::from_millis(0));
        if crossterm::event::poll(timeout)? {
            match event::read()? {
                // Windows delivers Press AND Release for every keystroke;
                // acting on both double-fires toggles and doubles prompt input.
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    handle_key(app, key, &mut mode)?
                }
                Event::Key(_) => {}
                Event::Mouse(me) => match me.kind {
                    MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                        app.hover = Some((me.column, me.row));
                        app.hover_at = Instant::now();
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        // Surface input-pipeline failures (bad path, unsupported codec, ...)
        // that would otherwise be invisible under the alternate screen.
        if app.error.is_none() {
            if let Some(e) = app.pipeline_error.lock().unwrap().take() {
                app.error = Some(format!("input error: {e}"));
            }
        }
        if last_tick.elapsed() >= tick_rate {
            // Drain any available rows to minimize latency
            if !app.paused {
                let mut drained = 0usize;
                while let Ok(row) = app.spectrogram_rx.try_recv() {
                    app.push_row(row);
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
    Ok(())
}

fn handle_key(app: &mut App, key: KeyEvent, mode: &mut UiMode) -> Result<()> {
    match mode {
        UiMode::PromptSave { kind, input } => {
            match key.code {
                KeyCode::Esc => { *mode = UiMode::Normal; }
                KeyCode::Enter => {
                    let path = PathBuf::from(input.clone());
                    let result = match kind {
                        SaveKind::Png => {
                            let (w, h) = app.png_content_dims();
                            app.save_png(path.clone(), w, h)
                        }
                        SaveKind::Csv => app.save_csv(path.clone()),
                    };
                    report_save(app, result, &path);
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
        (KeyCode::Char('s'), _) => save_png_default(app),
        (KeyCode::Char('w'), _) => save_csv_default(app),
        (KeyCode::Char('S'), _) => { *mode = UiMode::PromptSave { kind: SaveKind::Png, input: String::new() }; }
        (KeyCode::Char('W'), _) => { *mode = UiMode::PromptSave { kind: SaveKind::Csv, input: String::new() }; }
        (KeyCode::Char('r'), _) => { app.clear(); app.set_status("history cleared"); }
        (KeyCode::Char('b'), _) => {
            app.toggle_bins_mode();
            app.set_status(match app.bins_mode {
                BinsMode::All => "bins: all",
                BinsMode::Peaks => "bins: peaks only (local spectral maxima)",
            });
        }
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

fn report_save(app: &mut App, result: Result<()>, path: &std::path::Path) {
    match result {
        Ok(()) => app.set_status(format!("saved {}", path.display())),
        Err(e) => app.set_status(format!("save failed: {e}")),
    }
}

fn save_png_default(app: &mut App) {
    let base: PathBuf = app
        .export_png_path
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("sgram_{}.png", chrono_like_ts())));
    let path = ensure_saved_dir(base);
    let (width, height) = app.png_content_dims();
    let result = app.save_png(path.clone(), width, height);
    report_save(app, result, &path);
}

fn save_csv_default(app: &mut App) {
    let base: PathBuf = app
        .export_csv_path
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("sgram_{}.csv", chrono_like_ts())));
    let path = ensure_saved_dir(base);
    let result = app.save_csv(path.clone());
    report_save(app, result, &path);
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
    // Constant status height (keys + info + message line) so the spectrogram
    // pane never jumps when a status message or prompt appears.
    let status_h = 5u16;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(status_h)].as_ref())
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
        AnimationStyle::Spectrum => draw_spectrum(f, inner, app),
    }
    if app.detailed { draw_overlay(f, inner, app, mode); }
    draw_hover_readout(f, inner, app);
}

fn fmap(app: &App) -> FreqMap {
    FreqMap {
        sample_rate: app.settings.sample_rate,
        zoom: app.zoom.max(1.0),
        scale: app.freq_scale,
    }
}

/// Palette position (0..1) for a pooled dB value.
fn color_frac(app: &App, v: f32) -> f32 {
    ((v - app.db_floor) / (app.db_ceiling - app.db_floor).max(1.0)).clamp(0.0, 1.0)
}

/// Buffer row range shown by terminal row `y` in waterfall mode (Cell renderer).
fn waterfall_row_range(app: &App, y: usize, h: usize, total: usize) -> (usize, usize) {
    if app.overview {
        view::overview_row_range(y, h, total)
    } else {
        (y, (y + 1).min(total))
    }
}

/// Quadrant glyphs indexed by sub-pixel bits (TL=8, TR=4, BL=2, BR=1).
const QUAD_CHARS: [&str; 16] = [
    " ", "▗", "▖", "▄", "▝", "▐", "▞", "▟", "▘", "▚", "▌", "▙", "▀", "▜", "▛", "█",
];

/// Render a 2x2 sub-pixel cell: quantize the four palette positions into a
/// fg/bg pair and pick the quadrant glyph matching the brighter group.
/// ts order: [TL, TR, BL, BR].
fn quad_cell_span(app: &App, ts: [f32; 4]) -> Span<'static> {
    let mn = ts.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let mx = ts.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
    if mx - mn < 1.0 / 255.0 {
        // Uniform cell: solid background, no glyph edges
        return Span::styled(" ", Style::default().bg(app.palette.color_at(mx)));
    }
    let thr = (mn + mx) / 2.0;
    let mut bits = 0usize;
    let (mut fg_sum, mut fg_n) = (0.0f32, 0u32);
    let (mut bg_sum, mut bg_n) = (0.0f32, 0u32);
    for (i, &t) in ts.iter().enumerate() {
        if t >= thr {
            bits |= 8 >> i;
            fg_sum += t;
            fg_n += 1;
        } else {
            bg_sum += t;
            bg_n += 1;
        }
    }
    let fg = app.palette.color_at(fg_sum / (fg_n.max(1) as f32));
    let bg = app.palette.color_at(if bg_n > 0 { bg_sum / bg_n as f32 } else { mn });
    Span::styled(QUAD_CHARS[bits], Style::default().fg(fg).bg(bg))
}

fn draw_waterfall(f: &mut ratatui::Frame, area: Rect, app: &mut App) {
    let h = area.height as usize;
    let w = area.width as usize;
    let total = app.buffer.len();
    if total == 0 || w == 0 || h == 0 { return; }
    let bins = app.buffer.front().map(|r| r.len()).unwrap_or(1).max(1);
    let m = fmap(app);
    // Bin ranges per column: every covered bin is max-pooled, so narrowband
    // peaks are never lost when many bins map to one cell.
    let ranges: Vec<(usize, usize)> = (0..w).map(|x| m.cell_bin_range(x, w, bins)).collect();
    match app.render_mode {
        crate::app::RenderMode::Cell => {
            let rows = if app.overview { h } else { total.min(h) };
            for y in 0..rows {
                let (r0, r1) = waterfall_row_range(app, y, h, total);
                let mut spans = Vec::with_capacity(w);
                for &(lo, hi) in &ranges {
                    let v = view::pool_cell(&app.buffer, r0, r1, lo, hi, app.bins_mode);
                    spans.push(Span::styled(" ", Style::default().bg(app.palette.color_at(color_frac(app, v)))));
                }
                let r = Rect { x: area.x, y: area.y + y as u16, width: area.width, height: 1 };
                f.render_widget(Paragraph::new(Line::from(spans)), r);
            }
        }
        crate::app::RenderMode::Half => {
            // Two time rows per terminal row via '▀' (fg = newer, bg = older);
            // treat the pane as a virtual grid of 2*h pixel rows.
            let vrows = h * 2;
            let cell_rows = if app.overview { h } else { h.min(total.div_ceil(2)) };
            for y in 0..cell_rows {
                let (t0, t1) = if app.overview {
                    view::overview_row_range(2 * y, vrows, total)
                } else {
                    (2 * y, (2 * y + 1).min(total))
                };
                let (b0, b1) = if app.overview {
                    view::overview_row_range(2 * y + 1, vrows, total)
                } else {
                    ((2 * y + 1).min(total), (2 * y + 2).min(total))
                };
                let mut spans = Vec::with_capacity(w);
                for &(lo, hi) in &ranges {
                    let v_top = view::pool_cell(&app.buffer, t0, t1, lo, hi, app.bins_mode);
                    let v_bot = view::pool_cell(&app.buffer, b0, b1, lo, hi, app.bins_mode);
                    let style = Style::default()
                        .fg(app.palette.color_at(color_frac(app, v_top)))
                        .bg(app.palette.color_at(color_frac(app, v_bot)));
                    spans.push(Span::styled("▀", style));
                }
                let r = Rect { x: area.x, y: area.y + y as u16, width: area.width, height: 1 };
                f.render_widget(Paragraph::new(Line::from(spans)), r);
            }
        }
        crate::app::RenderMode::Quad => {
            // 2x2 sub-pixels per cell: virtual grid of 2*w x 2*h pixels
            let vrows = h * 2;
            let vcols = w * 2;
            let vranges: Vec<(usize, usize)> = (0..vcols).map(|x| m.cell_bin_range(x, vcols, bins)).collect();
            let cell_rows = if app.overview { h } else { h.min(total.div_ceil(2)) };
            for y in 0..cell_rows {
                let sub_rows: [(usize, usize); 2] = if app.overview {
                    [
                        view::overview_row_range(2 * y, vrows, total),
                        view::overview_row_range(2 * y + 1, vrows, total),
                    ]
                } else {
                    [
                        (2 * y, (2 * y + 1).min(total)),
                        ((2 * y + 1).min(total), (2 * y + 2).min(total)),
                    ]
                };
                let mut spans = Vec::with_capacity(w);
                for x in 0..w {
                    let mut ts = [0.0f32; 4]; // TL, TR, BL, BR
                    for (i, t) in ts.iter_mut().enumerate() {
                        let (r0, r1) = sub_rows[i / 2];
                        let (lo, hi) = vranges[2 * x + (i % 2)];
                        let v = view::pool_cell(&app.buffer, r0, r1, lo, hi, app.bins_mode);
                        *t = color_frac(app, v);
                    }
                    spans.push(quad_cell_span(app, ts));
                }
                let r = Rect { x: area.x, y: area.y + y as u16, width: area.width, height: 1 };
                f.render_widget(Paragraph::new(Line::from(spans)), r);
            }
        }
    }
}

/// Buffer row range covered by column `x` of `w` in horizontal mode
/// (time left->right, newest on the right; buffer index 0 = newest).
fn horizontal_col_range(x: usize, w: usize, total: usize) -> (usize, usize) {
    let w = w.max(1);
    let t0 = x * total / w;
    let t1 = ((x + 1) * total).div_ceil(w).clamp(t0 + 1, total.max(1));
    (total - t1, total - t0)
}

fn draw_horizontal(f: &mut ratatui::Frame, area: Rect, app: &mut App) {
    // Time runs left->right (newest on right), frequency low->high is bottom->top
    let w = area.width as usize;
    let h = area.height as usize;
    let total = app.buffer.len();
    if total == 0 || w == 0 || h == 0 { return; }
    let bins = app.buffer.front().map(|r| r.len()).unwrap_or(1).max(1);
    let m = fmap(app);
    if app.render_mode == crate::app::RenderMode::Quad {
        // 2x2 sub-pixels per cell
        let vrows = h * 2;
        let vcols = w * 2;
        let col_ranges: Vec<(usize, usize)> = (0..vcols).map(|x| horizontal_col_range(x, vcols, total)).collect();
        let bin_ranges: Vec<(usize, usize)> = (0..vrows).map(|vy| m.cell_bin_range(vrows - 1 - vy, vrows, bins)).collect();
        for y in 0..h {
            let mut spans = Vec::with_capacity(w);
            for x in 0..w {
                let mut ts = [0.0f32; 4]; // TL, TR, BL, BR
                for (i, t) in ts.iter_mut().enumerate() {
                    let (lo, hi) = bin_ranges[2 * y + i / 2];
                    let (r0, r1) = col_ranges[2 * x + (i % 2)];
                    let v = view::pool_cell(&app.buffer, r0, r1, lo, hi, app.bins_mode);
                    *t = color_frac(app, v);
                }
                spans.push(quad_cell_span(app, ts));
            }
            let r = Rect { x: area.x, y: area.y + y as u16, width: area.width, height: 1 };
            f.render_widget(Paragraph::new(Line::from(spans)), r);
        }
        return;
    }
    let col_ranges: Vec<(usize, usize)> = (0..w).map(|x| horizontal_col_range(x, w, total)).collect();
    for y in 0..h {
        let (lo, hi) = m.cell_bin_range(h - 1 - y, h, bins); // low freq at bottom
        let mut spans = Vec::with_capacity(w);
        for &(r0, r1) in &col_ranges {
            let v = view::pool_cell(&app.buffer, r0, r1, lo, hi, app.bins_mode);
            spans.push(Span::styled(" ", Style::default().bg(app.palette.color_at(color_frac(app, v)))));
        }
        let r = Rect { x: area.x, y: area.y + y as u16, width: area.width, height: 1 };
        f.render_widget(Paragraph::new(Line::from(spans)), r);
    }
}

const EIGHTHS: [&str; 9] = [" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

fn draw_spectrum(f: &mut ratatui::Frame, area: Rect, app: &mut App) {
    // Instantaneous spectrum of the newest frame: frequency on x, dB on y.
    let w = area.width as usize;
    let h = area.height as usize;
    let Some(row) = app.buffer.front() else { return };
    if w == 0 || h == 0 { return; }
    let bins = row.len().max(1);
    let m = fmap(app);
    // Bar height in eighth-blocks per column (sub-cell vertical resolution)
    let bars: Vec<(usize, f32)> = (0..w)
        .map(|x| {
            let (lo, hi) = m.cell_bin_range(x, w, bins);
            let (bi, mut v) = view::max_bin_in(row, lo, hi);
            if app.bins_mode == BinsMode::Peaks && !view::is_local_peak(row, bi) {
                v = f32::NEG_INFINITY;
            }
            let frac = color_frac(app, v);
            (((frac * (h * 8) as f32).round()) as usize, frac)
        })
        .collect();
    for y in 0..h {
        let cell_base = (h - 1 - y) * 8;
        let mut spans = Vec::with_capacity(w);
        for &(bar8, frac) in &bars {
            let fill = bar8.saturating_sub(cell_base).min(8);
            let style = Style::default().fg(app.palette.color_at(frac.max(0.15)));
            spans.push(Span::styled(EIGHTHS[fill], style));
        }
        let r = Rect { x: area.x, y: area.y + y as u16, width: area.width, height: 1 };
        f.render_widget(Paragraph::new(Line::from(spans)), r);
    }
    // Peak annotation: strongest displayed bin. Drawn at the top-left so the
    // details panel (top-right) can't cover it; a hover readout replaces it.
    if app.detailed && area.width > 24 && app.active_hover().is_none() {
        let fs = app.settings.sample_rate as f32;
        let hz_per_bin = (fs / 2.0) / (bins as f32);
        let vis_hi = ((fs / 2.0 / app.zoom.max(1.0)) / hz_per_bin).ceil() as usize;
        let (pi, pv) = view::max_bin_in(row, 0, vis_hi.clamp(1, bins));
        if pv.is_finite() {
            let label = format!(" peak {} {:+.1} dB ", format_freq(pi as f32 * hz_per_bin), pv);
            let lw = (label.chars().count() as u16).min(area.width);
            // Top-center: clear of the dB axis labels (left) and details panel (right)
            let r = Rect { x: area.x + (area.width - lw) / 2, y: area.y, width: lw, height: 1 };
            f.render_widget(Paragraph::new(label).style(Style::default().add_modifier(ratatui::style::Modifier::REVERSED)), r);
        }
    }
}

fn draw_hover_readout(f: &mut ratatui::Frame, inner: Rect, app: &App) {
    let Some((cx, cy)) = app.active_hover() else { return };
    if cx < inner.x || cy < inner.y || cx >= inner.x + inner.width || cy >= inner.y + inner.height {
        return;
    }
    let total = app.buffer.len();
    if total == 0 || inner.width < 24 { return; }
    let x = (cx - inner.x) as usize;
    let y = (cy - inner.y) as usize;
    let w = inner.width as usize;
    let h = inner.height as usize;
    let bins = app.buffer.front().map(|r| r.len()).unwrap_or(1).max(1);
    let m = fmap(app);
    let fs = app.settings.sample_rate as f32;
    let sec_per_row = (app.settings.hop_size as f32) / fs.max(1.0);

    let text = match app.style {
        AnimationStyle::Waterfall => {
            let (lo, hi) = m.cell_bin_range(x, w, bins);
            let (r0, r1) = match app.render_mode {
                crate::app::RenderMode::Cell => waterfall_row_range(app, y, h, total),
                crate::app::RenderMode::Half | crate::app::RenderMode::Quad => {
                    if app.overview {
                        let (a, _) = view::overview_row_range(2 * y, h * 2, total);
                        let (_, b) = view::overview_row_range(2 * y + 1, h * 2, total);
                        (a, b)
                    } else {
                        (2 * y, (2 * y + 2).min(total))
                    }
                }
            };
            if r0 >= total { return; }
            let v = view::pool_cell(&app.buffer, r0, r1, lo, hi, app.bins_mode);
            let f_mid = m.frac_to_freq((x as f32 + 0.5) / w as f32);
            let sec = ((r0 + r1) as f32 / 2.0) * sec_per_row;
            format!(" t -{:.2}s | {} | {} ", sec, format_freq(f_mid), db_str(v, app))
        }
        AnimationStyle::Horizontal => {
            let (lo, hi) = m.cell_bin_range(h - 1 - y, h, bins);
            let (r0, r1) = horizontal_col_range(x, w, total);
            if r0 >= total { return; }
            let v = view::pool_cell(&app.buffer, r0, r1, lo, hi, app.bins_mode);
            let f_mid = m.frac_to_freq(1.0 - (y as f32 + 0.5) / h as f32);
            let sec = ((r0 + r1) as f32 / 2.0) * sec_per_row;
            format!(" t -{:.2}s | {} | {} ", sec, format_freq(f_mid), db_str(v, app))
        }
        AnimationStyle::Spectrum => {
            let row = app.buffer.front().expect("total > 0");
            let (lo, hi) = m.cell_bin_range(x, w, bins);
            let (bi, mut v) = view::max_bin_in(row, lo, hi);
            // Match the bars: suppressed non-peaks read as below-floor
            if app.bins_mode == BinsMode::Peaks && !view::is_local_peak(row, bi) {
                v = f32::NEG_INFINITY;
            }
            let f_mid = m.frac_to_freq((x as f32 + 0.5) / w as f32);
            format!(" {} | {} ", format_freq(f_mid), db_str(v, app))
        }
    };
    let lw = (text.chars().count() as u16).min(inner.width);
    let r = Rect { x: inner.x, y: inner.y, width: lw, height: 1 };
    f.render_widget(
        Paragraph::new(text).style(Style::default().add_modifier(ratatui::style::Modifier::REVERSED)),
        r,
    );
}

fn db_str(v: f32, app: &App) -> String {
    if v.is_finite() {
        format!("{:+.1} dB", v)
    } else {
        format!("< {:.0} dB", app.db_floor)
    }
}

fn draw_status(f: &mut ratatui::Frame, area: Rect, app: &App, mode: &UiMode) {
    if app.fullscreen { return; }
    let mut lines = vec![
        Line::from(vec![
            Span::raw("[q] quit  [p] pause  [a] style  [b] bins  [+/-] zoom  [[/]] floor  [c/C] palette  [s/S] png  [w/W] csv  [r] reset  [f] fullscreen  [d] details  [o] overview  [h] help"),
        ]),
    ];
    let f_max = (app.settings.sample_rate as f32) / 2.0 / app.zoom;
    let seconds = (app.buffer.len() as f32) * (app.settings.hop_size as f32) / (app.settings.sample_rate as f32);
    lines.push(Line::from(Span::raw(format!(
        "src: {} | style: {:?} | zoom: {:.2} | floor: {:.1} dB ceil: {:.1} | rows: {} | freq: 0..{:.0} Hz | time: 0..{:.2}s | L/H/N: {}/{}/{} | fps: {} | rps: {:.1} | rt: {} | scale: {:?} | render: {:?} | bins: {:?}",
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
        app.render_mode,
        app.bins_mode
    ))));
    // Third line is always present (possibly blank) so the layout is stable.
    // Priority: prompt > recent status (action feedback) > sticky input error.
    if let UiMode::PromptSave { kind, input } = mode {
        let title = match kind { SaveKind::Png => "PNG path:", SaveKind::Csv => "CSV path:" };
        lines.push(Line::from(Span::raw(format!("{} {}", title, input))));
    } else if let Some(msg) = app.current_status() {
        lines.push(Line::from(Span::raw(msg.to_string())));
    } else if let Some(err) = &app.error {
        lines.push(Line::from(Span::styled(
            err.clone(),
            Style::default().add_modifier(ratatui::style::Modifier::REVERSED),
        )));
    } else {
        lines.push(Line::from(Span::raw("")));
    }
    let p = Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("status"));
    f.render_widget(p, area);
}

fn format_freq(hz: f32) -> String {
    if hz >= 10_000.0 {
        format!("{:.1}kHz", hz / 1000.0)
    } else if hz >= 1_000.0 {
        format!("{:.2}kHz", hz / 1000.0)
    } else {
        format!("{:.0}Hz", hz)
    }
}

fn draw_overlay(f: &mut ratatui::Frame, area: Rect, app: &App, _mode: &UiMode) {
    if area.width < 12 || area.height < 4 { return; }
    let m = fmap(app);
    match app.style {
        AnimationStyle::Spectrum => {
            // Spectrum view: y axis is dB, x axis is frequency
            let ticks = (area.height as usize / 6).clamp(3, 8);
            for i in 0..=ticks {
                let y = area.y + (i as u16) * (area.height.saturating_sub(1)) / (ticks as u16);
                let frac = (ticks - i) as f32 / ticks as f32;
                let db = app.db_floor + frac * (app.db_ceiling - app.db_floor);
                let label = format!("{:.0}dB", db);
                let r = Rect { x: area.x, y, width: (label.len() as u16).min(area.width), height: 1 };
                f.render_widget(Paragraph::new(label), r);
            }
            // Frequency labels along the bottom edge
            let fticks = (area.width as usize / 20).clamp(2, 8);
            for i in 0..=fticks {
                let frac = i as f32 / fticks as f32;
                let label = format_freq(m.frac_to_freq(frac));
                let lw = label.len() as u16;
                let x_pos = area.x + (i as u16) * (area.width.saturating_sub(1)) / (fticks as u16);
                let x_pos = x_pos.min(area.x + area.width.saturating_sub(lw));
                let r = Rect { x: x_pos, y: area.y + area.height - 1, width: lw.min(area.width), height: 1 };
                f.render_widget(Paragraph::new(label), r);
            }
        }
        _ => {
            let horizontal = matches!(app.style, AnimationStyle::Horizontal);
            if horizontal {
                // Frequency on the y axis (low at bottom)
                let ticks = (area.height as usize / 6).clamp(4, 12);
                for i in 0..=ticks {
                    let y = area.y + (i as u16) * (area.height.saturating_sub(1)) / (ticks as u16);
                    let frac = (ticks - i) as f32 / ticks as f32;
                    let label = format_freq(m.frac_to_freq(frac));
                    let r = Rect { x: area.x, y, width: (label.len() as u16).min(area.width), height: 1 };
                    f.render_widget(Paragraph::new(label), r);
                }
            } else {
                // Waterfall: frequency on the x axis (low at left)
                let fticks = (area.width as usize / 20).clamp(2, 8);
                for i in 0..=fticks {
                    let frac = i as f32 / fticks as f32;
                    let label = format_freq(m.frac_to_freq(frac));
                    let lw = label.len() as u16;
                    let x_pos = area.x + (i as u16) * (area.width.saturating_sub(1)) / (fticks as u16);
                    let x_pos = x_pos.min(area.x + area.width.saturating_sub(lw));
                    let r = Rect { x: x_pos, y: area.y + area.height - 1, width: lw.min(area.width), height: 1 };
                    f.render_widget(Paragraph::new(label), r);
                }
            }
        }
    }
    if !matches!(app.style, AnimationStyle::Spectrum) {
        draw_colorbar(f, area, app);
    }
    // Metadata panel (top-right); clamp to the pane so tiny terminals
    // (e.g. fullscreen on a 5-row window) can't render outside the buffer
    let panel_w = area.width.min(52);
    let panel_h = 7u16.min(area.height);
    let px = area.x + area.width.saturating_sub(panel_w);
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
        Line::from(format!("throughput: {:.1} rows/s | RTF: {:.2}x", rps, rtf)),
        Line::from(format!("frames: vis {} | rows: {} | total: {:.2}s", app.buffer.len(), app.total_rows, total_time)),
        Line::from(format!("scale: {:?} | render: {:?}", app.freq_scale, app.render_mode)),
    ];
    let p = Paragraph::new(meta).block(Block::default().borders(Borders::ALL).title("details"));
    let rect = Rect { x: px, y: py, width: panel_w, height: panel_h };
    f.render_widget(p, rect);
}

fn draw_colorbar(f: &mut ratatui::Frame, area: Rect, app: &App) {
    // Vertical dB reference bar on the right edge, below the details panel,
    // so on-screen colors can be read back as absolute dB values.
    let top = area.y + 8; // details panel occupies the top 7 rows
    let bottom = area.y + area.height.saturating_sub(1);
    if bottom <= top + 4 || area.width < 12 { return; }
    let h = bottom - top;
    let bar_x = area.x + area.width.saturating_sub(2);

    let title = Paragraph::new("dB");
    f.render_widget(title, Rect { x: bar_x, y: top - 1, width: 2, height: 1 });
    for i in 0..h {
        let t = 1.0 - (i as f32) / ((h.max(2) - 1) as f32);
        let color = app.palette.color_at(t);
        let p = Paragraph::new(Line::from(Span::styled("██", Style::default().fg(color))));
        f.render_widget(p, Rect { x: bar_x, y: top + i, width: 2, height: 1 });
    }
    let label_ticks = 4u16;
    for i in 0..=label_ticks {
        let y = top + i * (h - 1) / label_ticks;
        let tfrac = 1.0 - (i as f32) / (label_ticks as f32);
        let db = app.db_floor + tfrac * (app.db_ceiling - app.db_floor);
        let label = format!("{:>4.0}", db);
        let w = label.len() as u16;
        if bar_x <= area.x + w { continue; }
        let p = Paragraph::new(label);
        f.render_widget(p, Rect { x: bar_x - w - 1, y, width: w, height: 1 });
    }
}

// help overlay
fn draw_help(f: &mut ratatui::Frame, area: Rect) {
    let lines = vec![
        Line::from("Usage: sgram-tui [mic|wav|render|FILE] [FILE] [flags]"),
        Line::from("Examples: sgram-tui song.mp3  |  sgram-tui mic  |  sgram-tui render song.wav"),
        Line::from("Keys: q/Esc quit, p pause, a style (waterfall/horizontal/spectrum), b bins, +/- zoom, [[/]] floor, c/C palette,"),
        Line::from("      r reset, f fullscreen, o overview, d details, s/S png, w/W csv, h help. Hover mouse for freq/dB readout."),
    ];
    let p = Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Help"));
    let w = area.width.min(112);
    let h = 6u16;
    if area.width < 10 || area.height < h { return; }
    let x = area.x + (area.width - w) / 2;
    let y = area.y + (area.height - h) / 2;
    f.render_widget(p, Rect { x, y, width: w, height: h });
}
