# The README's screenshots, taken from the example profiles that screenshots.txt presses in.
# Run by demo_screenshot.sh, which lays them out in /tmp/tdl-shots first.
#
# vhs saves a screenshot a moment after asking for it, so each one is followed by a second's wait,
# or the next key gets there first and the picture shows the screen after it (or halfway to it).
# Needs vhs and ttyd on PATH. tdl runs with HOME in /tmp/tdl-shots, so your own settings and
# loadouts are never read or written, and with no splash or music.

Output "/tmp/tdl-shots/screenshots.gif"

Set Shell "bash"
Set FontSize 14
Set Width 870
Set Height 536
Set Padding 10
Set TypingSpeed 0

Hide
Type "mkdir -p /tmp/tdl-shots/home && clear && HOME=/tmp/tdl-shots/home ./target/release/tdl --session --loadout /tmp/tdl-shots/loadout --wads /tmp/tdl-shots/wads --wads /tmp/tdl-shots/mods --no-splash --no-music" Enter
Sleep 1s
Show

# 🩸 the launcher, on the broken profile, so its badge and the status panel show
Type "j"
Sleep 500ms
Screenshot "docs/screenshots/launcher.png"
Sleep 1s

# ✏️ the editor on the same profile, the cursor on the missing wad
Type "e"
Sleep 300ms
Type "j"
Sleep 500ms
Screenshot "docs/screenshots/editor.png"
Sleep 1s

# 🔎 what is wrong with it
Space
Sleep 500ms
Screenshot "docs/screenshots/inspect.png"
Sleep 1s
Escape
Sleep 300ms
Escape
Sleep 300ms

# 🎮 hosting the healthy profile, with a map typed in
Type "k"
Sleep 200ms
Type "h"
Sleep 300ms
Down
Down
Type "MAP07"
Sleep 500ms
Screenshot "docs/screenshots/host.png"
Sleep 1s
Escape
Sleep 300ms

# ⚙️ the settings
Type "o"
Sleep 500ms
Screenshot "docs/screenshots/settings.png"
Sleep 1s
Escape
Sleep 300ms

# 📚 the wads screen, each dir with its wads, then every wad in one list
Type "l"
Sleep 500ms
Screenshot "docs/screenshots/wads.png"
Sleep 1s
Tab
Sleep 500ms
Screenshot "docs/screenshots/every-wad.png"
Sleep 1s
Escape
Sleep 300ms

# 🔎 a search for a wad by name, before Enter
Type "/"
Type "*brutal"
Sleep 500ms
Screenshot "docs/screenshots/search.png"
Sleep 1s
Escape
Sleep 300ms

# 📋 one profile copied, then two marked for the next copy
Type "j"
Type "y"
Type "k"
Type "m"
Type "j"
Type "m"
Sleep 500ms
Screenshot "docs/screenshots/selection.png"
Sleep 1s

# Leave without saving anything: Ctrl+C ends tdl straight away.
Hide
Ctrl+C
