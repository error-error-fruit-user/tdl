# Short tour of tdl using a sandboxed copy of your real config + data.
# Run from the project root:  vhs demo.tape
# Needs: vhs, ttyd, ffmpeg on PATH, and `cargo build --release` done first.
# Nothing here launches UZDoom or touches the network.

Output demo.gif

Set Shell "bash"
Set FontSize 14
Set Width 640
Set Height 540
Set Padding 10
Set Framerate 8
Set Theme "Catppuccin Mocha"
Set TypingSpeed 200ms

# Work on a copy so swaps/edits/saves never touch ~/.config/tdl or ~/.local/share/tdl.
Hide
Type@5ms "rm -rf /tmp/tdl-demo && mkdir -p /tmp/tdl-demo && cp -r ~/.config/tdl /tmp/tdl-demo/config && cp -r ~/.local/share/tdl /tmp/tdl-demo/data" Enter
Type@5ms "clear && ./target/release/tdl --vault /tmp/tdl-demo" Enter
Sleep 1s
Show

# Launcher + legend
Sleep 1s
Type "?"
Sleep 2.5s
Type " "
Sleep 500ms

# Traverse and reorder (put back afterwards)
Type "jj"
Sleep 500ms
Type "}"
Sleep 800ms
Type "{"
Sleep 800ms
Type "jjjj"
Sleep 500ms
Type "kk"
Sleep 500ms

# Edit a profile
Type "e"
Sleep 1.5s
Type "?"
Sleep 2.5s
Type " "
Tab
Type "_v2"
Sleep 800ms
Backspace 3
Escape
Sleep 800ms

# Duplicate-name warning
Type "a"
Sleep 800ms
Type "vanilla"
Enter
Sleep 1.5s
Type " "
Backspace 7
Escape
Sleep 500ms

# Host setup
Type "h"
Sleep 1.2s
Type "6"
Tab
Type "j"
Sleep 800ms
Tab
Type "MAP07"
Sleep 1.2s
Escape
Sleep 500ms

# Join
Type "n"
Sleep 1s
Type "192.168.1.42"
Sleep 1s
Escape
Sleep 500ms

# Delete + quit dialogs
Type "d"
Sleep 1.5s
Type "n"
Escape
Sleep 1.5s
Type "y"
Sleep 1s
