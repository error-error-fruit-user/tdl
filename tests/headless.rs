//! tdl's tests: the scripts of presses in `tests/scripts/`, each pressed by
//! `tdl headless --script ... --log` on a loadout of its own, with `tests/fake-uzdoom` standing in
//! for UZDoom, and checked by how the run ended and by what it left: `tdl.log`, a line per press
//! saying which screen heard it and what it did there, and `uzdoom.log`. Each run has a home of
//! its own and `tests/fixtures` as its wad dir. A failing run prints its `tdl.log`.
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

    /// The run's own account of itself, for a failure message.
    fn account(&self) -> String {
        format!(
            "stdout:\n{}\nstderr:\n{}\ntdl.log:\n{}",
            String::from_utf8_lossy(&self.output.stdout),
            String::from_utf8_lossy(&self.output.stderr),
            self.log("tdl.log")
        )
    }

    fn succeeded(&self) -> &Self {
        assert!(self.output.status.success(), "{}", self.account());
        assert!(self.log("tdl.log").contains("] done"), "{}", self.account());
        self
    }

    /// That `press`, heard by `screen`, did `deed` (spaces between them don't matter).
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

    fn played_uzdoom(&self, times: usize) -> &Self {
        let log = self.log("uzdoom.log");
        assert_eq!(log.matches("fake uzdoom args").count(), times, "{log}");
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

/// Plays `tests/scripts/<name>.txt` from the package's dir, so the paths in it are relative to it.
fn play(name: &str) -> Run {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = env::temp_dir().join(format!("tdl-script-{name}-{}", std::process::id()));
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
        .arg(root.join("tests/scripts").join(format!("{name}.txt")))
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

#[test]
fn a_profile_made_with_keys_and_buttons_launches_and_shows_its_log_window() {
    play("launch")
        .succeeded()
        .played_uzdoom(1)
        .confessed("⟨lb+a⟩", "launcher", "Add a Profile")
        .confessed("e", "editor", "Input (Spaces Too)")
        .confessed("⟨a⟩", "editor", "Save and Quit")
        .confessed("⟨up⟩", "launcher", "Traverse (Home/End: First/Last)")
        .confessed("⟨a⟩", "launcher", "Launch")
        .confessed("L", "running", "Show UZDoom's log")
        .confessed("⟨b⟩", "running", "Close UZDoom's log")
        .confessed("⟨b⟩", "launcher", "Save and Quit (unselects first)");
}

#[test]
fn a_profile_with_a_wad_from_the_wads_screen_is_hosted_with_the_host_trigger() {
    let run = play("host");
    run.succeeded()
        .played_uzdoom(1)
        .confessed("w", "editor", "Add Wads From the Wads/Files")
        .confessed(
            "enter",
            "library",
            "Open/Close Dir (Takes the Wad When Picking)",
        )
        // 🎮 the trigger makes F2, which the legend names only by its alias: H⟨host⟩
        .confessed("⟨host⟩", "launcher", "Host")
        .confessed("1", "host settings form", "Input")
        .confessed("enter", "host settings form", "Launch");
    let uzdoom = run.log("uzdoom.log");
    assert!(
        uzdoom.contains("-host 2") && uzdoom.contains("e1m1.wad"),
        "{uzdoom}"
    );
}

#[test]
fn buttons_are_written_the_way_the_legends_alias_them() {
    let run = play("controller");
    run.succeeded()
        .played_uzdoom(0)
        .confessed("⟨start⟩", "launcher", "Show the legend")
        .confessed("⟨b⟩", "legend", "Close the legend")
        .confessed(
            "⟨lb+rb+left⟩",
            "launcher",
            "Add a Profile That Watches a Dir",
        )
        .confessed("⟨lb+a⟩", "launcher", "Add a Profile")
        .confessed("⟨b⟩", "editor", "Quit (Asks to Save Changes)")
        .confessed("⟨b⟩", "launcher", "Save and Quit (unselects first)");
    // 🫢 the watched dir's dialog can't be answered, and the spirit is told
    assert!(
        run.log("tdl.log").contains("no spirit can answer one"),
        "{}",
        run.account()
    );
}

#[test]
fn buttons_can_be_written_in_plain_angle_brackets() {
    play("ascii")
        .succeeded()
        .confessed("<lb+a>", "launcher", "Add a Profile")
        .confessed("<b>", "editor", "Quit (Asks to Save Changes)")
        .confessed("<b>", "launcher", "Save and Quit (unselects first)");
}

#[test]
fn buttons_that_arent_any_end_the_run_with_an_error() {
    let run = play("no-such-button");
    assert_eq!(run.output.status.code(), Some(1), "{}", run.account());
    let log = run.log("tdl.log");
    assert!(
        log.contains("failed: line 2: ⟨lb+zz⟩ isn't buttons"),
        "{log}"
    );
}

#[test]
fn a_press_that_is_no_key_ends_the_run_with_an_error() {
    let run = play("unreadable");
    assert_eq!(run.output.status.code(), Some(1), "{}", run.account());
    let log = run.log("tdl.log");
    assert!(log.contains("failed: line 3"), "{log}");
    // 🛑 nothing on the bad line is pressed, even what comes before the bad press
    assert_eq!(log.matches("→").count(), 1, "{log}");
}
