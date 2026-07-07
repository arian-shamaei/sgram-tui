//! Minimal embedded 5x7 bitmap font for labeling PNG exports without pulling
//! in a font-rendering dependency. Column-major, LSB = top pixel. Glyph set
//! covers axis/metadata labels (digits, units, a few letters); unknown
//! characters render as blanks.

use image::{ImageBuffer, Rgb};

const GLYPH_W: u32 = 5;
const GLYPH_H: u32 = 7;

fn glyph(c: char) -> Option<[u8; 5]> {
    Some(match c {
        '0' => [0x3E, 0x51, 0x49, 0x45, 0x3E],
        '1' => [0x00, 0x42, 0x7F, 0x40, 0x00],
        '2' => [0x42, 0x61, 0x51, 0x49, 0x46],
        '3' => [0x21, 0x41, 0x45, 0x4B, 0x31],
        '4' => [0x18, 0x14, 0x12, 0x7F, 0x10],
        '5' => [0x27, 0x45, 0x45, 0x45, 0x39],
        '6' => [0x3C, 0x4A, 0x49, 0x49, 0x30],
        '7' => [0x01, 0x71, 0x09, 0x05, 0x03],
        '8' => [0x36, 0x49, 0x49, 0x49, 0x36],
        '9' => [0x06, 0x49, 0x49, 0x29, 0x1E],
        '-' => [0x08, 0x08, 0x08, 0x08, 0x08],
        '+' => [0x08, 0x08, 0x3E, 0x08, 0x08],
        '.' => [0x00, 0x60, 0x60, 0x00, 0x00],
        ':' => [0x00, 0x36, 0x36, 0x00, 0x00],
        '=' => [0x14, 0x14, 0x14, 0x14, 0x14],
        '/' => [0x20, 0x10, 0x08, 0x04, 0x02],
        ' ' => [0x00, 0x00, 0x00, 0x00, 0x00],
        'B' => [0x7F, 0x49, 0x49, 0x49, 0x36],
        'H' => [0x7F, 0x08, 0x08, 0x08, 0x7F],
        'L' => [0x7F, 0x40, 0x40, 0x40, 0x40],
        'N' => [0x7F, 0x04, 0x08, 0x10, 0x7F],
        'S' => [0x46, 0x49, 0x49, 0x49, 0x31],
        'W' => [0x3F, 0x40, 0x38, 0x40, 0x3F],
        'a' => [0x20, 0x54, 0x54, 0x54, 0x78],
        'b' => [0x7F, 0x48, 0x44, 0x44, 0x38],
        'c' => [0x38, 0x44, 0x44, 0x44, 0x20],
        'd' => [0x38, 0x44, 0x44, 0x48, 0x7F],
        'e' => [0x38, 0x54, 0x54, 0x54, 0x18],
        'f' => [0x08, 0x7E, 0x09, 0x01, 0x02],
        'g' => [0x0C, 0x52, 0x52, 0x52, 0x3E],
        'h' => [0x7F, 0x08, 0x04, 0x04, 0x78],
        'i' => [0x00, 0x44, 0x7D, 0x40, 0x00],
        'k' => [0x7F, 0x10, 0x28, 0x44, 0x00],
        'l' => [0x00, 0x41, 0x7F, 0x40, 0x00],
        'm' => [0x7C, 0x04, 0x18, 0x04, 0x78],
        'n' => [0x7C, 0x08, 0x04, 0x04, 0x78],
        'o' => [0x38, 0x44, 0x44, 0x44, 0x38],
        'p' => [0x7C, 0x14, 0x14, 0x14, 0x08],
        'r' => [0x7C, 0x08, 0x04, 0x04, 0x08],
        's' => [0x48, 0x54, 0x54, 0x54, 0x20],
        't' => [0x04, 0x3F, 0x44, 0x40, 0x20],
        'u' => [0x3C, 0x40, 0x40, 0x20, 0x7C],
        'v' => [0x1C, 0x20, 0x40, 0x20, 0x1C],
        'w' => [0x3C, 0x40, 0x30, 0x40, 0x3C],
        'x' => [0x44, 0x28, 0x10, 0x28, 0x44],
        'z' => [0x44, 0x64, 0x54, 0x4C, 0x44],
        _ => return None,
    })
}

/// Pixel width of `text` at integer scale `scale` (1 px inter-glyph gap).
pub fn text_width(text: &str, scale: u32) -> u32 {
    (text.chars().count() as u32) * (GLYPH_W + 1) * scale
}

pub fn text_height(scale: u32) -> u32 {
    GLYPH_H * scale
}

/// Draw `text` with its top-left corner at (x, y). Out-of-bounds pixels are
/// clipped. Unknown glyphs advance the cursor but draw nothing.
pub fn draw_text(
    img: &mut ImageBuffer<Rgb<u8>, Vec<u8>>,
    x: i64,
    y: i64,
    text: &str,
    color: Rgb<u8>,
    scale: u32,
) {
    let scale = scale.max(1);
    let (iw, ih) = (img.width() as i64, img.height() as i64);
    let mut cx = x;
    for c in text.chars() {
        if let Some(cols) = glyph(c) {
            for (gx, col) in cols.iter().enumerate() {
                for gy in 0..GLYPH_H {
                    if col >> gy & 1 == 1 {
                        for sx in 0..scale {
                            for sy in 0..scale {
                                let px = cx + (gx as u32 * scale + sx) as i64;
                                let py = y + (gy * scale + sy) as i64;
                                if px >= 0 && py >= 0 && px < iw && py < ih {
                                    img.put_pixel(px as u32, py as u32, color);
                                }
                            }
                        }
                    }
                }
            }
        }
        cx += ((GLYPH_W + 1) * scale) as i64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_visible_pixels_and_clips() {
        let mut img = ImageBuffer::from_pixel(64, 16, Rgb([0u8, 0, 0]));
        draw_text(&mut img, 1, 1, "8.k", Rgb([255, 255, 255]), 1);
        let lit = img.pixels().filter(|p| p.0[0] > 0).count();
        assert!(lit > 10, "expected glyph pixels, got {lit}");
        // Clipping must not panic
        draw_text(&mut img, -3, -3, "999", Rgb([255, 255, 255]), 2);
        draw_text(&mut img, 60, 14, "999", Rgb([255, 255, 255]), 2);
    }

    #[test]
    fn width_accounts_for_all_chars() {
        assert_eq!(text_width("123", 1), 18);
        assert_eq!(text_width("123", 2), 36);
    }
}
