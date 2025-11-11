Sgram TUI
=========

Terminal spectrogram viewer with live feed from microphone or WAV file, with zoom, palettes, animation modes, and export to PNG/CSV.

[![Scope TUI Demo](https://img.youtube.com/vi/AtW3dyPjL08/0.jpg)](https://www.youtube.com/watch?v=AtW3dyPjL08 "Scope TUI Demo")

![Waterfall](saved/scope-tui_demo1.png)
![Horizontal](saved/scope-tui_demo2.png)

Features
--------
- Live spectrogram from `mic` or a `.wav` file
- Tunable analysis: window length (L), hop (H), FFT size (N), sample rate
- Absolute dB display with floor/ceiling; responsive floor control
- Zoom into low frequencies; linear/log/mel display scaling
- Styles: horizontal (time→x, freq→y) and waterfall (time→y, freq→x); overview + fullscreen
- Color palettes: Grayscale, Heat, Jet, Viridis, Inferno, Magma, Plasma, PurpleFire
- Fast rendering; low-latency updates; optional real-time sync for WAV
- Default save directory: `saved/` (auto-created); custom paths supported; PNG/CSV export

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
- `o`: Overview (fit entire history vertically into pane)
- `d`: Details (metadata + throughput; frequency ticks only)
- `s`/`w`: Quick save PNG/CSV (to `saved/` by default)
- `S`/`W`: Prompt for PNG/CSV path and save
- `h`/`F1`: Help overlay (usage + keys)

Details view
------------
Shows metadata and live processing throughput (rows/sec and real-time factor). Includes:
- Source, fs, L/H/N, bin spacing (df)
- dB floor/ceiling, zoom, scale, renderer
- Throughput (rows/sec) and RTF (~1.0 equals real-time)
- Total processed time (H×rows / fs)

Notes
-----
- WAV input is downmixed to mono and linearly resampled to target sample rate if needed.
- PNG export re-renders the terminal buffer; dimensions are fixed (800x600) for now.
- Replace demo assets in `saved/` (`demo.mp4`, screenshots) with your own captures.

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
