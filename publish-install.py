#!/usr/bin/env python3
"""Publish tdl, then install it with the tdl-install just published.

usage: python3 publish-install.py [--accept] [--no-build | --source] [--no-shortcuts]
                                  [tdl-install options]

First it says, under a red warning, exactly what it will do, and asks before doing any of it
(--accept goes ahead without asking). Then it builds the release with publish.py (tdl and
tdl-install for Windows and as AppImages, into published/releases/), or with --source only tdl
and tdl-install for this machine (cargo build --release), or with --no-build nothing at all; and
runs the tdl-install in published/releases/, which copies the tdl beside it where it is asked to
(--mode), puts UZDoom and Freedoom 2 in its artifacts/ folder (published/artifacts/ in place) and
points tdl at them. --mode, --dir,
--no-uzdoom, --no-freedoom and --no-link go to tdl-install (see its --help). Once it may go
ahead, it asks whether to make shortcuts for tdl (the app menu and the desktop, or the Start menu
and the desktop on Windows) with the system's lock icon; --no-shortcuts makes none without asking.

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
WHAT = "tdl"
# The installer it runs, which takes --computer-speak too
INSTALLER = 'tdl-install'
EXE = ".exe" if WINDOWS else ""
# The workspace's programs, as --source leaves them in published/releases/
PROGRAMS = ["tdl", "tdl-install"]


# The journal: everything this does, with the time since it began (what it plans and is told,
# every command and all it prints, every file it puts somewhere), added to JOURNAL on each run.
JOURNAL = os.path.join(HERE, "published", "artifacts", "publish-install.log")
BEGAN = time.monotonic()
# Blender's line for every frame it renders, which would bury the rest
CHATTER = ("Fra:", "Saved:", "Time:", "Read blend")


def note(line):
    """Writes `line` down in the journal."""
    os.makedirs(os.path.dirname(JOURNAL), exist_ok=True)
    with open(JOURNAL, "a", encoding="utf-8") as journal:
        for part in line.splitlines() or [""]:
            journal.write(f"[{time.monotonic() - BEGAN:8.3f}s] {part}\n")


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
    path = os.path.join(os.path.dirname(HERE), "installation-groans", name)
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
    or `plain` without one; the journal keeps the plain line either way. Now and then the voice
    interjects (its [interjection])."""
    import random
    phrases = VOICE.get("phrases", {}).get(event) or []
    if not phrases:
        print(plain, flush=True)
        note(plain)
        return
    said = random.choice(phrases)
    for name, value in fills.items():
        said = said.replace("{" + name + "}", str(value))
    said = said[:1].upper() + said[1:]
    print(said, flush=True)
    note(f"{said}  ({plain})")
    interjection = VOICE.get("interjection", {})
    if interjection.get("phrases") and random.random() < float(interjection.get("chance", 0)):
        colors = {"yellow": 33, "green": 32, "blue": 34, "magenta": 35, "cyan": 36, "white": 37}
        code = colors.get(interjection.get("color", "red"), 31)
        shout = random.choice(interjection["phrases"])
        tty = sys.stdout.isatty() and "NO_COLOR" not in os.environ
        print(f"\033[1;{code}m{shout}\033[0m" if tty else shout, flush=True)
        note(shout)


def not_mine(folders):
    """The first file in `folders` (and the folders themselves) owned by someone else, as a run
    with sudo leaves them; None when every one is this user's."""
    if WINDOWS:
        return None
    for folder in folders:
        for root, dirs, files in os.walk(folder):
            for path in [root] + [os.path.join(root, name) for name in dirs + files]:
                try:
                    if os.lstat(path).st_uid != os.getuid():
                        return path
                except OSError:
                    pass
    return None


def check_owner(folders):
    """Stops, saying how to take them back, when anything in `folders` isn't this user's."""
    if WINDOWS or os.getuid() == 0:
        return
    stranger = not_mine(folders)
    if stranger is not None:
        import getpass
        fix = "sudo chown -R " + getpass.getuser() + " " + " ".join(folders)
        tell("problem", f"{stranger} isn't yours (a run with sudo left it)", problem=f"{stranger} isn't yours")
        sys.exit(f"Something in published/ belongs to another user, root most likely (a run with sudo "
                 f"leaves it): nothing can replace it. Take it back with:\n  {fix}")


def run(command, cwd=HERE):
    """Runs `command`, its output shown as it comes (prompts included) and written down, with how it
    ended. Raises CalledProcessError if it failed."""
    shown = " ".join(str(part) for part in command)
    tell("run", f"+ {shown}", command=shown)
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
    answer = input("Are you sure? [y/N] ").strip()
    note(f"asked to go ahead, the answer was {answer!r}")
    return answer.lower() in ("y", "yes")



# The system's lock icon for every shortcut: the icon theme's on Linux, shell32.dll's padlock on Windows
LOCK_ICON = "system-lock-screen"
WINDOWS_LOCK_ICON = r"%SystemRoot%\System32\shell32.dll,47"


def shortcut_places():
    """Where shortcuts go on Linux: the app menu, and the desktop if there is one."""
    home = os.path.expanduser("~")
    data = os.environ.get("XDG_DATA_HOME") or os.path.join(home, ".local", "share")
    places = [os.path.join(data, "applications")]
    try:
        desktop = subprocess.run(["xdg-user-dir", "DESKTOP"], capture_output=True,
                                 text=True).stdout.strip()
    except OSError:
        desktop = ""
    desktop = desktop or os.path.join(home, "Desktop")
    if os.path.isdir(desktop) and os.path.abspath(desktop) != os.path.abspath(home):
        places.append(desktop)
    return places


def wants_shortcuts(args, what):
    """Asks, once it may go ahead, whether to make shortcuts for `what` (--no-shortcuts: no,
    without asking)."""
    if args.no_shortcuts:
        note("--no-shortcuts: no shortcuts, and no question about them")
        return False
    if not sys.stdin.isatty():
        note("no one to ask about shortcuts: none are made")
        return False
    if WINDOWS:
        where = "your Start menu and desktop"
    else:
        where = " and ".join(place.replace(os.path.expanduser("~"), "~", 1)
                             for place in shortcut_places())
    answer = input(f"Make shortcuts for {what} in {where}, with the system's lock icon? "
                   "[y/N] ").strip()
    note(f"asked about shortcuts for {what}, the answer was {answer!r}")
    return answer.lower() in ("y", "yes")


def exec_quoted(arg):
    """`arg` as a desktop entry's Exec line wants it."""
    if arg and not any(c in arg for c in " \t\n\"'\\><~|&;$*?#()`"):
        return arg
    return '"' + "".join("\\" + c if c in '"`$\\' else c for c in arg) + '"'


def make_shortcut(title, file_id, command, comment):
    """A shortcut called `title` that runs `command` in a terminal, with the lock icon: a desktop
    entry `file_id`.desktop in each of `shortcut_places()` on Linux, a .lnk in the Start menu and on
    the desktop on Windows."""
    if WINDOWS:
        def ps(text):
            return "'" + text.replace("'", "''") + "'"
        script = (
            "$shell = New-Object -ComObject WScript.Shell\n"
            "foreach ($folder in @([Environment]::GetFolderPath('Programs'), "
            "[Environment]::GetFolderPath('Desktop'))) {\n"
            f"  $link = $shell.CreateShortcut((Join-Path $folder {ps(title + '.lnk')}))\n"
            f"  $link.TargetPath = {ps(command[0])}\n"
            f"  $link.Arguments = {ps(subprocess.list2cmdline(command[1:]))}\n"
            f"  $link.WorkingDirectory = {ps(os.path.dirname(os.path.abspath(command[0])))}\n"
            f"  $link.Description = {ps(comment)}\n"
            f"  $link.IconLocation = {ps(WINDOWS_LOCK_ICON)}\n"
            "  $link.Save()\n"
            "  Write-Output (Join-Path $folder " + ps(title + ".lnk") + ")\n"
            "}\n")
        run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script])
        return
    exec_line = " ".join(exec_quoted(arg) for arg in command)
    entry = ("[Desktop Entry]\nType=Application\n"
             f"Name={title}\nComment={comment}\n"
             f"Exec={exec_line.replace(chr(92), chr(92) * 2).replace('%', '%%')}\n"
             f"Icon={LOCK_ICON}\nTerminal=true\nCategories=Game;\n")
    for place in shortcut_places():
        os.makedirs(place, exist_ok=True)
        path = os.path.join(place, file_id + ".desktop")
        with open(path, "w", encoding="utf-8") as file:
            file.write(entry)
        os.chmod(path, 0o755)
        # a desktop launches only the shortcuts it is told to trust
        if not path.startswith(shortcut_places()[0]) and shutil.which("gio"):
            subprocess.run(["gio", "set", path, "metadata::trusted", "true"],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        note(f"made the shortcut {path}: {exec_line}")
        print("Shortcut:", path)


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
                   "(needs root: the installer runs with sudo)" if not WINDOWS else
                   "    - copies tdl to %ProgramFiles%\\tdl (or --dir) and puts it on the machine's PATH "
                   "(needs administrator)"),
    }
    lines.append(where.get(args.mode) or
                 "    - asks where to put tdl: in place, for you (~/.local/opt/tdl, with the tdl command "
                 "on your PATH) or for everyone (/opt/tdl, the installer run with sudo); asked before "
                 "the build")
    if args.dir:
        art = "artifacts/ in --dir"
    elif args.mode == "in-place":
        art = "published/artifacts/"
    else:
        art = "artifacts/ beside tdl (published/artifacts/ if tdl stays in place here)"
    lines.append(f"    - puts everything but tdl itself, its artifacts, in {art}: its log among them")
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
    lines.append("    - leaves alone what is there already")
    lines.append("")
    lines.append("To change that: --no-build installs what is published already, --source builds only "
                 "for this machine, --no-uzdoom, --no-freedoom and --no-link leave those out, and "
                 "--mode and --dir say where; --no-shortcuts doesn't ask about shortcuts.")
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


def installed_tdl(args):
    """The tdl tdl-install just put somewhere: in --dir, or where --mode puts it, or (when it
    asked) the newest of those places."""
    if args.dir:
        folders = [args.dir]
    else:
        home = os.path.expanduser("~")
        modes = {
            "in-place": RELEASES,
            "user": (os.path.join(os.environ.get("LOCALAPPDATA", ""), "Programs", "tdl") if WINDOWS
                     else os.path.join(home, ".local", "opt", "tdl")),
            "system": (os.path.join(os.environ.get("ProgramFiles", ""), "tdl") if WINDOWS
                       else "/opt/tdl"),
        }
        folders = [modes[args.mode]] if args.mode else list(modes.values())
    names = ["tdl.exe"] if WINDOWS else ["tdl", "tdl.AppImage"]
    found = [os.path.abspath(os.path.join(folder, name)) for folder in folders for name in names
             if os.path.isfile(os.path.join(folder, name))]
    return max(found, key=os.path.getmtime) if found else None


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


def refuse_root_build(args):
    """Building as root (sudo) leaves files in published/ that only root can change: refused."""
    if as_root() and not args.no_build:
        note("run as root: refused to build")
        sys.exit("Don't run this with sudo: building as root leaves files in published/ that only "
                 "root can change. Run it as yourself; to install for everyone, it runs only the "
                 "installer with sudo.")


def ask_mode(args):
    """Where to install, asked here when --mode doesn't say, so an install for everyone can run the
    installer with sudo instead of failing for want of root."""
    if args.mode or not sys.stdin.isatty():
        return
    choices = {"": "in-place", "1": "in-place", "2": "user", "3": "system"}
    print(f"Where should tdl be installed?\n"
          "  1) In place, in published/releases/ (default)\n"
          "  2) For you\n"
          "  3) For everyone (the installer runs with sudo, which asks for your password)")
    while True:
        answer = input("Choice [1]: ").strip()
        if answer in choices:
            args.mode = choices[answer]
            note(f"asked where to install, the answer was {answer!r}: {args.mode}")
            return
        print("1, 2 or 3")


def elevated(command, args):
    """`command`, run with sudo for an install for everyone when this isn't root already."""
    if args.mode == "system" and not WINDOWS and not as_root():
        print("Installing for everyone needs root: running the installer with sudo")
        note("running the installer with sudo")
        return ["sudo", *command]
    return command


def main():
    rust_on_path()
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
    parser.add_argument("--no-shortcuts", action="store_true",
                        help="make no shortcuts, and don't ask about them")
    parser.add_argument("--computer-speak", action="store_true",
                        help="say what it does plainly and formally, rather than as a descent "
                             "through hell" + (" (and have the installer do the same)" if INSTALLER else ""))
    args = parser.parse_args()
    speak_in("computer-speak.toml" if args.computer_speak else "toiling-through-hell.toml")
    note(f"=== {os.path.basename(__file__)} {' '.join(sys.argv[1:])} ===")
    refuse_root_build(args)

    warning(plan(args))
    if not agreed(args.accept):
        note("not going ahead: nothing was done")
        sys.exit("Nothing was done.")
    shortcuts = wants_shortcuts(args, "tdl")
    check_owner([folder for folder in (os.path.normpath(os.path.join(HERE, "published")),) if os.path.isdir(folder)])
    tell("begin", f"Publishing and installing {WHAT}", what=WHAT, place=os.path.join(HERE, "published"))
    ask_mode(args)
    build(args)
    command = [installer()] + (["--computer-speak"] if args.computer_speak else [])
    if args.mode:
        command += ["--mode", args.mode]
    if args.dir:
        command += ["--dir", args.dir]
    command += [flag for flag, on in (("--no-uzdoom", args.no_uzdoom),
                                      ("--no-freedoom", args.no_freedoom),
                                      ("--no-link", args.no_link)) if on]
    run(elevated(command, args))
    if shortcuts:
        tdl = installed_tdl(args)
        if tdl:
            make_shortcut("tdl", "tdl", [tdl], "Terminal launcher for UZDoom")
        else:
            print("Made no shortcut: tdl-install put tdl nowhere this can find.")


if __name__ == "__main__":
    try:
        main()
        note("done")
    except subprocess.CalledProcessError as error:
        note(f"failed: {error}")
        tell("problem", f"publish-install failed: {error}", problem=error)
        tell("journal", f"What it did is in {JOURNAL}", place=JOURNAL)
        sys.exit(1)
    tell("done", "Done.", what=WHAT)
    tell("journal", f"What it did is in {JOURNAL}", place=JOURNAL)
