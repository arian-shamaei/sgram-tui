Sgram TUI (sgram-tui)
=====================

Terminal spectrogram viewer with live feed from microphone or WAV file, with zoom, palettes, animation modes, and export to PNG/CSV.

Features
--------
- Live spectrogram from `mic` or a `.wav` file
- Adjustable FFT size, hop, sample rate, dB floor, FPS
- Zoom into low frequencies
- Animation styles: horizontal (time→x, freq→y) and vertical waterfall (time→y, freq→x)
- Color palettes: Grayscale, Heat, Jet, Viridis, Inferno, Magma, Plasma
- Exports snapshot to PNG (`s` quick save / `S` path prompt), CSV (`w` / `W`)
- Fullscreen mode, detailed overlay with frequency range
- Keyboard controls and status overlay
- Default save directory: `saved/` (auto-created); custom paths supported

Build
-----
- Requires Rust 1.70+ (edition 2021)
- Default build includes microphone support via `cpal`:
  - `cargo build --release`
- If you want to skip microphone feature: `cargo build --no-default-features`

Run
---
General usage
- `sgram-tui [mic|wav|FILE] [FILE] [flags]`
- Examples:
  - `sgram-tui wav path/to/audio.wav --fft 2048 --hop 512 --floor -90 --style waterfall --palette purplefire`
  - `sgram-tui mic --fft 1024 --hop 256 --device "Mic Name"`
  - `sgram-tui path/to/audio.wav --style horizontal`
  - Add `--resolution high` for a denser view; use `--render half` for higher time resolution in waterfall mode

Controls
--------
- `q`/`Esc`: Quit
- `p`: Pause/resume
- `a`: Toggle style (horizontal/waterfall)
- `+`/`-`: Zoom frequency range
- `[[/]]`: Adjust dB floor down/up
- `c`/`C`: Next/previous palette
- `f`: Fullscreen toggle
- `o`: Overview (fit entire history into pane)
- `d`: Details (frequency/time ticks + processing stats)
- `s`/`w`: Quick save PNG/CSV (to `saved/` by default)
- `S`/`W`: Prompt for PNG/CSV path and save
- `h`/`F1`: Help overlay (usage + keys)

Notes
-----
- WAV input is downmixed to mono and linearly resampled to target sample rate if needed.
- PNG export re-renders the terminal buffer; dimensions are fixed (800x600) for now.
- The display normalizes each frame to its instantaneous max; for absolute calibration, extend DSP to track global max.

Configuration
-------------
- Default config path: `${CONFIG_DIR}/sgram-tui/config.toml` (macOS: `~/Library/Application Support/sgram-tui/config.toml`, Linux: `~/.config/sgram-tui/config.toml`)
- Example `config.toml`:

  detailed = true
  fullscreen = false
  device = "USB Audio"   # substring match for mic device
  png_path = "./out.png" # default for quick save
  csv_path = "./out.csv"


Troubleshooting
---------------
- No input device (mic): rebuild without `mic` feature: `cargo run --no-default-features -- path.wav`
- Small/empty display: ensure FFT/hop are reasonable and terminal window is large enough.
- High CPU: reduce FPS, increase hop, or lower FFT size.



Flags
-----
- `--fft <N>`: FFT size (bin spacing fs/N)
- `--win <L>`: Window length; zero-pads to FFT if L < N
- `--hop <H>`: Hop size
- `--sample-rate <fs>`: Target sample rate for processing
- `--alpha <1|2>`: 1=magnitude dB, 2=power dB
- `--floor <dB>` / `--ceil <dB>`: dB range for display
- `--zoom <z>`: Zoom into low frequencies
- `--palette <name>`: grayscale, heat, jet, viridis, inferno, magma, plasma, purplefire
- `--style <mode>`: horizontal | waterfall
- `--render <mode>`: cell | half
- `--resolution <preset>`: low | medium | high | ultra
- `--freq-scale <scale>`: linear | log | mel (display warping)
- `--png-path <PATH>` / `--csv-path <PATH>`: quick-save destinations
- `--device <substring>`: mic device selection by substring
- `--overview`: start with overview mode enabled (fit history into pane)
- `--realtime`: throttle WAV to approximately real time


License
-------
MIT 
