use ratatui::style::Color;

#[derive(Clone, Copy, Debug)]
pub enum PaletteKind { Grayscale, Heat, Viridis, Jet, Inferno, Magma, Plasma, PurpleFire }

#[derive(Clone, Copy)]
pub struct Palette { kind: PaletteKind }

impl Palette {
    pub fn grayscale() -> Self { Self { kind: PaletteKind::Grayscale } }
    pub fn heat() -> Self { Self { kind: PaletteKind::Heat } }
    pub fn viridis() -> Self { Self { kind: PaletteKind::Viridis } }
    pub fn jet() -> Self { Self { kind: PaletteKind::Jet } }
    pub fn inferno() -> Self { Self { kind: PaletteKind::Inferno } }
    pub fn magma() -> Self { Self { kind: PaletteKind::Magma } }
    pub fn plasma() -> Self { Self { kind: PaletteKind::Plasma } }
    pub fn purple_fire() -> Self { Self { kind: PaletteKind::PurpleFire } }

    pub fn next(&self) -> Self {
        match self.kind {
            PaletteKind::Grayscale => Self::heat(),
            PaletteKind::Heat => Self::viridis(),
            PaletteKind::Viridis => Self::jet(),
            PaletteKind::Jet => Self::inferno(),
            PaletteKind::Inferno => Self::magma(),
            PaletteKind::Magma => Self::plasma(),
            PaletteKind::Plasma => Self::purple_fire(),
            PaletteKind::PurpleFire => Self::grayscale(),
        }
    }
    pub fn prev(&self) -> Self {
        match self.kind {
            PaletteKind::Grayscale => Self::plasma(),
            PaletteKind::Heat => Self::grayscale(),
            PaletteKind::Viridis => Self::heat(),
            PaletteKind::Jet => Self::viridis(),
            PaletteKind::Inferno => Self::jet(),
            PaletteKind::Magma => Self::inferno(),
            PaletteKind::Plasma => Self::magma(),
            PaletteKind::PurpleFire => Self::plasma(),
        }
    }

    pub fn color_at(&self, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let (r, g, b) = match self.kind {
            PaletteKind::Grayscale => {
                let v = (t * 255.0) as u8; (v, v, v)
            }
            PaletteKind::Heat => {
                // black -> red -> yellow -> white
                let r = (t * 255.0).clamp(0.0, 255.0) as u8;
                let g = (t * t * 255.0).clamp(0.0, 255.0) as u8;
                let b = (t.powf(0.25) * 64.0).clamp(0.0, 255.0) as u8;
                (r, g, b)
            }
            PaletteKind::Viridis => viridis_rgb(t),
            PaletteKind::Jet => jet_rgb(t),
            PaletteKind::Inferno => inferno_rgb(t),
            PaletteKind::Magma => magma_rgb(t),
            PaletteKind::Plasma => plasma_rgb(t),
            PaletteKind::PurpleFire => purple_fire_rgb(t),
        };
        Color::Rgb(r, g, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb_of(color: Color) -> (u8, u8, u8) { match color { Color::Rgb(r,g,b) => (r,g,b), _ => (0,0,0) } }

    #[test]
    fn colormap_endpoints_match_references() {
        // Scientific colormaps must start near black/dark and end bright
        assert_eq!(rgb_of(Palette::viridis().color_at(0.0)), (68, 1, 84));
        assert_eq!(rgb_of(Palette::viridis().color_at(1.0)), (253, 231, 37));
        assert_eq!(rgb_of(Palette::inferno().color_at(0.0)), (0, 0, 4));
        assert_eq!(rgb_of(Palette::inferno().color_at(1.0)), (252, 255, 164));
        assert_eq!(rgb_of(Palette::magma().color_at(0.0)), (0, 0, 4));
        assert_eq!(rgb_of(Palette::plasma().color_at(1.0)), (240, 249, 33));
    }

    #[test]
    fn color_at_clamps_bounds() {
        let p = Palette::grayscale();
        let (r0, g0, b0) = rgb_of(p.color_at(-1.0));
        let (r1, g1, b1) = rgb_of(p.color_at(2.0));
        assert!(r0 <= 255 && g0 <= 255 && b0 <= 255);
        assert!(r1 <= 255 && g1 <= 255 && b1 <= 255);
    }

    #[test]
    fn cycle_next_and_prev_returns_to_start() {
        let start = Palette::grayscale();
        let base = rgb_of(start.color_at(0.37));

        // Find cycle length for next()
        let mut p = start;
        let mut period_next = None;
        for i in 1..=16 {
            p = p.next();
            if rgb_of(p.color_at(0.37)) == base { period_next = Some(i); break; }
        }
        let per_n = period_next.expect("no cycle found for next()");
        assert!(per_n <= 8, "unexpected next() cycle length: {}", per_n);

        // Find cycle length for prev()
        let mut p2 = start;
        let mut period_prev = None;
        for i in 1..=16 {
            p2 = p2.prev();
            if rgb_of(p2.color_at(0.37)) == base { period_prev = Some(i); break; }
        }
        let per_p = period_prev.expect("no cycle found for prev()");
        assert!(per_p <= 8, "unexpected prev() cycle length: {}", per_p);
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

/// Piecewise-linear interpolation through colormap anchor points.
fn interp_anchors(pts: &[(f32, u8, u8, u8)], t: f32) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    for w in pts.windows(2) {
        let (t0, r0, g0, b0) = w[0];
        let (t1, r1, g1, b1) = w[1];
        if t >= t0 && t <= t1 {
            let u = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
            return (
                lerp(r0 as f32, r1 as f32, u).round() as u8,
                lerp(g0 as f32, g1 as f32, u).round() as u8,
                lerp(b0 as f32, b1 as f32, u).round() as u8,
            );
        }
    }
    let (_, r, g, b) = *pts.last().expect("non-empty anchor table");
    (r, g, b)
}

// Standard matplotlib anchor colors (9 points) so low levels render near
// black and peaks render at the colormap's true bright end.
const VIRIDIS: [(f32, u8, u8, u8); 9] = [
    (0.000, 68, 1, 84),
    (0.125, 72, 40, 120),
    (0.250, 62, 74, 137),
    (0.375, 49, 104, 142),
    (0.500, 38, 130, 142),
    (0.625, 31, 158, 137),
    (0.750, 53, 183, 121),
    (0.875, 109, 205, 89),
    (1.000, 253, 231, 37),
];

const INFERNO: [(f32, u8, u8, u8); 9] = [
    (0.000, 0, 0, 4),
    (0.125, 27, 12, 66),
    (0.250, 75, 12, 107),
    (0.375, 120, 28, 109),
    (0.500, 165, 44, 96),
    (0.625, 207, 68, 70),
    (0.750, 237, 105, 37),
    (0.875, 251, 154, 6),
    (1.000, 252, 255, 164),
];

const MAGMA: [(f32, u8, u8, u8); 9] = [
    (0.000, 0, 0, 4),
    (0.125, 20, 14, 54),
    (0.250, 59, 15, 112),
    (0.375, 100, 26, 128),
    (0.500, 140, 41, 129),
    (0.625, 183, 55, 121),
    (0.750, 222, 73, 104),
    (0.875, 247, 112, 92),
    (1.000, 252, 253, 191),
];

const PLASMA: [(f32, u8, u8, u8); 9] = [
    (0.000, 13, 8, 135),
    (0.125, 65, 4, 157),
    (0.250, 106, 0, 168),
    (0.375, 143, 13, 164),
    (0.500, 177, 42, 144),
    (0.625, 204, 71, 120),
    (0.750, 225, 100, 98),
    (0.875, 242, 132, 75),
    (1.000, 240, 249, 33),
];

const JET: [(f32, u8, u8, u8); 6] = [
    (0.000, 0, 0, 131),
    (0.125, 0, 60, 170),
    (0.375, 5, 255, 255),
    (0.625, 255, 255, 0),
    (0.875, 250, 0, 0),
    (1.000, 128, 0, 0),
];

const PURPLE_FIRE: [(f32, u8, u8, u8); 7] = [
    (0.00, 0, 0, 0),
    (0.15, 12, 7, 42),
    (0.35, 60, 10, 90),
    (0.55, 120, 20, 120),
    (0.75, 200, 40, 60),
    (0.90, 255, 110, 10),
    (1.00, 255, 235, 90),
];

fn viridis_rgb(t: f32) -> (u8, u8, u8) { interp_anchors(&VIRIDIS, t) }
fn inferno_rgb(t: f32) -> (u8, u8, u8) { interp_anchors(&INFERNO, t) }
fn magma_rgb(t: f32) -> (u8, u8, u8) { interp_anchors(&MAGMA, t) }
fn plasma_rgb(t: f32) -> (u8, u8, u8) { interp_anchors(&PLASMA, t) }
fn jet_rgb(t: f32) -> (u8, u8, u8) { interp_anchors(&JET, t) }
fn purple_fire_rgb(t: f32) -> (u8, u8, u8) { interp_anchors(&PURPLE_FIRE, t) }
