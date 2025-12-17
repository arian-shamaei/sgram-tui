Changelog

All notable changes to this project will be documented in this file.

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
