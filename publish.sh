#!/usr/bin/bash
set -euo pipefail

target=x86_64-pc-windows-gnu
out=published

./build_release.sh

rm -rf "$out"
mkdir "$out"
cp "target/$target/release/tdl.exe" "$out/"
cp LICENSE "$out/LICENSE"

# Gather the license text of every crate that is linked into the binary
cargo metadata --format-version 1 --filter-platform "$target" > "$out/.metadata.json"
python3 -I - "$out/.metadata.json" "$out/THIRD_PARTY_LICENSES.txt" <<'PY'
import json, os, sys

meta = json.load(open(sys.argv[1]))
packages = {p["id"]: p for p in meta["packages"]}
nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}

# walk normal (non-build, non-dev) dependencies from the root
seen, stack = set(), [meta["resolve"]["root"]]
while stack:
    id = stack.pop()
    if id in seen:
        continue
    seen.add(id)
    for dep in nodes[id]["deps"]:
        if any(k["kind"] is None for k in dep["dep_kinds"]):
            stack.append(dep["pkg"])
seen.remove(meta["resolve"]["root"])

with open(sys.argv[2], "w") as out:
    for id in sorted(seen, key=lambda i: (packages[i]["name"], packages[i]["version"])):
        p = packages[id]
        out.write("=" * 72 + "\n")
        name, version, license = p["name"], p["version"], p["license"]
        out.write(f"{name} {version} ({license})\n")
        out.write("=" * 72 + "\n\n")
        root = os.path.dirname(p["manifest_path"])
        files = sorted(f for f in os.listdir(root) if f.upper().startswith(("LICENSE", "LICENCE", "COPYING", "UNLICENSE", "NOTICE")))
        for f in files:
            if os.path.isfile(os.path.join(root, f)):
                out.write(f"--- {f} ---\n")
                out.write(open(os.path.join(root, f), errors="replace").read().rstrip() + "\n\n")
        if not files:
            out.write("(no license file shipped in the crate; see its repository)\n\n")
PY
rm "$out/.metadata.json"
