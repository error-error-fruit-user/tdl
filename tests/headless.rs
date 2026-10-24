//! tdl's tests: the scripts in `tests/scripts/`, each played by `tdl headless --script ... --log`
//! on a loadout of its own, with `tests/fake-uzdoom` standing in for UZDoom, and checked by how
//! the run ended and by the `tdl.log` and `uzdoom.log` it left. Each run has a home of its own
//! and `tests/fixtures` as its wad dir. A script checks the screens itself (`expect`,
//! `wait-for`); a failing run prints its `tdl.log`, screens and all.
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
fn a_profile_made_in_the_editor_launches_with_its_log_window() {
    let run = play("launch");
    run.succeeded().played_uzdoom(1);
    assert!(run.log("tdl.log").contains("UZDoom's log"));
}

#[test]
fn a_command_plays_with_the_screens_closed_and_they_open_again() {
    let run = play("command");
    run.succeeded().played_uzdoom(1);
    let stdout = String::from_utf8_lossy(&run.output.stdout);
    assert!(stdout.contains("Launching e1"), "{}", run.account());
}

#[test]
fn a_file_dialog_is_answered_by_pick() {
    play("wads").succeeded().played_uzdoom(0);
}

#[test]
fn a_hosted_profile_starts_uzdoom_as_the_host() {
    let run = play("host");
    run.succeeded().played_uzdoom(1);
    assert!(run.log("uzdoom.log").contains("-host"), "{}", run.account());
}

#[test]
fn a_line_that_isnt_on_the_screen_fails_the_script() {
    let run = play("fails");
    assert_eq!(run.output.status.code(), Some(1), "{}", run.account());
    let log = run.log("tdl.log");
    assert!(log.contains("failed: line 3"), "{log}");
    // 🛑 nothing after the failure is played
    assert!(!log.contains("line 4: press a"), "{log}");
}
