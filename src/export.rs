use crate::colors::Palette;
use anyhow::Result;
use image::{ImageBuffer, Rgb};
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;

pub fn save_png(buffer: &VecDeque<Vec<f32>>, palette: &Palette, db_floor: f32, db_ceiling: f32, width: u32, height: u32, path: PathBuf) -> Result<()> {
    if buffer.is_empty() { return Ok(()); }
    let w = width.max(1);
    let h = height.max(1);
    let mut img = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(w, h);
    let bins = buffer.front().map(|r| r.len()).unwrap_or(1).max(1);
    for y in 0..h {
        let src_row_idx = y as usize;
        let src = buffer.get(src_row_idx).unwrap_or_else(|| buffer.back().unwrap());
        for x in 0..w {
            let bin_idx = ((x as f32) / (w as f32) * (bins as f32)) as usize;
            let db = *src.get(bin_idx.min(bins - 1)).unwrap_or(&db_floor);
            let t = ((db - db_floor) / (db_ceiling - db_floor).max(1.0)).clamp(0.0, 1.0);
            let c = palette.color_at(t);
            let (r, g, b) = match c { ratatui::style::Color::Rgb(r, g, b) => (r, g, b), _ => (0, 0, 0) };
            img.put_pixel(x, y, Rgb([r, g, b]));
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
