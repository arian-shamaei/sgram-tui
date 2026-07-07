Changelog

All notable changes to this project will be documented in this file.

0.4.0 – Measurement tools, more formats, figure-quality exports
- Quadrant renderer (new default): 2x2 sub-pixels per terminal cell using
  quadrant glyphs with two-color quantization — 4x the pixel density of the
  old cell renderer, in both time and frequency, for waterfall and horizontal
  views. `--render quad|half|cell` selects explicitly; `--resolution low`
  still uses cell for minimal CPU.
- Stable layout: the status bar is constant-height, so the spectrogram no
  longer shifts when a status message or save prompt appears.
- Mouse hover readout: move the mouse over the display for a live
  time / frequency / dB readout in every view (fades after 3s idle).
- Peak-bins mode (--bins peaks, key `b`): draw only local spectral maxima,
  spending the terminal's limited pixels on information.
- Spectrum view (--style spectrum, `a` cycles all three styles):
  instantaneous spectrum of the newest frame as an eighth-block bar graph
  with dB/frequency axes and a live peak annotation.
- New input formats via symphonia: MP3, FLAC, Ogg/Vorbis, M4A/AAC
  (WAV still decodes via hound).
- Headless render mode: `sgram-tui render FILE [flags]` processes a whole
  file offline (no TUI) and writes a labeled PNG figure (+ optional CSV) —
  batch spectrograms in shell scripts.
- PNG exports are labeled scientific figures: frequency and time axes with
  tick marks (following the active linear/log/mel scale), a dB colorbar, and
  a metadata title (fs, N/L/H, floor/ceil) — rendered with an embedded 5x7
  bitmap font, no new dependencies. Exports default to 2x native data
  resolution; small sizes still export raw content.
- Rendering accuracy: every display cell (terminal and PNG) now max-pools
  all frequency bins and time rows it covers, so narrowband peaks are never
  skipped by nearest-bin sampling. Applies to all views, zoom levels, and
  overview mode.
- True colormaps: viridis, inferno, magma, plasma, and jet now use the
  standard reference anchor colors (the old polynomial approximations were
  visibly wrong — inferno rendered blue at the floor).
- Detailed-overlay axes are now style-aware: frequency ticks follow the axis
  the frequency is actually on (horizontal in waterfall view, vertical in
  horizontal view; dB axis in spectrum view).
- CSV/PNG default quick-saves unchanged; save paths still confirmed in the
  status bar.
- Review-sweep fixes:
  - Key events filter out Release, so Windows no longer double-fires every
    keystroke (double toggles, doubled prompt input).
  - Saving with an empty history is now an error in the status bar instead of
    a false "saved" confirmation for a file that was never written.
  - All time measurements (PNG time axis, hover readout, status span, render
    summary) use the effective clamped hop, not the raw CLI value.
  - DC bin calibration corrected (was +6 dB high; the two-sided sine factor
    does not apply at 0 Hz).
  - Input-pipeline errors (bad path, unsupported codec) surface as a sticky
    status-bar message instead of being swallowed by the alternate screen.
  - `render FILE` warns when a very long file exceeds the offline history cap
    instead of silently truncating the figure.
  - Explicit --history always wins over --resolution presets.
  - A file literally named "render" is still openable by name.
  - Spectrum hover readout honors peaks mode (suppressed bars read as below
    floor, matching the display).
  - Details panel no longer panics on very short fullscreen terminals.

0.3.0 – Calibrated dB, non-destructive zoom, robustness
- dB values are now calibrated dBFS: spectra are scaled by the window's coherent
  gain so a full-scale sine reads ~0 dB regardless of FFT size, window length,
  or window type. The default --floor/--ceil range (-80..0) is now physically
  meaningful. Applies to both magnitude (alpha=1) and power (alpha=2) modes.
- Zoom is non-destructive: the history buffer keeps full-resolution rows and
  zoom is applied at render time, so zooming out recovers detail and changing
  zoom mid-run no longer corrupts older rows. CSV export always contains the
  full 0..fs/2 resolution.
- New --window flag: hann (default), hamming, blackman.
- New key: `r` clears the spectrogram history.
- Details view: dB colorbar legend (read colors back as absolute dB), denser
  frequency ticks with kHz formatting.
- PNG quick-save exports at native data resolution (one pixel per bin/row,
  capped at 4096) instead of a fixed 800x600 canvas.
- Robustness: panic hook and error paths restore the terminal (no more broken
  shells in raw mode); save failures show in the status bar instead of exiting;
  saves confirm their path in the status bar; help overlay no longer underflows
  on tiny terminals; status bar sized so the info line is no longer clipped.
- Internal: dead fields/params removed; clippy-clean; new calibration test.

0.2.1 – CI and release fixes
- Update GitHub Actions to use `macos-15-intel` for Intel macOS builds (macOS 13 runners retired).
- Fix Linux CI by installing `pkg-config` and `libasound2-dev` for `alsa-sys`.
- Improve release workflow artifact download to collect all target builds.
- Minor code formatting tidy-ups; no functional changes.

0.2.0 – Improved DSP, exports, and mic stability
- PNG export honors the selected style (waterfall or horizontal), frequency scale (linear/log/mel), and zoom; maps full history cleanly.
- Added DSP options:
  - --normalize: normalize each frame to 0 dB peak.
  - --clamp-floor: clamp bins to the configured dB floor.
- Mic input is now non-blocking (avoids freezes when input queue is full) and resamples to target sample rate when needed.
- WAV 8-bit PCM handling corrected (centered around 0) to avoid DC bias artifacts.
- Fixed bottom-row artifacts in PNG export caused by naive row mapping.
- Unified UI scaling between Cell and Half render modes.
- Updated config directory identifiers (io.github/arian-shamaei/sgram-tui).
- Added tests across dsp/colors/export/input; cargo test passes.
- Added GitHub Actions CI for build/test/clippy/fmt on Linux/macOS/Windows.

0.1.1 – Initial public code
- Terminal spectrogram viewer with mic and WAV input.
- Export to PNG and CSV.
- Configurable FFT, hop, palettes, and render styles.
