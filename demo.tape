# The demo: ten seconds of the splash screen.
# Run from the project root:  vhs demo.tape  (or ./demo-screenshots.sh, which also takes the
# README's screenshots)
# Needs: vhs, ttyd, ffmpeg on PATH, and `cargo build --release` done first.
#
# Kept light on purpose: a small window and a short clip. If rendering is still heavy,
# lower the Framerate first (12 still looks smooth), then the size.

Output demo.gif

Set Shell "bash"
Set FontSize 12
Set Width 640
Set Height 400
# No padding, so the fire runs right to the edges of the gif.
Set Padding 0
Set Framerate 35
Set TypingSpeed 0

# Start out of sight, on a throwaway vault and home, so nothing of yours (vaults, settings, your
# song) is read or written, and the splash looks the same for everyone.
Hide
Type "mkdir -p /tmp/tdl-splash/home && clear && HOME=/tmp/tdl-splash/home ./target/release/tdl --vault-once /tmp/tdl-splash/vault --no-music" Enter
Sleep 2000ms
Show

Sleep 10s

# Leave quietly: Ctrl+C ends the splash without saving anything.
Hide
Ctrl+C
