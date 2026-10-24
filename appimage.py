#!/usr/bin/env python3
"""Build Linux AppImages of the programs next to this repository, for publish.py.

usage: python3 appimage.py [--out FOLDER] [program...]

Builds tdl and tdl-install (its installer) with `cargo build --release` and wraps each executable
in <name>.AppImage in FOLDER (default: published/releases/). They are terminal programs: the
AppImage runs the program with its arguments, and its desktop entry opens a terminal.

An AppImage is the runtime stub (secrets/appimage/runtime-x86_64, downloaded from
github.com/AppImage/type2-runtime the first time) followed by a squashfs of the program. Needs
cargo and mksquashfs (squashfs-tools). Linux only. Dependency-free Python 3.8+.
"""

import argparse
import os
import platform
import shutil
import struct
import subprocess
import sys
import tempfile
import urllib.request
import zlib

ROOT = os.path.dirname(os.path.abspath(__file__))
RUNTIME = os.path.join(ROOT, "secrets", "appimage", "runtime-x86_64")
RUNTIME_URL = "https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-x86_64"

PROGRAMS = ["tdl", "tdl-install"]

DESKTOP = """[Desktop Entry]
Type=Application
Name={name}
Exec={name}
Icon={name}
Categories=Utility;
Terminal=true
"""

APPRUN = """#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/{name}" "$@"
"""


def icon():
    """A plain 64x64 PNG, since an AppImage wants an icon."""
    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))
    row = b"\x00" + bytes((0x7a, 0x1f, 0x1f)) * 64
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 64, 64, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(row * 64, 9)) + chunk(b"IEND", b""))


def runtime():
    if not os.path.isfile(RUNTIME):
        print(f"downloading {RUNTIME_URL}", flush=True)
        os.makedirs(os.path.dirname(RUNTIME), exist_ok=True)
        urllib.request.urlretrieve(RUNTIME_URL, RUNTIME + ".part")
        os.replace(RUNTIME + ".part", RUNTIME)
    return RUNTIME


def build(name):
    """cargo build --release, and the path of the executable."""
    command = ["cargo", "build", "--release", "-p", name]
    print("+", " ".join(command), flush=True)
    subprocess.run(command, cwd=ROOT, check=True)
    return os.path.join(ROOT, "target", "release", name)


def make(name, executable, target):
    with tempfile.TemporaryDirectory() as work:
        app = os.path.join(work, name + ".AppDir")
        os.makedirs(os.path.join(app, "usr", "bin"))
        shutil.copy(executable, os.path.join(app, "usr", "bin", name))
        with open(os.path.join(app, "AppRun"), "w") as f:
            f.write(APPRUN.format(name=name))
        with open(os.path.join(app, name + ".desktop"), "w") as f:
            f.write(DESKTOP.format(name=name))
        with open(os.path.join(app, name + ".png"), "wb") as f:
            f.write(icon())
        os.symlink(name + ".png", os.path.join(app, ".DirIcon"))
        os.chmod(os.path.join(app, "AppRun"), 0o755)
        os.chmod(os.path.join(app, "usr", "bin", name), 0o755)

        image = os.path.join(work, "image.squashfs")
        subprocess.run(["mksquashfs", app, image, "-root-owned", "-noappend", "-quiet",
                        "-comp", "gzip"], check=True, stdout=subprocess.DEVNULL)
        partial = target + ".tmp"
        with open(partial, "wb") as out:
            for part in (runtime(), image):
                with open(part, "rb") as f:
                    shutil.copyfileobj(f, out)
        os.chmod(partial, 0o755)
        os.replace(partial, target)


def rust_on_path():
    """Puts rustup's folder ($CARGO_HOME/bin, else ~/.cargo/bin) on the PATH when cargo isn't on
    it, as a shell that never ran rustup's profile line leaves it; everything this runs inherits it."""
    if shutil.which("cargo"):
        return
    home = os.environ.get("CARGO_HOME") or os.path.join(os.path.expanduser("~"), ".cargo")
    folder = os.path.join(home, "bin")
    if any(os.path.isfile(os.path.join(folder, name)) for name in ("cargo", "cargo.exe")):
        os.environ["PATH"] = folder + os.pathsep + os.environ.get("PATH", "")


def build_all(out, names=None):
    """Make the AppImages of names (default: all) in out. Returns the paths made."""
    rust_on_path()
    if platform.system() != "Linux":
        print("note: AppImages are Linux-only; skipped", file=sys.stderr)
        return []
    for tool in ("cargo", "mksquashfs"):
        if shutil.which(tool) is None:
            sys.exit(f"making AppImages needs {tool}")
    os.makedirs(out, exist_ok=True)
    made = []
    for name in names or PROGRAMS:
        target = os.path.join(out, f"{name}.AppImage")
        make(name, build(name), target)
        print(f"{target}: {os.path.getsize(target) / (1024 * 1024):.1f} MB")
        made.append(target)
    return made


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out", default=os.path.join(ROOT, "published", "releases"), help="where to write them")
    parser.add_argument("programs", nargs="*", choices=PROGRAMS,
                        help="only these (default: all)")
    options = parser.parse_args()
    build_all(options.out, options.programs or None)


if __name__ == "__main__":
    main()
