#!/usr/bin/env bash
# Records the README's demo and screenshots: demo.gif (ten seconds of the splash, from
# demo.tape) and every picture in docs/screenshots/ (from screenshots.tape, taken of the same
# example profiles the tests use).
#
# Needs vhs, ttyd and ffmpeg on PATH, and the nightly toolchain. Everything runs on throwaway
# vaults and homes in /tmp, which are removed afterwards, so nothing of yours is read or written.
set -euo pipefail

cd "$(dirname "$0")"

for tool in vhs ttyd ffmpeg; do
    if ! command -v "$tool" >/dev/null; then
        echo "demo-screenshots.sh needs $tool on PATH" >&2
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
cargo +nightly build --release

echo "Recording demo.gif..."
vhs demo.tape

echo "Laying out the example profiles..."
TDL_SCREENSHOT_DIR="$SHOTS" cargo +nightly test screenshot_vault -- --ignored --quiet

echo "Taking the screenshots..."
vhs screenshots.tape

echo "Done: demo.gif and docs/screenshots/"
