#!/usr/bin/env python3
"""Cross-build the Windows release into published/releases/, with THIRD_PARTY_LICENSES.txt, and Linux
AppImages of tdl and tdl-install.

usage: python3 publish.py [--no-appimages]

Needs cargo-license (cargo install cargo-license). published/releases/ gets tdl.exe, tdl-install.exe
(the installer for UZDoom and Freedoom 2, from installer/), LICENSE,
upgrade_legacy.py and the license text of every crate linked into the binary. Then appimage.py
makes tdl.AppImage and tdl-install.AppImage there (needs mksquashfs;
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

import appimage

TARGET = "x86_64-pc-windows-gnu"
# The workspace's own programs, shipped side by side
PROGRAMS = ["tdl", "tdl-install"]
OUT = os.path.join("published", "releases")
# A crate that ships no license file gets its text from here
KEPT = "licenses"
# 🏚️ for anyone coming from a tdl before 1.0.0, whose files it upgrades
SCRIPTS = ["upgrade_legacy.py"]
LICENSE_PREFIXES = ("LICENSE", "LICENCE", "COPYING", "UNLICENSE", "NOTICE")


def cargo(*args, **kwargs):
    shown = " ".join(["cargo", *args])
    tell("run", f"+ {shown}", command=shown)
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


# The voice it speaks in, from installation-groans: the workspace's checkout beside this project,
# else GitHub. Toiling Through Hell unless told --computer-speak; without one (or Python older than
# 3.11, which can't read TOML), plain lines.
GROANS = "https://raw.githubusercontent.com/error-error-fruit-user/installation-groans/main/"
VOICE = {}


def speak_in(name):
    """Speaks in the installation-groans voice `name` (a file name) from now on, if it is found."""
    global VOICE
    try:
        import tomllib
    except ImportError:
        return
    here = os.path.dirname(os.path.abspath(__file__))
    path = os.path.join(os.path.dirname(here), "installation-groans", name)
    try:
        if os.path.isfile(path):
            with open(path, encoding="utf-8") as source:
                text = source.read()
        else:
            import urllib.request
            with urllib.request.urlopen(GROANS + name, timeout=5) as answer:
                text = answer.read().decode()
        VOICE = tomllib.loads(text)
    except Exception:
        VOICE = {}


def tell(event, plain, **fills):
    """Says `event` as one of the voice's phrases for it, picked at random and filled from `fills`,
    or `plain` without one (a problem says `plain` too, if its phrase leaves the problem out). Now
    and then the voice interjects (its [interjection])."""
    import random
    phrases = VOICE.get("phrases", {}).get(event) or []
    out = sys.stderr if event == "problem" else sys.stdout
    if not phrases:
        print(plain, file=out, flush=True)
        return
    said = random.choice(phrases)
    for name, value in fills.items():
        said = said.replace("{" + name + "}", str(value))
    print(said[:1].upper() + said[1:], file=out, flush=True)
    if event == "problem" and str(fills.get("problem", plain)) not in said:
        print(plain, file=out, flush=True)
    interjection = VOICE.get("interjection", {})
    if interjection.get("phrases") and random.random() < float(interjection.get("chance", 0)):
        colors = {"yellow": 33, "green": 32, "blue": 34, "magenta": 35, "cyan": 36, "white": 37}
        code = colors.get(interjection.get("color", "red"), 31)
        shout = random.choice(interjection["phrases"])
        tty = sys.stdout.isatty() and "NO_COLOR" not in os.environ
        print(f"\033[1;{code}m{shout}\033[0m" if tty else shout, flush=True)


def built(target, count=None):
    """Says a file of the release is made: `target`, with how many files went into it."""
    size = os.path.getsize(target) / (1024 * 1024)
    inside = f"{count} files, " if count is not None else ""
    tell("copy", f"{target}: {inside}{size:.1f} MB",
         what=f"{os.path.basename(target)} ({inside}{size:.1f} MB)", place=os.path.dirname(target) or ".")


def main():
    rust_on_path()
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--no-appimages", action="store_true", help="don't build the AppImages")
    parser.add_argument("--computer-speak", action="store_true",
                        help="say what it does plainly and formally, rather than as a descent "
                             "through hell")
    options = parser.parse_args()
    speak_in("computer-speak.toml" if options.computer_speak else "toiling-through-hell.toml")
    tell("begin", "Publishing tdl", what="tdl", place=os.path.join("published", "releases"))

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
        print("These crates ship no license file and have none in licenses/, so the release is "
              "missing their text:", file=sys.stderr)
        for crate in missing:
            print("  " + crate, file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    try:
        main()
        tell("done", "Published.", what="tdl")
    except SystemExit as stop:
        # 🗣️ a reason to stop is said in the voice too
        if isinstance(stop.code, str):
            tell("problem", stop.code, problem=stop.code)
            sys.exit(1)
        raise
