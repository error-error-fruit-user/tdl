#!/usr/bin/bash
set -euo pipefail

target=x86_64-pc-windows-gnu
out=published

command -v cargo-license >/dev/null || { echo "cargo-license is missing: cargo install cargo-license" >&2; exit 1; }

cargo build --release --target "$target"

rm -rf "$out"
mkdir "$out"
cp "target/$target/release/tdl.exe" "$out/"
cp LICENSE "$out/LICENSE"
# 🏚️ for anyone coming from a tdl before 1.0.0, whose files it upgrades
cp upgrade_legacy.py "$out/"

# Gather the license text of every crate that is linked into the binary:
# cargo-license picks the crates (no build or dev dependencies, only those used by the target),
# cargo metadata says where each one's files are
cargo license --json --avoid-build-deps --avoid-dev-deps --filter-platform "$target" > "$out/.licenses.json"
cargo metadata --format-version 1 --filter-platform "$target" > "$out/.metadata.json"
# A crate that ships no license file gets its text from licenses/ instead (see licenses/README.md)
python3 -I - "$out/.licenses.json" "$out/.metadata.json" "$out/THIRD_PARTY_LICENSES.txt" licenses <<'PY'
import json, os, sys

crates = [c for c in json.load(open(sys.argv[1])) if c["name"] != "tdl"]
manifests = {(p["name"], p["version"]): p["manifest_path"] for p in json.load(open(sys.argv[2]))["packages"]}
kept = sys.argv[4]
missing = []

def kept_license(name):
    """The text kept in licenses/ for a crate: its own file, or its family's (the part of the name
    before the first -), or None."""
    for candidate in (name, name.split("-")[0]):
        path = os.path.join(kept, candidate + ".txt")
        if os.path.isfile(path):
            return open(path).read().rstrip() + "\n\n"
    return None

with open(sys.argv[3], "w") as out:
    for c in sorted(crates, key=lambda c: (c["name"], c["version"])):
        out.write("=" * 72 + "\n")
        name, version, license = c["name"], c["version"], c["license"]
        out.write(f"{name} {version} ({license})\n")
        out.write("=" * 72 + "\n\n")
        root = os.path.dirname(manifests[(name, version)])
        files = sorted(f for f in os.listdir(root) if f.upper().startswith(("LICENSE", "LICENCE", "COPYING", "UNLICENSE", "NOTICE")))
        for f in files:
            if os.path.isfile(os.path.join(root, f)):
                out.write(f"--- {f} ---\n")
                out.write(open(os.path.join(root, f), errors="replace").read().rstrip() + "\n\n")
        if not files:
            text = kept_license(name)
            if text is None:
                missing.append(f"{name} {version} ({c['repository'] or 'no repository listed'})")
                text = f"(no license file shipped in the crate; see {c['repository'] or 'its repository'})\n\n"
            out.write(text)

if missing:
    print("These crates ship no license file and have none in licenses/, so the release is missing their text:", file=sys.stderr)
    for crate in missing:
        print("  " + crate, file=sys.stderr)
    sys.exit(1)
PY
rm "$out/.licenses.json" "$out/.metadata.json"
