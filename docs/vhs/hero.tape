# Hero GIF: the TUI live-rendering the WAV that spells the tool's name.
# Regenerate: python3 scripts/spell.py SGRAM-TUI docs/assets/sgram-spell.wav
#             vhs docs/vhs/hero.tape
Output docs/assets/hero.gif
Set FontSize 14
Set Width 1200
Set Height 520
Set Padding 8
Set Theme "Catppuccin Mocha"
Hide
Type "./target/release/sgram-tui docs/assets/sgram-spell.wav --realtime --style horizontal --palette magma --zoom 10.5 --fft 4096 --hop 256 --render quad --fullscreen"
Enter
Show
Sleep 8.5s
Type "q"
Sleep 0.5s
