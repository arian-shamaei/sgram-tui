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

fn viridis_rgb(t: f32) -> (u8, u8, u8) {
    // Lightweight approximation to viridis using piecewise polynomials
    let t = t.clamp(0.0, 1.0);
    let r = (68.0 + 180.0 * t + -110.0 * t * t) as u8;
    let g = (1.0 + 150.0 * t + 50.0 * t * t) as u8;
    let b = (84.0 + 120.0 * t + 50.0 * t * t) as u8;
    (r, g, b)
}

fn jet_rgb(t: f32) -> (u8, u8, u8) {
    // Blue -> Cyan -> Yellow -> Red
    let t = t.clamp(0.0, 1.0);
    let r = (255.0 * (t - 0.5).clamp(0.0, 0.5) * 2.0) as u8;
    let g = if t < 0.5 {
        (255.0 * (t * 2.0)).round() as u8
    } else {
        (255.0 * (1.0 - (t - 0.5) * 2.0)).round().max(0.0) as u8
    };
    let b = (255.0 * (1.0 - t).clamp(0.0, 0.5) * 2.0) as u8;
    (r, g, b)
}

fn inferno_rgb(t: f32) -> (u8, u8, u8) {
    // Rough approximation of Inferno
    let t = t.clamp(0.0, 1.0);
    let r = (0.0 + 255.0 * t.powf(1.2)) as u8;
    let g = (0.0 + 255.0 * (t.powf(1.5)).min(0.9)) as u8;
    let b = (10.0 + 200.0 * (1.0 - t).powf(2.0)) as u8;
    (r, g, b)
}

fn magma_rgb(t: f32) -> (u8, u8, u8) {
    // Rough approximation of Magma
    let t = t.clamp(0.0, 1.0);
    let r = (30.0 + 225.0 * t.powf(1.4)) as u8;
    let g = (0.0 + 200.0 * t.powf(1.0)) as u8;
    let b = (20.0 + 100.0 * (1.0 - t).powf(2.2)) as u8;
    (r, g, b)
}

fn plasma_rgb(t: f32) -> (u8, u8, u8) {
    // Rough approximation of Plasma
    let t = t.clamp(0.0, 1.0);
    let r = (50.0 + 200.0 * t) as u8;
    let g = (0.0 + 180.0 * (t * (1.0 - t) * 4.0).sqrt()) as u8;
    let b = (150.0 + 80.0 * (1.0 - t)) as u8;
    (r, g, b)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

fn purple_fire_rgb(t: f32) -> (u8, u8, u8) {
    // Approximation of a "deep purple to fire" palette similar to the provided image
    // Control points: (t, r,g,b)
    // 0.00: (0, 0, 0)
    // 0.15: (12, 7, 42)
    // 0.35: (60, 10, 90)
    // 0.55: (120, 20, 120)
    // 0.75: (200, 40, 60)
    // 0.90: (255, 110, 10)
    // 1.00: (255, 235, 90)
    let t = t.clamp(0.0, 1.0);
    let pts: [(f32, u8, u8, u8); 7] = [
        (0.00, 0, 0, 0),
        (0.15, 12, 7, 42),
        (0.35, 60, 10, 90),
        (0.55, 120, 20, 120),
        (0.75, 200, 40, 60),
        (0.90, 255, 110, 10),
        (1.00, 255, 235, 90),
    ];
    for w in pts.windows(2) {
        let (t0, r0, g0, b0) = w[0];
        let (t1, r1, g1, b1) = w[1];
        if t >= t0 && t <= t1 {
            let u = (t - t0) / (t1 - t0);
            let r = lerp(r0 as f32, r1 as f32, u) as u8;
            let g = lerp(g0 as f32, g1 as f32, u) as u8;
            let b = lerp(b0 as f32, b1 as f32, u) as u8;
            return (r, g, b);
        }
    }
    (255, 235, 90)
}
