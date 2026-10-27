#!/usr/bin/env python3
"""Build Linux AppImages of the programs next to this repository, for publish.py.

usage: python3 appimage.py [--out FOLDER] [program...]

Builds tdl and install-tdl (its installer) with `cargo build --release` and wraps each executable
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

GROANS_GITHUB = "https://raw.githubusercontent.com/error-error-fruit-user/installation-groans/main/"


def load_groans():
    """installation-groans' groans.py, which these scripts speak through: the workspace's
    checkout beside the project, else a copy
    fetched from its GitHub into published/artifacts/groans/."""
    project = ROOT
    places = [os.path.join(os.path.dirname(project), "installation-groans"),
              os.path.join(project, "published", "artifacts", "groans")]
    folder = next((place for place in places if os.path.isfile(os.path.join(place, "groans.py"))), None)
    if folder is None:
        folder = places[-1]
        try:
            import urllib.request
            os.makedirs(folder, exist_ok=True)
            for name in ("groans.py", "toiling-through-hell.toml", "computer-speak.toml"):
                with urllib.request.urlopen(GROANS_GITHUB + name, timeout=10) as answer:
                    with open(os.path.join(folder, name), "wb") as out:
                        out.write(answer.read())
        except Exception as error:
            sys.exit(f"installation-groans isn't beside {os.path.basename(project)}, and couldn't be "
                     f"fetched ({error}): clone it beside it.")
    if folder not in sys.path:
        sys.path.insert(0, folder)
    import groans
    return groans


groans = load_groans()

RUNTIME = os.path.join(ROOT, "secrets", "appimage", "runtime-x86_64")
RUNTIME_URL = "https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-x86_64"

PROGRAMS = ["tdl", "install-tdl"]

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
        groans.say("download", f"downloading {RUNTIME_URL}", name="the AppImage runtime", size="a few hundred KB")
        os.makedirs(os.path.dirname(RUNTIME), exist_ok=True)
        urllib.request.urlretrieve(RUNTIME_URL, RUNTIME + ".part")
        os.replace(RUNTIME + ".part", RUNTIME)
    return RUNTIME


def build(name):
    """cargo build --release, and the path of the executable."""
    groans.run(["cargo", "build", "--release", "-p", name], cwd=ROOT)
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
        groans.run(["mksquashfs", app, image, "-root-owned", "-noappend", "-quiet", "-comp", "gzip"],
                   quiet_run=True)
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
    groans.section("build")
    if platform.system() != "Linux":
        groans.other("note: AppImages are Linux-only; skipped", sys.stderr)
        return []
    for tool in ("cargo", "mksquashfs"):
        if shutil.which(tool) is None:
            sys.exit(f"making AppImages needs {tool}")
    os.makedirs(out, exist_ok=True)
    made = []
    for name in names or PROGRAMS:
        target = os.path.join(out, f"{name}.AppImage")
        make(name, build(name), target)
        groans.built(target)
        made.append(target)
    return made


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out", default=os.path.join(ROOT, "published", "releases"), help="where to write them")
    parser.add_argument("programs", nargs="*", choices=PROGRAMS,
                        help="only these (default: all)")
    parser.add_argument("--computer-speak", action="store_true",
                        help="say what it does plainly and formally, rather than as a descent "
                             "through hell")
    options = parser.parse_args()
    groans.start(None, options.computer_speak)
    try:
        build_all(options.out, options.programs or None)
    except subprocess.CalledProcessError as error:
        groans.finish("the AppImages", ok=False,
                      error=f"{' '.join(map(str, error.cmd))} ended with code {error.returncode}")
        sys.exit(1)
    groans.finish("the AppImages")


if __name__ == "__main__":
    main()
