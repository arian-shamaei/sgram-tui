use crate::app::{AnimationStyle, BinsMode, FreqScale};
use crate::colors::Palette;
use crate::font;
use crate::view::{self, FreqMap};
use anyhow::Result;
use image::{ImageBuffer, Rgb};
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;

pub struct PngRequest<'a> {
    pub buffer: &'a VecDeque<Vec<f32>>,
    pub palette: &'a Palette,
    pub db_floor: f32,
    pub db_ceiling: f32,
    /// Content (spectrogram) size in pixels; axis margins are added on top
    /// when the content is large enough for a labeled figure.
    pub width: u32,
    pub height: u32,
    pub style: AnimationStyle,
    pub freq_scale: FreqScale,
    pub sample_rate: u32,
    pub zoom: f32,
    pub bins_mode: BinsMode,
    /// Hop size in samples; used to label the time axis.
    pub hop: usize,
    pub title: Option<String>,
}

type Img = ImageBuffer<Rgb<u8>, Vec<u8>>;

// Figure layout (font scale 2 -> 10x14 px glyphs)
const AXES_MIN_W: u32 = 320;
const AXES_MIN_H: u32 = 240;
const ML: u32 = 92; // left margin: y-axis labels
const MR: u32 = 108; // right margin: colorbar + labels
const MT: u32 = 34; // top margin: title
const MB: u32 = 44; // bottom margin: x-axis labels
const FSC: u32 = 2; // font scale
const TICK: u32 = 5; // tick mark length

const BG: Rgb<u8> = Rgb([16, 16, 20]);
const FG: Rgb<u8> = Rgb([208, 208, 214]);
const DIM: Rgb<u8> = Rgb([95, 95, 105]);

fn rgb_of(palette: &Palette, t: f32) -> Rgb<u8> {
    match palette.color_at(t) {
        ratatui::style::Color::Rgb(r, g, b) => Rgb([r, g, b]),
        _ => Rgb([0, 0, 0]),
    }
}

fn freq_label(hz: f32) -> String {
    if hz >= 9999.5 {
        format!("{:.0}kHz", hz / 1000.0)
    } else if hz >= 999.5 {
        format!("{:.1}kHz", hz / 1000.0)
    } else {
        format!("{:.0}Hz", hz)
    }
}

fn time_label(sec_ago: f32) -> String {
    if sec_ago <= 0.005 {
        "0s".to_string()
    } else if sec_ago < 10.0 {
        format!("-{:.2}s", sec_ago)
    } else {
        format!("-{:.1}s", sec_ago)
    }
}

fn hline(img: &mut Img, x0: u32, x1: u32, y: u32, c: Rgb<u8>) {
    if y >= img.height() { return; }
    for x in x0..x1.min(img.width()) {
        img.put_pixel(x, y, c);
    }
}

fn vline(img: &mut Img, x: u32, y0: u32, y1: u32, c: Rgb<u8>) {
    if x >= img.width() { return; }
    for y in y0..y1.min(img.height()) {
        img.put_pixel(x, y, c);
    }
}

pub fn save_png(req: &PngRequest, path: PathBuf) -> Result<()> {
    if req.buffer.is_empty() {
        return Err(anyhow::anyhow!("nothing to export: history is empty"));
    }
    let cw = req.width.max(1);
    let ch = req.height.max(1);
    let axes = cw >= AXES_MIN_W && ch >= AXES_MIN_H;
    let (iw, ih) = if axes { (cw + ML + MR, ch + MT + MB) } else { (cw, ch) };
    let (ox, oy) = if axes { (ML, MT) } else { (0, 0) };
    let mut img: Img = ImageBuffer::from_pixel(iw, ih, BG);

    let bins = req.buffer.front().map(|r| r.len()).unwrap_or(1).max(1);
    let total = req.buffer.len();
    let fmap = FreqMap { sample_rate: req.sample_rate, zoom: req.zoom, scale: req.freq_scale };
    let range = (req.db_ceiling - req.db_floor).max(1.0);
    let total_sec = (total as f32) * (req.hop as f32) / (req.sample_rate as f32).max(1.0);

    // ---- content: the full history is always fitted to the content rect,
    // max-pooling every covered bin/row so narrow features survive ----
    match req.style {
        AnimationStyle::Waterfall | AnimationStyle::Spectrum => {
            // y = time (newest at top), x = frequency (low -> high)
            for py in 0..ch {
                let (r0, r1) = view::overview_row_range(py as usize, ch as usize, total);
                for px in 0..cw {
                    let (lo, hi) = fmap.cell_bin_range(px as usize, cw as usize, bins);
                    let v = view::pool_cell(req.buffer, r0, r1, lo, hi, req.bins_mode);
                    let t = ((v - req.db_floor) / range).clamp(0.0, 1.0);
                    img.put_pixel(ox + px, oy + py, rgb_of(req.palette, t));
                }
            }
        }
        AnimationStyle::Horizontal => {
            // x = time (oldest -> newest), y = frequency (low at bottom)
            for px in 0..cw {
                // oldest-based index range covered by this pixel column
                let t0 = (px as usize) * total / (cw as usize);
                let t1 = (((px as usize + 1) * total).div_ceil(cw as usize)).clamp(t0 + 1, total);
                // buffer stores newest at index 0
                let r0 = total - t1;
                let r1 = total - t0;
                for py in 0..ch {
                    let fy = (ch - 1 - py) as usize; // low freq at bottom
                    let (lo, hi) = fmap.cell_bin_range(fy, ch as usize, bins);
                    let v = view::pool_cell(req.buffer, r0, r1, lo, hi, req.bins_mode);
                    let t = ((v - req.db_floor) / range).clamp(0.0, 1.0);
                    img.put_pixel(ox + px, oy + py, rgb_of(req.palette, t));
                }
            }
        }
    }

    if axes {
        draw_axes(&mut img, req, &fmap, ox, oy, cw, ch, total_sec);
    }

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }
    img.save(path)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn draw_axes(
    img: &mut Img,
    req: &PngRequest,
    fmap: &FreqMap,
    ox: u32,
    oy: u32,
    cw: u32,
    ch: u32,
    total_sec: f32,
) {
    let glyph_h = font::text_height(FSC);

    // frame around content
    hline(img, ox - 1, ox + cw + 1, oy - 1, DIM);
    hline(img, ox - 1, ox + cw + 1, oy + ch, DIM);
    vline(img, ox - 1, oy - 1, oy + ch + 1, DIM);
    vline(img, ox + cw, oy - 1, oy + ch + 1, DIM);

    // title
    if let Some(title) = &req.title {
        font::draw_text(img, 8, ((MT - glyph_h) / 2) as i64, title, FG, FSC);
    }

    let freq_on_x = !matches!(req.style, AnimationStyle::Horizontal);

    // frequency axis (ticks follow the active scale via frac_to_freq)
    let fticks = 6u32;
    for i in 0..=fticks {
        let frac = i as f32 / fticks as f32;
        let label = freq_label(fmap.frac_to_freq(frac));
        if freq_on_x {
            let x = ox + ((frac * (cw - 1) as f32) as u32);
            vline(img, x, oy + ch, oy + ch + TICK, FG);
            let lw = font::text_width(&label, FSC) as i64;
            font::draw_text(
                img,
                (x as i64 - lw / 2).max(2),
                (oy + ch + TICK + 3) as i64,
                &label,
                FG,
                FSC,
            );
        } else {
            let y = oy + (((1.0 - frac) * (ch - 1) as f32) as u32);
            hline(img, ox - TICK, ox, y, FG);
            let lw = font::text_width(&label, FSC) as i64;
            font::draw_text(
                img,
                (ox - TICK) as i64 - lw - 4,
                y as i64 - (glyph_h / 2) as i64,
                &label,
                FG,
                FSC,
            );
        }
    }

    // time axis (0 = newest row)
    let tticks = 4u32;
    for i in 0..=tticks {
        let frac = i as f32 / tticks as f32;
        if freq_on_x {
            // waterfall: time runs down the y axis, newest at top
            let y = oy + ((frac * (ch - 1) as f32) as u32);
            let label = time_label(frac * total_sec);
            hline(img, ox - TICK, ox, y, FG);
            let lw = font::text_width(&label, FSC) as i64;
            font::draw_text(
                img,
                (ox - TICK) as i64 - lw - 4,
                y as i64 - (glyph_h / 2) as i64,
                &label,
                FG,
                FSC,
            );
        } else {
            // horizontal: time runs along x, newest at the right edge
            let x = ox + ((frac * (cw - 1) as f32) as u32);
            let label = time_label((1.0 - frac) * total_sec);
            vline(img, x, oy + ch, oy + ch + TICK, FG);
            let lw = font::text_width(&label, FSC) as i64;
            font::draw_text(
                img,
                (x as i64 - lw / 2).max(2),
                (oy + ch + TICK + 3) as i64,
                &label,
                FG,
                FSC,
            );
        }
    }

    // colorbar: absolute dB reference for the palette
    let bar_x = ox + cw + 22;
    let bar_w = 16u32;
    font::draw_text(img, bar_x as i64, (oy as i64) - (glyph_h as i64) - 6, "dB", FG, FSC);
    for py in 0..ch {
        let t = 1.0 - (py as f32) / ((ch - 1).max(1) as f32);
        let c = rgb_of(req.palette, t);
        for x in bar_x..bar_x + bar_w {
            if x < img.width() && oy + py < img.height() {
                img.put_pixel(x, oy + py, c);
            }
        }
    }
    vline(img, bar_x - 1, oy, oy + ch, DIM);
    vline(img, bar_x + bar_w, oy, oy + ch, DIM);
    let dticks = 4u32;
    for i in 0..=dticks {
        let frac = i as f32 / dticks as f32;
        let y = oy + ((frac * (ch - 1) as f32) as u32);
        let db = req.db_ceiling - frac * (req.db_ceiling - req.db_floor);
        let label = format!("{:.0}", db);
        hline(img, bar_x + bar_w, bar_x + bar_w + TICK, y, FG);
        font::draw_text(
            img,
            (bar_x + bar_w + TICK + 3) as i64,
            y as i64 - (glyph_h / 2) as i64,
            &label,
            FG,
            FSC,
        );
    }
}

pub fn save_csv(buffer: &VecDeque<Vec<f32>>, path: PathBuf) -> Result<()> {
    if buffer.is_empty() {
        return Err(anyhow::anyhow!("nothing to export: history is empty"));
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }
    let mut wtr = csv::Writer::from_path(path)?;
    for row in buffer.iter().rev() {
        // oldest to newest
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

    fn req<'a>(
        buffer: &'a VecDeque<Vec<f32>>,
        palette: &'a Palette,
        width: u32,
        height: u32,
    ) -> PngRequest<'a> {
        PngRequest {
            buffer,
            palette,
            db_floor: -80.0,
            db_ceiling: 0.0,
            width,
            height,
            style: AnimationStyle::Waterfall,
            freq_scale: FreqScale::Linear,
            sample_rate: 48000,
            zoom: 1.0,
            bins_mode: BinsMode::All,
            hop: 256,
            title: Some("fs=48000Hz N=1024".to_string()),
        }
    }

    #[test]
    fn csv_writes_rows_oldest_first() {
        let mut buf: VecDeque<Vec<f32>> = VecDeque::new();
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
    fn png_small_raw_creates_nonempty_file() {
        let mut buf: VecDeque<Vec<f32>> = VecDeque::new();
        buf.push_front(vec![-80.0, 0.0]);
        let path = tmp_path("png_raw").with_extension("png");
        let palette = Palette::viridis();
        save_png(&req(&buf, &palette, 64, 32), path.clone()).unwrap();
        let meta = std::fs::metadata(&path).unwrap();
        assert!(meta.len() > 0);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn png_figure_has_margins_and_labels() {
        let mut buf: VecDeque<Vec<f32>> = VecDeque::new();
        for i in 0..64 {
            let mut row = vec![-80.0f32; 128];
            row[i * 2] = -5.0;
            buf.push_front(row);
        }
        let path = tmp_path("png_fig").with_extension("png");
        let palette = Palette::viridis();
        save_png(&req(&buf, &palette, 480, 320), path.clone()).unwrap();
        let img = image::open(&path).unwrap().to_rgb8();
        assert_eq!(img.width(), 480 + ML + MR);
        assert_eq!(img.height(), 320 + MT + MB);
        // some label pixels must be lit in the left margin
        let lit = (0..ML)
            .flat_map(|x| (MT..MT + 320).map(move |y| (x, y)))
            .filter(|&(x, y)| img.get_pixel(x, y).0[0] > 100)
            .count();
        assert!(lit > 20, "expected axis labels in left margin, lit={lit}");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn png_narrow_peak_survives_pooling() {
        // 1 loud bin out of 2048 must still be visible in a 480px-wide export
        let mut row = vec![-80.0f32; 2048];
        row[1234] = -2.0;
        let mut buf: VecDeque<Vec<f32>> = VecDeque::new();
        buf.push_front(row);
        let path = tmp_path("png_peak").with_extension("png");
        let palette = Palette::grayscale();
        save_png(&req(&buf, &palette, 480, 320), path.clone()).unwrap();
        let img = image::open(&path).unwrap().to_rgb8();
        // scan the content row for a bright pixel
        let bright = (0..480).filter(|&px| img.get_pixel(ML + px, MT + 10).0[0] > 200).count();
        assert!(bright >= 1, "narrow peak lost in export");
        let _ = std::fs::remove_file(path);
    }
}
