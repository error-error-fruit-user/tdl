#!/usr/bin/env bash
# Records the README's demo and screenshots: demo.gif (ten seconds of the splash, from
# demo.tape) and every picture in docs/screenshots/ (from screenshots.tape, taken of the example
# profiles that `tdl headless` lays out on the screens from screenshots.txt).
#
# Needs vhs, ttyd and ffmpeg on PATH, and Rust (stable is fine). Everything runs on throwaway
# loadouts and homes in /tmp, which are removed afterwards, so nothing of yours is read or written.
set -euo pipefail

cd "$(dirname "$0")"

for tool in vhs ttyd ffmpeg; do
    if ! command -v "$tool" >/dev/null; then
        echo "demo_screenshot.sh needs $tool on PATH" >&2
        exit 1
    fi
done

# 🧹 the tapes' throwaway dirs, cleared before and after, so a run never sees the last one's
SPLASH=/tmp/tdl-splash
SHOTS=/tmp/tdl-shots
cleanup() { rm -rf "$SPLASH" "$SHOTS"; }
trap cleanup EXIT
cleanup

echo "Building the release binary..."
cargo build --release

echo "Recording demo.gif..."
vhs demo.tape

echo "Laying out the example profiles..."
# 📁 the wads they use (a second dir, mods/, has an older brutalv22.pk3 of the same name) and a
# UZDoom that does nothing
mkdir -p "$SHOTS/wads" "$SHOTS/mods" "$SHOTS/home"
for name in DOOM2.WAD brutalv22.pk3 brutal_extras.pk3 hud_minimal.pk3 sigil_v1_21.wad gone.pk3 \
    changed.wad my_mod_wip.pk3; do
    printf '%s' "$name" >"$SHOTS/wads/$name"
done
for name in brutalv22.pk3 nashgore.pk3 smooth_weapons.pk3; do
    printf 'old %s' "$name" >"$SHOTS/mods/$name"
done
printf '#!/bin/sh\n' >"$SHOTS/uzdoom.sh"
chmod +x "$SHOTS/uzdoom.sh"
# 📜 played in $SHOTS, where its paths are, with the same HOME the tape gives tdl
tdl="$PWD/target/release/tdl"
script="$PWD/screenshots.txt"
(cd "$SHOTS" && env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$SHOTS/home" "$tdl" headless \
    --script "$script" --session --loadout "$SHOTS/loadout" --wads "$SHOTS/wads" \
    --wads "$SHOTS/mods" >/dev/null)
# 🩹 then one of sigil_run's wads goes missing and another changes
rm "$SHOTS/wads/gone.pk3"
printf 'changed-and-grown' >"$SHOTS/wads/changed.wad"

echo "Taking the screenshots..."
vhs screenshots.tape

echo "Done: demo.gif and docs/screenshots/"
