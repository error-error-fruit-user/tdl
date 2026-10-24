//! `tdl cli`'s tests: each rite run on a loadout and a home of their own, checked by what it says
//! and by what it keeps, as later rites see it. `tests/fixtures` holds the wads, and
//! `tests/fake-uzdoom` stands in for UZDoom.
#![cfg(unix)]

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

/// A loadout and home to run rites in, gone afterwards unless a test failed.
struct Altar {
    dir: PathBuf,
}

/// What a rite said and how it ended.
struct Said {
    output: Output,
}

impl Said {
    fn out(&self) -> String {
        String::from_utf8_lossy(&self.output.stdout).into_owned()
    }

    fn err(&self) -> String {
        String::from_utf8_lossy(&self.output.stderr).into_owned()
    }

    #[track_caller]
    fn says(&self, words: &str) -> &Self {
        assert!(self.output.status.success(), "{}{}", self.out(), self.err());
        assert!(
            self.out().contains(words),
            "no \"{words}\" in\n{}",
            self.out()
        );
        self
    }

    #[track_caller]
    fn fails_with(&self, words: &str) {
        assert_eq!(
            self.output.status.code(),
            Some(1),
            "{}{}",
            self.out(),
            self.err()
        );
        assert!(
            self.err().contains(words),
            "no \"{words}\" in\n{}",
            self.err()
        );
    }
}

impl Altar {
    fn new(name: &str) -> Self {
        let dir = env::temp_dir().join(format!("tdl-cli-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("home")).unwrap();
        Altar { dir }
    }

    fn root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
    }

    fn fixture(name: &str) -> String {
        Self::root()
            .join("tests/fixtures")
            .join(name)
            .display()
            .to_string()
    }

    /// `tdl --session --loadout <altar>/loadout cli <args>`.
    fn rite(&self, args: &[&str]) -> Said {
        let home = self.dir.join("home");
        let output = Command::new(env!("CARGO_BIN_EXE_tdl"))
            .current_dir(&self.dir)
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env("XDG_DATA_HOME", home.join(".local/share"))
            .env("XDG_CACHE_HOME", home.join(".cache"))
            .args(["--session", "--loadout"])
            .arg(self.dir.join("loadout"))
            .arg("cli")
            .args(args)
            .output()
            .unwrap();
        Said { output }
    }
}

impl Drop for Altar {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }
}

#[test]
fn a_standard_user_makes_a_profile_adds_a_wad_and_plays_it() {
    let altar = Altar::new("standard");
    altar.rite(&["profiles"]).says("There are no profiles yet");
    altar
        .rite(&["new", "e1", &Altar::fixture("e1m1.wad")])
        .says("Made e1, with 1 wad");
    altar
        .rite(&["wads", "add", "e1", &Altar::fixture("base.wad")])
        .says("Added 1 wad to e1");
    altar
        .rite(&["profiles"])
        .says("1 profiles")
        .says("e1  2 wads");
    altar
        .rite(&["show", "e1"])
        .says(" 1. ")
        .says(" 2. ")
        .says("base.wad");
    let uzdoom = Altar::root()
        .join("tests/fake-uzdoom")
        .display()
        .to_string();
    altar
        .rite(&[
            "settings",
            "uzdoom",
            &uzdoom,
            "base-wad",
            &Altar::fixture("base.wad"),
        ])
        .says("Kept the settings");
    altar
        .rite(&["launch", "e1"])
        .says("Launching e1")
        .says("The game is over");
    let log = fs::read_to_string(altar.dir.join("loadout/data/e1/uzdoom.log")).unwrap();
    assert!(log.contains("e1m1.wad"), "{log}");
}

#[test]
fn a_super_user_reshapes_profiles_settings_pools_and_shares_them() {
    let altar = Altar::new("super");
    let fixtures = Altar::root().join("tests/fixtures").display().to_string();
    altar
        .rite(&["new", "E1 Run", &Altar::fixture("e1m1.wad"), &fixtures])
        .says("Made E1 Run, with 2 wads");
    // 🔠 a name in another case finds it, if only one fits
    altar
        .rite(&["copy", "e1 run"])
        .says("Copied E1 Run as E1 Run_2");
    altar
        .rite(&["rename", "E1 Run_2", "second"])
        .says("Renamed E1 Run_2 to second");
    altar
        .rite(&[
            "wads",
            "add",
            "second",
            &Altar::fixture("base.wad"),
            "--at",
            "1",
        ])
        .says("Added 1 wad");
    altar
        .rite(&["wads", "move", "second", "E1M1.WAD", "1"])
        .says("Moved e1m1.wad to place 1");
    altar
        .rite(&["wads", "volatile", "second", "1"])
        .says("is volatile");
    altar
        .rite(&[
            "wads",
            "replace",
            "second",
            "2",
            &Altar::fixture("e1m1.wad"),
        ])
        .says("Put");
    altar
        .rite(&["wads", "remove", "second", "2", "3"])
        .says("Took 2 wads out of second");
    altar.rite(&["show", "second"]).says("e1m1.wad  (volatile)");
    altar
        .rite(&[
            "set",
            "second",
            "args",
            "-skill 4 +map E1M1",
            "base-wad",
            &Altar::fixture("base.wad"),
        ])
        .says("args      -skill 4 +map E1M1");
    altar
        .rite(&["set", "second", "args", "global"])
        .says("args      none");
    altar
        .rite(&[
            "settings",
            "music",
            "kept",
            "quips",
            "off",
            "theme",
            "dracula",
            "splash-screen",
            "off",
            "splash-quip",
            "#12, hun!",
            "pride",
            "on",
        ])
        .says("music         kept playing")
        .says("theme         dracula")
        .says("splash screen off")
        .says("#12, hun!");
    altar
        .rite(&["pools", "add", &fixtures, "--this-loadout"])
        .says("for this loadout");
    altar
        .rite(&["pools", "list"])
        .says("(kept for this loadout)");
    altar
        .rite(&["pools", "remove", &fixtures])
        .says("no longer a wad pool");
    let mods = altar.dir.join("mods");
    fs::create_dir_all(&mods).unwrap();
    fs::copy(Altar::fixture("e1m1.wad"), mods.join("e1m1.wad")).unwrap();
    altar
        .rite(&["watch", &mods.display().to_string()])
        .says("Made mods, watching")
        .says("(1 wad in it)");
    let scroll = altar.dir.join("shared").display().to_string();
    altar
        .rite(&["seal", &scroll, "second", "mods", "--with-settings"])
        .says("Sealed 2 profiles and your global settings");
    // 📜 into a loadout of its own, where one name is taken already, and with the maker's settings
    let taker = Altar::new("super-taker");
    taker.rite(&["new", "second"]).says("Made second");
    let output = Command::new(env!("CARGO_BIN_EXE_tdl"))
        .env("HOME", taker.dir.join("home"))
        .env("XDG_CONFIG_HOME", taker.dir.join("home/.config"))
        .env("XDG_DATA_HOME", taker.dir.join("home/.local/share"))
        .args(["--session", "--loadout"])
        .arg(taker.dir.join("loadout"))
        .args(["--wads", &fixtures])
        .args([
            "cli",
            "unseal",
            &format!("{scroll}.scroll"),
            "--with-settings",
        ])
        .output()
        .unwrap();
    Said { output }
        .says("Unsealed 2 profiles")
        .says("Took the scroll's global settings");
    taker.rite(&["profiles"]).says("second_2").says("mods");
    taker
        .rite(&["settings"])
        .says("music         kept playing")
        .says("#12, hun!");
    altar
        .rite(&["delete", "second", "--yes"])
        .says("Deleted second");
}

#[test]
fn a_garbage_user_gets_errors_not_damage() {
    let altar = Altar::new("garbage");
    altar
        .rite(&["new", "e1", &Altar::fixture("e1m1.wad")])
        .says("Made e1");
    // 💀 names that would escape the loadout, or aren't names at all
    for name in [
        "../../escape",
        "a/b",
        "$(rm -rf ~)",
        "e1\n",
        "😈",
        &"x".repeat(200),
        " ",
    ] {
        altar
            .rite(&["new", name])
            .fails_with("can't be a profile's name");
    }
    altar
        .rite(&["new", "e1"])
        .fails_with("There is a profile called 'e1' already");
    altar.rite(&["rename", "e1", "e1"]).says("Renamed e1 to e1");
    // 🗑️ wads that aren't there, or aren't in the profile
    altar
        .rite(&["wads", "add", "e1", "/no/such.wad"])
        .fails_with("There is no file at");
    for wad in ["0", "99", "nope.wad"] {
        altar
            .rite(&["wads", "remove", "e1", wad])
            .fails_with("'e1' has no wad");
    }
    altar
        .rite(&["wads", "move", "e1", "1", "50"])
        .fails_with("e1 has no place 50");
    altar
        .rite(&["wads", "volatile", "nobody", "1"])
        .fails_with("There is no profile called 'nobody'");
    // 🎛️ settings that aren't any, half-said, or point at things that aren't songs or themes
    altar
        .rite(&["settings", "colour", "red"])
        .fails_with("colour isn't one of");
    altar
        .rite(&["settings", "music"])
        .fails_with("music has no value after it");
    altar
        .rite(&["settings", "music", "loud"])
        .fails_with("music is off, splash or kept");
    altar
        .rite(&["settings", "theme", "../../etc/passwd"])
        .fails_with("theme:");
    altar
        .rite(&["settings", "song", "/etc/passwd"])
        .fails_with("song:");
    // 📜 a scroll that is junk, a seal over a file that is there, a dir that isn't one
    let junk = altar.dir.join("junk.scroll");
    fs::write(&junk, b"\x00\xffnot a scroll at all").unwrap();
    altar
        .rite(&["unseal", &junk.display().to_string()])
        .fails_with("isn't a scroll tdl can read");
    altar
        .rite(&["seal", &junk.display().to_string()])
        .fails_with("There is already a file at");
    altar
        .rite(&["watch", "/no/such/dir"])
        .fails_with("There is no dir at");
    altar
        .rite(&["pools", "add", "/no/such/dir"])
        .fails_with("There is no dir at");
    // 🛑 nothing deleted without a yes, and with no terminal to say it on, it's no
    altar.rite(&["delete", "e1"]).says("Nothing was deleted");
    altar.rite(&["profiles"]).says("e1  1 wad");
}
