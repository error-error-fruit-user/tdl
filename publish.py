#!/usr/bin/env python3
"""Cross-build the Windows release into published/releases/, with THIRD_PARTY_LICENSES.txt, and Linux
AppImages of tdl and install-tdl.

usage: python3 publish.py [--no-appimages]

Needs cargo-license (cargo install cargo-license). published/releases/ gets tdl.exe, install-tdl.exe
(the installer for UZDoom and Freedoom 2, from installer/), LICENSE,
upgrade_legacy.py and the license text of every crate linked into the binary. Then appimage.py
makes tdl.AppImage and install-tdl.AppImage there (needs mksquashfs;
--no-appimages skips them). A crate that ships
no license file gets its text from licenses/ instead (see licenses/README.md); the
script fails, naming them, if any has none.

Dependency-free Python 3.8+.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys

GROANS_GITHUB = "https://raw.githubusercontent.com/error-error-fruit-user/installation-groans/main/"


def load_groans():
    """installation-groans' groans.py, which these scripts speak through: the workspace's
    checkout beside the project, else a copy
    fetched from its GitHub into published/artifacts/groans/."""
    project = os.path.dirname(os.path.abspath(__file__))
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


import appimage

TARGET = "x86_64-pc-windows-gnu"
# The workspace's own programs, shipped side by side
PROGRAMS = ["tdl", "install-tdl"]
OUT = os.path.join("published", "releases")
# A crate that ships no license file gets its text from here
KEPT = "licenses"
# 🏚️ for anyone coming from a tdl before 1.0.0, whose files it upgrades
SCRIPTS = ["upgrade_legacy.py"]
LICENSE_PREFIXES = ("LICENSE", "LICENCE", "COPYING", "UNLICENSE", "NOTICE")


def cargo(*args, capture=False):
    """Runs cargo, in the voice; with `capture`, returns what it prints (its JSON)."""
    if capture:
        return groans.capture(["cargo", *args])
    groans.run(["cargo", *args])


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
                     "--filter-platform", TARGET, capture=True)
    metadata = cargo("metadata", "--format-version", "1", "--filter-platform", TARGET,
                     capture=True)
    crates = [c for c in json.loads(licensed) if c["name"] not in PROGRAMS]
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


def rust_on_path():
    """Puts rustup's folder ($CARGO_HOME/bin, else ~/.cargo/bin) on the PATH when cargo isn't on
    it, as a shell that never ran rustup's profile line leaves it; everything this runs inherits it."""
    if shutil.which("cargo"):
        return
    home = os.environ.get("CARGO_HOME") or os.path.join(os.path.expanduser("~"), ".cargo")
    folder = os.path.join(home, "bin")
    if any(os.path.isfile(os.path.join(folder, name)) for name in ("cargo", "cargo.exe")):
        os.environ["PATH"] = folder + os.pathsep + os.environ.get("PATH", "")


def clear(folder):
    """Empties `folder` for the new release. One that can't be emptied, left by a run with sudo,
    is said, with how to take it back, rather than failed on."""
    try:
        shutil.rmtree(folder)
    except FileNotFoundError:
        pass
    except PermissionError as error:
        import getpass
        sys.exit(f"{folder} can't be replaced ({error.strerror}: {error.filename}). It was left by "
                 "another user, root most likely, by a run with sudo. Take it back with:\n"
                 f"  sudo chown -R {getpass.getuser()} {os.path.abspath(os.path.dirname(folder))}")
    os.makedirs(folder)


def main():
    rust_on_path()
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--no-appimages", action="store_true", help="don't build the AppImages")
    parser.add_argument("--computer-speak", action="store_true",
                        help="say what it does plainly and formally, rather than as a descent "
                             "through hell")
    options = parser.parse_args()
    groans.start(None, options.computer_speak)
    groans.say("begin", "Publishing tdl", what="tdl", place=os.path.join("published", "releases"))
    groans.section("build")

    os.chdir(os.path.dirname(os.path.abspath(__file__)))
    if shutil.which("cargo-license") is None:
        sys.exit("cargo-license is missing: cargo install cargo-license")

    try:
        packages = [arg for program in PROGRAMS for arg in ("-p", program)]
        cargo("+stable", "build", "--release", "--target", TARGET, *packages)

        clear(OUT)
        for program in PROGRAMS:
            shutil.copy(os.path.join("target", TARGET, "release", program + ".exe"), OUT)
        shutil.copy("LICENSE", os.path.join(OUT, "LICENSE"))
        for script in SCRIPTS:
            shutil.copy(script, OUT)

        missing = third_party_licenses(os.path.join(OUT, "THIRD_PARTY_LICENSES.txt"))
        if not options.no_appimages:
            appimage.build_all(OUT)
    except subprocess.CalledProcessError as error:
        sys.exit(error.returncode)

    if missing:
        sys.exit("These crates ship no license file and have none in licenses/, so the release is "
                 "missing their text: " + ", ".join(missing))


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as error:
        groans.finish("tdl", ok=False,
                      error=f"{' '.join(map(str, error.cmd))} ended with code {error.returncode}")
        sys.exit(1)
    except SystemExit as stop:
        # 🗣️ a reason to stop is said in the voice too
        if isinstance(stop.code, str):
            groans.say("problem", stop.code, problem=stop.code)
            sys.exit(1)
        raise
    groans.finish("tdl")
