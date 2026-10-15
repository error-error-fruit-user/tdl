# Tour of tdl using a sandboxed copy of your real config + data.
# Run from the project root:  vhs demo.tape
# Needs: vhs, ttyd, ffmpeg on PATH, and `cargo build --release` done first.
# Nothing here launches UZDoom or touches the network.

Output demo.gif

Set Shell "bash"
Set FontSize 18
Set Width 1000
Set Height 760
Set Padding 20
Set Theme "Catppuccin Mocha"
Set TypingSpeed 250ms
Set PlaybackSpeed 0.6

# Work on a copy so swaps/edits/saves never touch ~/.config/tdl or ~/.local/share/tdl.
Hide
Type@5ms "rm -rf /tmp/tdl-demo && mkdir -p /tmp/tdl-demo && cp -r ~/.config/tdl /tmp/tdl-demo/config && cp -r ~/.local/share/tdl /tmp/tdl-demo/data" Enter
Type@5ms "clear && ./target/release/tdl /tmp/tdl-demo" Enter
Sleep 1s
Show

# --- Launcher + legend -------------------------------------------------
Sleep 1.5s
Type "?"
Sleep 3s
Type " "
Sleep 500ms

# --- Traverse (J/K and arrows), including the "+" row -----------------
Type "jjj"
Sleep 600ms
Down
Sleep 600ms
Down
Sleep 600ms
Up
Type "kk"
Sleep 1s

# --- Reorder profiles with } and { (restored afterwards) --------------
Type "}"
Sleep 700ms
Type "}"
Sleep 700ms
Type "{"
Sleep 700ms
Type "{"
Sleep 1s

# --- Edit a profile: wads table, then name field ----------------------
Type "e"
Sleep 1.5s
Type "?"
Sleep 3s
Type " "
Type "jj"
Sleep 600ms
Type "}"
Sleep 700ms
Type "{"
Sleep 700ms
Type "k"
Sleep 800ms
Tab
Sleep 1s
Type "?"
Sleep 2.5s
Type " "
Type "_v2"
Sleep 800ms
Backspace 3
Sleep 800ms
Escape
Sleep 1s

# --- Add a profile; a duplicate name triggers a warning ---------------
Type "a"
Sleep 1s
Type "vanilla"
Sleep 600ms
Enter
Sleep 2s
Type " "
Sleep 500ms
Type "?"
Sleep 2.5s
Type " "
Type "_copy"
Sleep 800ms
Backspace 12
Sleep 500ms
Escape
Sleep 1s

# --- Host setup: player count, mode, map ------------------------------
Type "h"
Sleep 1.5s
Type "?"
Sleep 2.5s
Type " "
Type "6"
Sleep 800ms
Tab
Sleep 800ms
Type "?"
Sleep 2.5s
Type " "
Type "j"
Sleep 800ms
Type "k"
Sleep 800ms
Tab
Sleep 800ms
Type "MAP07"
Sleep 1.2s
Type "?"
Sleep 2.5s
Type " "
Escape
Sleep 1s

# --- Join: IP input only accepts digits and dots ----------------------
Type "n"
Sleep 1.2s
Type "192.168.1.42"
Sleep 1s
Type "abc"
Sleep 600ms
Backspace 5
Sleep 800ms
Type "?"
Sleep 2.5s
Type " "
Escape
Sleep 1s

# --- Delete confirmation (answer no) ----------------------------------
Type "d"
Sleep 2s
Type "n"
Sleep 800ms

# --- Quit confirmation: decline, then confirm -------------------------
Escape
Sleep 1.5s
Type "n"
Sleep 800ms
Escape
Sleep 1.5s
Type "y"
Sleep 1.5s
