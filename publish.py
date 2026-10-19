#!/usr/bin/env python3
"""Cross-build the Windows release into published/, with THIRD_PARTY_LICENSES.txt.

usage: python3 publish.py

Needs cargo-license (cargo install cargo-license). published/ gets tdl.exe, LICENSE,
upgrade_legacy.py and the license text of every crate linked into the binary. A crate that ships
no license file gets its text from licenses/ instead (see licenses/README.md); the
script fails, naming them, if any has none.

Dependency-free Python 3.8+.
"""

import json
import os
import shutil
import subprocess
import sys

TARGET = "x86_64-pc-windows-gnu"
OUT = "published"
# A crate that ships no license file gets its text from here
KEPT = "licenses"
# 🏚️ for anyone coming from a tdl before 1.0.0, whose files it upgrades
SCRIPTS = ["upgrade_legacy.py"]
LICENSE_PREFIXES = ("LICENSE", "LICENCE", "COPYING", "UNLICENSE", "NOTICE")


def cargo(*args, **kwargs):
    return subprocess.run(["cargo", *args], check=True, **kwargs)


def kept_license(name):
    """The text kept in licenses/ for a crate: its own file, or its family's (the part of the name
    before the first -), or None."""
    for candidate in (name, name.split("-")[0]):
        path = os.path.join(KEPT, candidate + ".txt")
        if os.path.isfile(path):
            with open(path) as f:
                return f.read().rstrip() + "\n\n"
    return None


def third_party_licenses(target):
    """Gather the license text of every crate that is linked into the binary into target:
    cargo-license picks the crates (no build or dev dependencies, only those used by the target),
    cargo metadata says where each one's files are. Returns the crates that had no text."""
    licensed = cargo("license", "--json", "--avoid-build-deps", "--avoid-dev-deps",
                     "--filter-platform", TARGET, stdout=subprocess.PIPE).stdout
    metadata = cargo("metadata", "--format-version", "1", "--filter-platform", TARGET,
                     stdout=subprocess.PIPE).stdout
    crates = [c for c in json.loads(licensed) if c["name"] != "tdl"]
    manifests = {(p["name"], p["version"]): p["manifest_path"]
                 for p in json.loads(metadata)["packages"]}
    missing = []

    with open(target, "w") as out:
        for c in sorted(crates, key=lambda c: (c["name"], c["version"])):
            out.write("=" * 72 + "\n")
            name, version, license = c["name"], c["version"], c["license"]
            out.write(f"{name} {version} ({license})\n")
            out.write("=" * 72 + "\n\n")
            root = os.path.dirname(manifests[(name, version)])
            files = sorted(f for f in os.listdir(root) if f.upper().startswith(LICENSE_PREFIXES))
            for f in files:
                if os.path.isfile(os.path.join(root, f)):
                    out.write(f"--- {f} ---\n")
                    with open(os.path.join(root, f), errors="replace") as text:
                        out.write(text.read().rstrip() + "\n\n")
            if not files:
                text = kept_license(name)
                if text is None:
                    missing.append(f"{name} {version} ({c['repository'] or 'no repository listed'})")
                    text = (f"(no license file shipped in the crate; see "
                            f"{c['repository'] or 'its repository'})\n\n")
                out.write(text)
    return missing


def main():
    os.chdir(os.path.dirname(os.path.abspath(__file__)))
    if shutil.which("cargo-license") is None:
        sys.exit("cargo-license is missing: cargo install cargo-license")

    try:
        cargo("+stable", "build", "--release", "--target", TARGET)

        shutil.rmtree(OUT, ignore_errors=True)
        os.makedirs(OUT)
        shutil.copy(os.path.join("target", TARGET, "release", "tdl.exe"), OUT)
        shutil.copy("LICENSE", os.path.join(OUT, "LICENSE"))
        for script in SCRIPTS:
            shutil.copy(script, OUT)

        missing = third_party_licenses(os.path.join(OUT, "THIRD_PARTY_LICENSES.txt"))
    except subprocess.CalledProcessError as error:
        sys.exit(error.returncode)

    if missing:
        print("These crates ship no license file and have none in licenses/, so the release is "
              "missing their text:", file=sys.stderr)
        for crate in missing:
            print("  " + crate, file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
