//! Shared frequency-axis mapping and cell sampling used by both the terminal
//! renderer (ui.rs) and the PNG exporter (export.rs).
//!
//! Rows in the history buffer are full resolution: `bins` values spanning
//! 0..fs/2. Zoom narrows the *displayed* frequency range only. Each display
//! cell (terminal cell or image pixel) covers a frequency interval; we
//! max-pool over every bin in that interval so narrowband peaks are never
//! skipped when many bins map to one cell.

use crate::app::{BinsMode, FreqScale};
use std::collections::VecDeque;

#[derive(Copy, Clone, Debug)]
pub struct FreqMap {
    pub sample_rate: u32,
    pub zoom: f32,
    pub scale: FreqScale,
}

impl FreqMap {
    /// Displayed frequency at fraction `t` in 0..1 (0 = bottom of range).
    pub fn frac_to_freq(&self, t: f32) -> f32 {
        let fs = self.sample_rate as f32;
        let fmax = fs / 2.0 / self.zoom.max(1.0);
        let fmin = match self.scale { FreqScale::Linear => 0.0, _ => 20.0 };
        match self.scale {
            FreqScale::Linear => t * fmax,
            FreqScale::Log => {
                let a = (fmax / fmin).max(1.01);
                fmin * a.powf(t)
            }
            FreqScale::Mel => {
                let mel = |hz: f32| 2595.0 * (1.0 + hz / 700.0).log10();
                let inv_mel = |m: f32| 700.0 * (10f32.powf(m / 2595.0) - 1.0);
                let mmin = mel(fmin);
                let mmax = mel(fmax);
                inv_mel(mmin + t * (mmax - mmin))
            }
        }
    }

    /// Full-resolution bin range [lo, hi) covered by cell `i` of `n` along the
    /// displayed frequency axis. Never empty; clamped to `bins`.
    pub fn cell_bin_range(&self, i: usize, n: usize, bins: usize) -> (usize, usize) {
        let n = n.max(1);
        let bins = bins.max(1);
        let hz_per_bin = (self.sample_rate as f32 / 2.0) / (bins as f32);
        let f0 = self.frac_to_freq(i as f32 / n as f32);
        let f1 = self.frac_to_freq((i as f32 + 1.0) / n as f32);
        let lo = ((f0 / hz_per_bin).floor() as usize).min(bins - 1);
        let hi = ((f1 / hz_per_bin).ceil() as usize).clamp(lo + 1, bins);
        (lo, hi)
    }
}

/// Index and value of the loudest bin in row[lo..hi].
pub fn max_bin_in(row: &[f32], lo: usize, hi: usize) -> (usize, f32) {
    let hi = hi.min(row.len());
    let mut best_i = lo.min(row.len().saturating_sub(1));
    let mut best_v = f32::NEG_INFINITY;
    for (off, &v) in row.get(lo..hi).unwrap_or(&[]).iter().enumerate() {
        if v > best_v {
            best_v = v;
            best_i = lo + off;
        }
    }
    (best_i, best_v)
}

/// True when bin `i` is a local spectral maximum (>= both frequency neighbours).
pub fn is_local_peak(row: &[f32], i: usize) -> bool {
    if row.is_empty() || i >= row.len() {
        return false;
    }
    let left = if i == 0 { f32::NEG_INFINITY } else { row[i - 1] };
    let right = if i + 1 >= row.len() { f32::NEG_INFINITY } else { row[i + 1] };
    row[i] >= left && row[i] >= right
}

/// Max-pooled dB value for a display cell covering buffer rows [r0, r1) and
/// bins [lo, hi). In Peaks mode, cells whose loudest bin is not a local
/// spectral maximum return NEG_INFINITY (rendered at the floor).
pub fn pool_cell(
    buffer: &VecDeque<Vec<f32>>,
    r0: usize,
    r1: usize,
    lo: usize,
    hi: usize,
    mode: BinsMode,
) -> f32 {
    let mut best_v = f32::NEG_INFINITY;
    let mut best: Option<(usize, usize)> = None;
    for (r, row) in buffer.iter().enumerate().take(r1.min(buffer.len())).skip(r0) {
        let (bi, bv) = max_bin_in(row, lo, hi);
        if bv > best_v {
            best_v = bv;
            best = Some((r, bi));
        }
    }
    match mode {
        BinsMode::All => best_v,
        BinsMode::Peaks => match best {
            Some((r, bi)) if is_local_peak(&buffer[r], bi) => best_v,
            _ => f32::NEG_INFINITY,
        },
    }
}

/// Buffer row range [r0, r1) for display row `y` of `n_rows` when the entire
/// history (`total` rows, newest at index 0) is fitted into the view.
pub fn overview_row_range(y: usize, n_rows: usize, total: usize) -> (usize, usize) {
    let n = n_rows.max(1);
    let r0 = y * total / n;
    let r1 = (((y + 1) * total).div_ceil(n)).clamp(r0 + 1, total.max(1));
    (r0, r1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fm(zoom: f32) -> FreqMap {
        FreqMap { sample_rate: 48_000, zoom, scale: FreqScale::Linear }
    }

    #[test]
    fn cell_ranges_cover_all_bins_without_gaps() {
        // With fewer cells than bins, consecutive cells must tile 0..bins
        let m = fm(1.0);
        let bins = 512;
        let n = 100;
        let mut covered = 0usize;
        for i in 0..n {
            let (lo, hi) = m.cell_bin_range(i, n, bins);
            assert!(lo <= covered, "gap before cell {i}: lo={lo} covered={covered}");
            covered = covered.max(hi);
        }
        assert_eq!(covered, bins, "cells must reach the last bin");
    }

    #[test]
    fn zoom_narrows_range() {
        let m = fm(4.0);
        let (_, hi) = m.cell_bin_range(99, 100, 512);
        // With 4x zoom the last cell should top out near bins/4
        assert!(hi <= 512 / 4 + 2, "hi={hi}");
    }

    #[test]
    fn pooling_finds_narrow_peak() {
        // A single loud bin inside a wide cell range must win the pool
        let mut row = vec![-80.0f32; 512];
        row[300] = -3.0;
        let mut buf = VecDeque::new();
        buf.push_front(row);
        let v = pool_cell(&buf, 0, 1, 250, 350, BinsMode::All);
        assert!((v - -3.0).abs() < 1e-6);
    }

    #[test]
    fn peaks_mode_suppresses_non_peaks() {
        // Rising ramp: interior bins are never local maxima except the last
        let row: Vec<f32> = (0..64).map(|i| -80.0 + i as f32).collect();
        let mut buf = VecDeque::new();
        buf.push_front(row);
        // Cell covering interior of the ramp: loudest bin (hi-1) is not a peak
        let v = pool_cell(&buf, 0, 1, 10, 20, BinsMode::Peaks);
        assert!(v.is_infinite() && v < 0.0);
        // Cell containing the global max (last bin) is a peak
        let v = pool_cell(&buf, 0, 1, 60, 64, BinsMode::Peaks);
        assert!(v.is_finite());
    }

    #[test]
    fn overview_ranges_tile_history() {
        let total = 1000;
        let n = 37;
        let mut covered = 0usize;
        for y in 0..n {
            let (r0, r1) = overview_row_range(y, n, total);
            assert!(r0 <= covered);
            covered = covered.max(r1);
            assert!(r1 > r0);
        }
        assert_eq!(covered, total);
    }
}
