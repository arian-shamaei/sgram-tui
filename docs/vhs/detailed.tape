# Feature still: the detailed view (axes, scale, status) mid-playback.
# Regenerate: vhs docs/vhs/detailed.tape
Output docs/assets/detailed-frames.gif
Set FontSize 14
Set Width 1200
Set Height 560
Set Padding 8
Set Theme "Catppuccin Mocha"
Hide
Type "./target/release/sgram-tui docs/assets/sgram-spell.wav --realtime --style horizontal --palette viridis --zoom 10.5 --fft 4096 --detailed"
Enter
Show
Sleep 7s
Screenshot docs/assets/detailed.png
Sleep 0.5s
Type "q"
Sleep 0.5s
