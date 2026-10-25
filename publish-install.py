#!/usr/bin/env python3
"""Publish tdl, then install it with the install-tdl just published.

usage: python3 publish-install.py [--accept] [--no-build | --source] [install-tdl options]

First it says, under a red warning, exactly what it will do, and asks once: "Are you sure? [y/n]".
The answer can carry any of its flags after the y (`y --mode user --menu-entry`), which win over
the same flags on the command line; --accept goes ahead without asking, with what the command line
says and the defaults for the rest (no shortcuts among them). Then it builds the release with
publish.py (tdl and install-tdl for Windows and as AppImages, into published/releases/), or with
--source only tdl and install-tdl for this machine (cargo build --release), or with --no-build
nothing at all; and runs the install-tdl in published/releases/, which copies the tdl beside it
where --mode says (in place by default), puts UZDoom and Freedoom 2 in its artifacts/ folder
(published/artifacts/ in place) and points tdl at them. --mode, --dir, --no-uzdoom,
--no-freedoom, --no-link, --desktop-shortcut and --menu-entry go to install-tdl (see its --help).

It speaks in a voice from ../installation-groans (Toiling Through Hell, or --computer-speak), and
everything it and what it runs did is in published/artifacts/publish-install.log.

Dependency-free Python 3.8+.
"""

import argparse
import os
import platform
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
RELEASES = os.path.join(HERE, "published", "releases")
JOURNAL = os.path.join(HERE, "published", "artifacts", "publish-install.log")
WINDOWS = platform.system() == "Windows"
EXE = ".exe" if WINDOWS else ""
# The workspace's programs, as --source leaves them in published/releases/
PROGRAMS = ["tdl", "install-tdl"]
GROANS_GITHUB = "https://raw.githubusercontent.com/error-error-fruit-user/installation-groans/main/"


def load_groans():
    """installation-groans' groans.py, which these scripts speak through: the project's
    installation-groans submodule, else the workspace's checkout beside the project, else a copy
    fetched from its GitHub into published/artifacts/groans/."""
    places = [os.path.join(HERE, "installation-groans"),
              os.path.join(os.path.dirname(HERE), "installation-groans"),
              os.path.join(HERE, "published", "artifacts", "groans")]
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
            sys.exit(f"installation-groans isn't beside {os.path.basename(HERE)}, and couldn't be "
                     f"fetched ({error}): clone it beside it.")
    sys.path.insert(0, folder)
    import groans
    return groans


groans = load_groans()


def rust_on_path():
    """Puts rustup's folder ($CARGO_HOME/bin, else ~/.cargo/bin) on the PATH when cargo isn't on
    it, as a shell that never ran rustup's profile line leaves it; everything this runs inherits it."""
    if shutil.which("cargo"):
        return
    home = os.environ.get("CARGO_HOME") or os.path.join(os.path.expanduser("~"), ".cargo")
    folder = os.path.join(home, "bin")
    if any(os.path.isfile(os.path.join(folder, name)) for name in ("cargo", "cargo.exe")):
        os.environ["PATH"] = folder + os.pathsep + os.environ.get("PATH", "")


def as_root():
    return not WINDOWS and os.geteuid() == 0


def check_owner(folders):
    """Stops, saying how to take them back, when anything in `folders` isn't this user's (a run
    with sudo leaves root's files behind)."""
    if WINDOWS or as_root():
        return
    for folder in folders:
        for root, dirs, files in os.walk(folder):
            for path in [root] + [os.path.join(root, name) for name in dirs + files]:
                try:
                    stranger = os.lstat(path).st_uid != os.getuid()
                except OSError:
                    continue
                if stranger:
                    import getpass
                    fix = "sudo chown -R " + getpass.getuser() + " " + " ".join(folders)
                    groans.say("problem", f"{path} isn't yours (a run with sudo left it)",
                               problem=f"{path} isn't yours")
                    sys.exit(f"Something in published/ belongs to another user, root most likely: "
                             f"nothing can replace it. Take it back with:\n  {fix}")


def plan(args):
    """What saying yes does, step by step."""
    lines = ["Saying yes will:"]
    if args.no_build:
        lines.append("  - build nothing, and install what published/releases/ has now")
    elif args.source:
        lines.append(f"  - build tdl{EXE} and install-tdl{EXE} for this machine (cargo build --release) and "
                     "copy them into published/releases/")
    else:
        lines.append("  - build tdl's release into published/releases/, replacing what is there: "
                     "tdl.exe and install-tdl.exe cross-built for Windows (Rust's "
                     "x86_64-pc-windows-gnu target), tdl.AppImage and install-tdl.AppImage (needs "
                     "mksquashfs), LICENSE and THIRD_PARTY_LICENSES.txt (needs cargo-license)")
    lines.append("  - run install-tdl from published/releases/, which:")
    where = {
        "in-place": "    - leaves tdl where it is, in published/releases/ (or --dir), and adds no command",
        "user": ("    - copies tdl to ~/.local/opt/tdl (or --dir) and puts the tdl command in ~/.local/bin, "
                 "adding a line to ~/.profile that puts it on your PATH if it isn't"
                 if not WINDOWS else
                 "    - copies tdl to %LOCALAPPDATA%\\Programs\\tdl (or --dir) and puts it on your PATH"),
    }
    lines.append(where[args.mode])
    if args.dir:
        art = "artifacts/ in --dir"
    elif args.mode == "in-place":
        art = "published/artifacts/"
    else:
        art = "artifacts/ beside tdl"
    lines.append(f"    - puts everything but tdl itself, its artifacts, in {art}")
    if not args.no_uzdoom:
        lines.append("    - downloads UZDoom 5.0.3 into uzdoom/ in the artifacts, checked against its pinned "
                     "size and hash")
    if not args.no_freedoom:
        lines.append("    - downloads Freedoom 0.13.0 and puts freedoom2.wad in wads/base_wads/ in the "
                     "artifacts")
    kept = [name for name, left_out in (("UZDoom", args.no_uzdoom), ("freedoom2.wad", args.no_freedoom))
            if not left_out]
    if kept and not args.no_link:
        lines.append(f"    - keeps that {' and '.join(kept)} as tdl's global "
                     f"{'ones' if len(kept) > 1 else 'one'}, in your tdl settings")
    for flag, where_to in (("desktop_shortcut", "a shortcut on your desktop"),
                           ("menu_entry", "an entry in your app menu")):
        if getattr(args, flag):
            lines.append(f"    - makes {where_to} for tdl, with the system's lock icon")
    lines.append("    - leaves alone what is there already")
    lines.append("")
    lines.append("To change that, here or after the y: --no-build installs what is published already, "
                 "--source builds only for this machine, --no-uzdoom, --no-freedoom and --no-link "
                 "leave those out, --mode in-place|user and --dir say where, "
                 "--desktop-shortcut and --menu-entry make shortcuts, --reinstall does it all again, and --computer-speak speaks "
                 "plainly.")
    return lines


def build(args):
    if args.no_build:
        return
    groans.section("build")
    if not args.reinstall and not args.source and groans.up_to_date([os.path.join(RELEASES, name + (".exe" if WINDOWS else ".AppImage")) for name in PROGRAMS], [HERE]):
        groans.say("skip", "The release is newer than its sources: nothing to build "
                   "(--reinstall builds it again)", what="tdl's release", place=RELEASES)
        return

    if args.source:
        groans.run(["cargo", "build", "--release", "--workspace"], cwd=HERE)
        os.makedirs(RELEASES, exist_ok=True)
        for program in PROGRAMS:
            shutil.copy2(os.path.join(HERE, "target", "release", program + EXE),
                         os.path.join(RELEASES, program + EXE))
        groans.note(f"copied the release build into {RELEASES}")
        return
    groans.run([sys.executable, "publish.py"], cwd=HERE, voiced=True)
    # 🧹 a plain tdl left by --source would be taken over the AppImage just built
    if not WINDOWS:
        for program in PROGRAMS:
            leftover = os.path.join(RELEASES, program)
            if os.path.isfile(leftover):
                os.remove(leftover)


def installer():
    """The install-tdl in published/releases/: the .exe on Windows, else the plain one --source
    built or the AppImage."""
    names = ["install-tdl.exe"] if WINDOWS else ["install-tdl", "install-tdl.AppImage"]
    for name in names:
        path = os.path.join(RELEASES, name)
        if os.path.isfile(path):
            return path
    sys.exit("There is no install-tdl in published/releases/: build it (leave out --no-build).")


def parser():
    flags = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    flags.add_argument("--accept", action="store_true",
                       help="go ahead without asking, with the defaults for what isn't given")
    build_how = flags.add_mutually_exclusive_group()
    build_how.add_argument("--no-build", action="store_true",
                           help="install what published/releases/ has now, building nothing")
    build_how.add_argument("--source", action="store_true",
                           help="build tdl and install-tdl for this machine only (cargo build --release)")
    passed = flags.add_argument_group("passed to install-tdl")
    passed.add_argument("--mode", choices=["in-place", "user"], default="in-place")
    passed.add_argument("--dir")
    passed.add_argument("--no-uzdoom", action="store_true")
    passed.add_argument("--no-freedoom", action="store_true")
    passed.add_argument("--no-link", action="store_true")
    passed.add_argument("--desktop-shortcut", action="store_true",
                        help="make a shortcut to tdl on the desktop")
    passed.add_argument("--menu-entry", action="store_true",
                        help="make an entry for tdl in the app menu (the Start menu on Windows)")
    flags.add_argument("--reinstall", action="store_true",
                       help="build and install everything again, where a run picks up where the last "
                            "one left off")
    flags.add_argument("--computer-speak", action="store_true",
                       help="say what it does plainly and formally, rather than as a descent through "
                            "hell (and have install-tdl do the same)")
    return flags


def main():
    rust_on_path()
    flags = parser()
    args = flags.parse_args()
    groans.start(JOURNAL, args.computer_speak)
    if as_root():
        groans.note("run as root: refused")
        sys.exit("Don't run this with sudo: it builds and installs for you (or in place), and root "
                 "would leave files in published/ and an install that only root can change.")
    groans.warning(plan(args))
    names = [name for action in flags._actions for name in action.option_strings
             if name not in ("-h", "--help", "--accept")]
    added = groans.confirm(args.accept, names)
    if added is None:
        groans.note("not going ahead: nothing was done")
        groans.declined()
    if added:
        # 🥇 the flags given with the y win over the command line's
        args = flags.parse_args(sys.argv[1:] + added)
        groans.note(f"with the flags given at the question: {' '.join(added)}")
    check_owner([folder for folder in (os.path.join(HERE, "published"),) if os.path.isdir(folder)])
    build(args)
    groans.section("install")
    command = [installer(), "--accept", "--mode", args.mode]
    if args.dir:
        command += ["--dir", args.dir]
    command += [flag for flag, on in (("--no-uzdoom", args.no_uzdoom),
                                      ("--no-freedoom", args.no_freedoom),
                                      ("--no-link", args.no_link),
                                      ("--reinstall", args.reinstall),
                                      ("--desktop-shortcut", args.desktop_shortcut),
                                      ("--menu-entry", args.menu_entry)) if on]
    groans.run(command, cwd=HERE, voiced=True)


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as error:
        groans.finish("tdl", ok=False, error=f"{' '.join(map(str, error.cmd))} ended with code {error.returncode}")
        sys.exit(1)
    groans.finish("tdl")
