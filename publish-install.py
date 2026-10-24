#!/usr/bin/env python3
"""Publish tdl, then install it with the tdl-install just published.

usage: python3 publish-install.py [--accept] [--no-build | --source] [tdl-install options]

First it says, under a red warning, exactly what it will do, and asks before doing any of it
(--accept goes ahead without asking). Then it builds the release with publish.py (tdl and
tdl-install for Windows and as AppImages, into published/releases/), or with --source only tdl
and tdl-install for this machine (cargo build --release), or with --no-build nothing at all; and
runs the tdl-install in published/releases/, which copies the tdl beside it where it is asked to
(--mode), puts UZDoom and Freedoom 2 next to it and points tdl at them. --mode, --dir,
--no-uzdoom, --no-freedoom and --no-link go to tdl-install (see its --help).

Dependency-free Python 3.8+.
"""

import argparse
import os
import platform
import shutil
import subprocess
import sys
import textwrap
import time

HERE = os.path.dirname(os.path.abspath(__file__))
RELEASES = os.path.join(HERE, "published", "releases")
WINDOWS = platform.system() == "Windows"
EXE = ".exe" if WINDOWS else ""
# The workspace's programs, as --source leaves them in published/releases/
PROGRAMS = ["tdl", "tdl-install"]


# The journal: everything this does, with the time since it began (what it plans and is told,
# every command and all it prints, every file it puts somewhere), added to JOURNAL on each run.
JOURNAL = os.path.join(HERE, "published", "publish-install.log")
BEGAN = time.monotonic()
# Blender's line for every frame it renders, which would bury the rest
CHATTER = ("Fra:", "Saved:", "Time:", "Read blend")


def note(line):
    """Writes `line` down in the journal."""
    os.makedirs(os.path.dirname(JOURNAL), exist_ok=True)
    with open(JOURNAL, "a", encoding="utf-8") as journal:
        for part in line.splitlines() or [""]:
            journal.write(f"[{time.monotonic() - BEGAN:8.3f}s] {part}\n")


def run(command, cwd=HERE):
    """Runs `command`, its output shown as it comes (prompts included) and written down, with how it
    ended. Raises CalledProcessError if it failed."""
    shown = " ".join(str(part) for part in command)
    print("+", shown, flush=True)
    note(f"+ {shown}  (in {cwd})")
    process = subprocess.Popen(command, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    pending, quiet = b"", 0
    while True:
        chunk = process.stdout.read1(4096)
        if not chunk:
            break
        sys.stdout.buffer.write(chunk)
        sys.stdout.buffer.flush()
        *lines, pending = (pending + chunk).split(b"\n")
        for line in lines:
            text = line.decode(errors="replace").rstrip("\r")
            if text.startswith(CHATTER):
                quiet += 1
            else:
                note("  | " + text)
    if pending:
        note("  | " + pending.decode(errors="replace"))
    code = process.wait()
    if quiet:
        note(f"  ({quiet} lines of Blender's frame by frame progress left out)")
    note(f"  ended with code {code}")
    if code:
        raise subprocess.CalledProcessError(code, command)



def warning(lines):
    """`lines` under a red '--- Warning ---' (plain where the output isn't a terminal)."""
    if WINDOWS:
        os.system("")  # lets the Windows console show colors
    color = sys.stdout.isatty() and "NO_COLOR" not in os.environ
    print("\033[31m--- Warning ---\033[0m" if color else "--- Warning ---")
    note("--- Warning ---")
    for line in lines:
        note(line)
    width = min(shutil.get_terminal_size().columns, 100)
    for line in lines:
        # a step's later lines line up under its text
        hang = len(line) - len(line.lstrip(" -"))
        print(textwrap.fill(line, width, subsequent_indent=" " * hang,
                            break_on_hyphens=False, break_long_words=False) if line else "")
    print()


def agreed(accept):
    if accept:
        note("--accept: going ahead without asking")
        return True
    if not sys.stdin.isatty():
        note("no one to ask, and no --accept: nothing was done")
        sys.exit("There is no one to ask, so nothing was done: pass --accept to go ahead.")
    answer = input("Go ahead? [y/N] ").strip()
    note(f"asked to go ahead, the answer was {answer!r}")
    return answer.lower() in ("y", "yes")


def plan(args):
    """What saying yes does, step by step."""
    lines = ["Saying yes will:"]
    if args.no_build:
        lines.append("  - build nothing, and install what published/releases/ has now")
    elif args.source:
        lines.append(f"  - build tdl{EXE} and tdl-install{EXE} for this machine (cargo build --release) and "
                     "copy them into published/releases/")
    else:
        lines.append("  - build tdl's release into published/releases/, replacing what is there: "
                     "tdl.exe and tdl-install.exe cross-built for Windows (Rust's "
                     "x86_64-pc-windows-gnu target), tdl.AppImage and tdl-install.AppImage (needs "
                     "mksquashfs), LICENSE and THIRD_PARTY_LICENSES.txt (needs cargo-license)")
    lines.append("  - run tdl-install from published/releases/, which:")
    where = {
        "in-place": "    - leaves tdl where it is, in published/releases/ (or --dir), and adds no command",
        "user": ("    - copies tdl to ~/.local/opt/tdl (or --dir) and puts the tdl command in ~/.local/bin, "
                 "adding a line to ~/.profile that puts it on your PATH if it isn't"
                 if not WINDOWS else
                 "    - copies tdl to %LOCALAPPDATA%\\Programs\\tdl (or --dir) and puts it on your PATH"),
        "system": ("    - copies tdl to /opt/tdl (or --dir) and puts the tdl command in /usr/local/bin "
                   "(needs root)" if not WINDOWS else
                   "    - copies tdl to %ProgramFiles%\\tdl (or --dir) and puts it on the machine's PATH "
                   "(needs administrator)"),
    }
    lines.append(where.get(args.mode) or
                 "    - asks where to put tdl: in place, for you (~/.local/opt/tdl, with the tdl command "
                 "on your PATH) or for everyone (/opt/tdl, needs root)")
    if not args.no_uzdoom:
        lines.append("    - downloads UZDoom 5.0.3 into uzdoom/ beside tdl, checked against its pinned "
                     "size and hash")
    if not args.no_freedoom:
        lines.append("    - downloads Freedoom 0.13.0 and puts freedoom2.wad in wads/base_wads/ beside tdl")
    kept = [name for name, left_out in (("UZDoom", args.no_uzdoom), ("freedoom2.wad", args.no_freedoom))
            if not left_out]
    if kept and not args.no_link:
        lines.append(f"    - keeps that {' and '.join(kept)} as tdl's global "
                     f"{'ones' if len(kept) > 1 else 'one'}, in your tdl settings")
    lines.append("    - leaves alone what is there already")
    lines.append("")
    lines.append("To change that: --no-build installs what is published already, --source builds only "
                 "for this machine, --no-uzdoom, --no-freedoom and --no-link leave those out, and "
                 "--mode and --dir say where.")
    return lines


def build(args):
    if args.no_build:
        return
    if args.source:
        run(["cargo", "build", "--release", "--workspace"])
        os.makedirs(RELEASES, exist_ok=True)
        for program in PROGRAMS:
            shutil.copy2(os.path.join(HERE, "target", "release", program + EXE),
                         os.path.join(RELEASES, program + EXE))
            note(f"copied the release build into {RELEASES}")
        return
    run([sys.executable, "publish.py"])
    # 🧹 a plain tdl left by --source would be taken over the AppImage just built
    if not WINDOWS:
        for program in PROGRAMS:
            leftover = os.path.join(RELEASES, program)
            if os.path.isfile(leftover):
                os.remove(leftover)


def installer():
    """The tdl-install in published/releases/: the .exe on Windows, else the plain one --source
    built or the AppImage."""
    names = ["tdl-install.exe"] if WINDOWS else ["tdl-install", "tdl-install.AppImage"]
    for name in names:
        path = os.path.join(RELEASES, name)
        if os.path.isfile(path):
            return path
    sys.exit("There is no tdl-install in published/releases/: build it (leave out --no-build).")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--accept", action="store_true", help="go ahead without asking")
    build_how = parser.add_mutually_exclusive_group()
    build_how.add_argument("--no-build", action="store_true",
                           help="install what published/releases/ has now, building nothing")
    build_how.add_argument("--source", action="store_true",
                           help="build tdl and tdl-install for this machine only (cargo build --release)")
    passed = parser.add_argument_group("passed to tdl-install")
    passed.add_argument("--mode", choices=["in-place", "user", "system"])
    passed.add_argument("--dir")
    passed.add_argument("--no-uzdoom", action="store_true")
    passed.add_argument("--no-freedoom", action="store_true")
    passed.add_argument("--no-link", action="store_true")
    args = parser.parse_args()
    note(f"=== {os.path.basename(__file__)} {' '.join(sys.argv[1:])} ===")

    warning(plan(args))
    if not agreed(args.accept):
        note("not going ahead: nothing was done")
        sys.exit("Nothing was done.")
    build(args)
    command = [installer()]
    if args.mode:
        command += ["--mode", args.mode]
    if args.dir:
        command += ["--dir", args.dir]
    command += [flag for flag, on in (("--no-uzdoom", args.no_uzdoom),
                                      ("--no-freedoom", args.no_freedoom),
                                      ("--no-link", args.no_link)) if on]
    run(command)


if __name__ == "__main__":
    try:
        main()
        note("done")
    except subprocess.CalledProcessError as error:
        note(f"failed: {error}")
        sys.exit(f"publish-install failed: {error}\nWhat it did is in {JOURNAL}")
    print(f"What it did is in {JOURNAL}")
