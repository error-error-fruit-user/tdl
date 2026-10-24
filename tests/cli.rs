//! `tdl cli`'s tests (and `tdl setup`'s): the sessions people go through (profiles, a profile's
//! wads, the settings, the wad pools, sharing, setting up, launching), each gone through by three
//! kinds of user in their own way: a standard one (the obvious commands), a super one (every
//! option, other cases, odd but fair inputs) and a garbage one (inputs that are wrong, malicious or
//! dumb, which must end in errors, never damage). Each runs on a loadout and a home of its own and
//! is checked by what it says and by what it keeps, as later commands see it. `tests/fixtures`
//! holds the wads, and `tests/fake-uzdoom` stands in for UZDoom.
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
        self.fails_with_code(1, words);
    }

    /// Ended with `code` (2 for what clap or the setup TOML refuse), saying `words` on stderr.
    #[track_caller]
    fn fails_with_code(&self, code: i32, words: &str) {
        assert_eq!(
            self.output.status.code(),
            Some(code),
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
        self.tdl(&[&["cli"], args].concat())
    }

    /// `tdl --session --loadout <altar>/loadout <args>`, from the altar's dir.
    fn tdl(&self, args: &[&str]) -> Said {
        let home = self.dir.join("home");
        let output = Command::new(env!("CARGO_BIN_EXE_tdl"))
            .current_dir(&self.dir)
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env("XDG_DATA_HOME", home.join(".local/share"))
            .env("XDG_CACHE_HOME", home.join(".cache"))
            .args(["--session", "--loadout"])
            .arg(self.dir.join("loadout"))
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

/// The fake UZDoom, as a path.
fn uzdoom() -> String {
    Altar::root()
        .join("tests/fake-uzdoom")
        .display()
        .to_string()
}

#[test]
fn profiles() {
    // 🧑 standard
    let standard = Altar::new("profiles-standard");
    standard
        .rite(&["profiles"])
        .says("There are no profiles yet");
    standard
        .rite(&["new", "e1", &Altar::fixture("e1m1.wad")])
        .says("Made e1, with 1 wad");
    standard
        .rite(&["profiles"])
        .says("1 profiles")
        .says("e1  1 wad");
    standard.rite(&["show", "e1"]).says(" 1. ").says("e1m1.wad");
    // 🦸 super: a name with spaces, found in another case; copies, renames, deleting without asking
    let super_user = Altar::new("profiles-super");
    let fixtures = Altar::root().join("tests/fixtures").display().to_string();
    super_user
        .rite(&["new", "E1 Run", &Altar::fixture("e1m1.wad"), &fixtures])
        .says("Made E1 Run, with 2 wads");
    super_user
        .rite(&["copy", "e1 run"])
        .says("Copied E1 Run as E1 Run_2");
    super_user
        .rite(&["copy", "E1 Run", "third"])
        .says("Copied E1 Run as third");
    super_user
        .rite(&["rename", "E1 Run_2", "second"])
        .says("Renamed E1 Run_2 to second");
    super_user
        .rite(&["delete", "third", "--yes"])
        .says("Deleted third");
    super_user.rite(&["profiles"]).says("2 profiles");
    super_user.rite(&["show", "second"]).says("(mod folder)");
    // 🗑️ garbage: names that would escape the loadout or aren't names, taken names, no yes
    let garbage = Altar::new("profiles-garbage");
    garbage.rite(&["new", "e1"]).says("Made e1");
    for name in [
        "../../escape",
        "a/b",
        "$(rm -rf ~)",
        "e1\\n",
        "😈",
        &"x".repeat(200),
        " ",
    ] {
        garbage
            .rite(&["new", name])
            .fails_with("can't be a profile's name");
    }
    garbage
        .rite(&["new", "e1"])
        .fails_with("There is a profile called 'e1' already");
    garbage
        .rite(&["show", "nobody"])
        .fails_with("There is no profile called 'nobody'");
    garbage.rite(&["delete", "e1"]).says("Nothing was deleted");
    garbage.rite(&["profiles"]).says("e1  0 wads");
}

#[test]
fn a_profiles_wads() {
    // 🧑 standard: add one, take it out
    let standard = Altar::new("wads-standard");
    standard
        .rite(&["new", "e1", &Altar::fixture("e1m1.wad")])
        .says("Made e1");
    standard
        .rite(&["wads", "add", "e1", &Altar::fixture("base.wad")])
        .says("Added 1 wad to e1");
    standard
        .rite(&["wads", "remove", "e1", "base.wad"])
        .says("Took 1 wad out of e1");
    standard.rite(&["profiles"]).says("e1  1 wad");
    // 🦸 super: a place to add at, moving by name in another case, volatility, a replacement
    let super_user = Altar::new("wads-super");
    super_user
        .rite(&["new", "e1", &Altar::fixture("e1m1.wad")])
        .says("Made e1");
    super_user
        .rite(&[
            "wads",
            "add",
            "e1",
            &Altar::fixture("base.wad"),
            "--at",
            "1",
        ])
        .says("Added 1 wad");
    super_user
        .rite(&["wads", "move", "e1", "E1M1.WAD", "1"])
        .says("Moved e1m1.wad to place 1 in e1");
    super_user
        .rite(&["wads", "volatile", "e1", "1"])
        .says("e1m1.wad is volatile in e1");
    super_user
        .rite(&["show", "e1"])
        .says("e1m1.wad  (volatile)");
    super_user
        .rite(&["wads", "volatile", "e1", "1", "--off"])
        .says("no longer volatile");
    super_user
        .rite(&["wads", "replace", "e1", "2", &Altar::fixture("e1m1.wad")])
        .says("Put");
    super_user
        .rite(&["wads", "remove", "e1", "1", "2"])
        .says("Took 2 wads out of e1");
    // 🗑️ garbage: wads that aren't there, numbers that aren't places, profiles that aren't
    let garbage = Altar::new("wads-garbage");
    garbage
        .rite(&["new", "e1", &Altar::fixture("e1m1.wad")])
        .says("Made e1");
    garbage
        .rite(&["wads", "add", "e1", "/no/such.wad"])
        .fails_with("There is no file at");
    for wad in ["0", "99", "nope.wad"] {
        garbage
            .rite(&["wads", "remove", "e1", wad])
            .fails_with("has no wad");
    }
    garbage
        .rite(&["wads", "remove", "e1", "-1"])
        .fails_with_code(2, "unexpected argument '-1'");
    garbage
        .rite(&["wads", "move", "e1", "1", "50"])
        .fails_with("e1 has no place 50");
    garbage
        .rite(&["wads", "volatile", "nobody", "1"])
        .fails_with("There is no profile called 'nobody'");
    garbage.rite(&["profiles"]).says("e1  1 wad");
}

#[test]
fn the_settings() {
    // 🧑 standard: look, then change one
    let standard = Altar::new("settings-standard");
    standard
        .rite(&["settings"])
        .says("The global settings")
        .says("music         on the splash");
    standard
        .rite(&["settings", "music", "off"])
        .says("music         off");
    standard.rite(&["settings"]).says("music         off");
    // 🦸 super: many at once, a theme by name, a quip with a built-in in it, a profile's own args
    let super_user = Altar::new("settings-super");
    super_user
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
            "base-wad",
            &Altar::fixture("base.wad"),
        ])
        .says("music         kept playing")
        .says("theme         dracula")
        .says("splash screen off")
        .says("#12, hun!")
        .says("base.wad");
    super_user.rite(&["new", "e1"]).says("Made e1");
    super_user
        .rite(&[
            "set",
            "e1",
            "args",
            "-skill 4 +map E1M1",
            "uzdoom",
            &uzdoom(),
        ])
        .says("args      -skill 4 +map E1M1");
    super_user
        .rite(&["set", "e1", "args", "global"])
        .says("args      none");
    // 🗑️ garbage: settings that aren't any, half said, or pointing at what isn't a song or theme
    let garbage = Altar::new("settings-garbage");
    garbage
        .rite(&["settings", "colour", "red"])
        .fails_with("colour isn't one of");
    garbage
        .rite(&["settings", "music"])
        .fails_with("music has no value after it");
    garbage
        .rite(&["settings", "music", "loud"])
        .fails_with("music is off, splash or kept");
    garbage
        .rite(&["settings", "theme", "../../etc/passwd"])
        .fails_with("theme:");
    garbage
        .rite(&["settings", "song", "/etc/passwd"])
        .fails_with("song:");
    garbage.rite(&["new", "e1"]).says("Made e1");
    garbage
        .rite(&["set", "e1", "uzdoom", "/no/uzdoom"])
        .fails_with("There is no file at");
    garbage
        .rite(&["settings"])
        .says("music         on the splash");
}

#[test]
fn wad_pools() {
    let fixtures = Altar::root().join("tests/fixtures").display().to_string();
    // 🧑 standard: keep one for every loadout, see it, let it go
    let standard = Altar::new("pools-standard");
    standard
        .rite(&["pools", "add", &fixtures])
        .says("as a wad pool for every loadout");
    standard
        .rite(&["pools", "list"])
        .says("(kept for every loadout)")
        .says("(the downloads)");
    standard
        .rite(&["pools", "remove", &fixtures])
        .says("is no longer a wad pool");
    // 🦸 super: kept for this loadout only, and kept twice
    let super_user = Altar::new("pools-super");
    super_user
        .rite(&["pools", "add", &fixtures, "--this-loadout"])
        .says("for this loadout");
    super_user
        .rite(&["pools", "add", &fixtures, "--this-loadout"])
        .says("is a wad pool already");
    super_user
        .rite(&["pools", "list"])
        .says("(kept for this loadout)");
    // 🗑️ garbage: dirs that aren't, letting go of what was never kept
    let garbage = Altar::new("pools-garbage");
    garbage
        .rite(&["pools", "add", "/no/such/dir"])
        .fails_with("There is no dir at");
    garbage
        .rite(&["pools", "remove", &fixtures])
        .fails_with("isn't a kept wad pool");
}

#[test]
fn sharing() {
    let fixtures = Altar::root().join("tests/fixtures").display().to_string();
    let unseal = |taker: &Altar, scroll: &str, more: &[&str]| {
        taker.tdl(&[&["--wads", &fixtures, "cli", "unseal", scroll], more].concat())
    };
    // 🧑 standard: seal everything, unseal it elsewhere
    let maker = Altar::new("share-standard");
    maker
        .rite(&["new", "e1", &Altar::fixture("e1m1.wad")])
        .says("Made e1");
    let scroll = maker.dir.join("all").display().to_string();
    maker.rite(&["seal", &scroll]).says("Sealed 1 profile in");
    let taker = Altar::new("share-standard-taker");
    unseal(&taker, &format!("{scroll}.scroll"), &[]).says("Unsealed 1 profile");
    taker.rite(&["show", "e1"]).says("e1m1.wad");
    // 🦸 super: chosen profiles with the settings, into a loadout where a name is taken, and a
    // profile that watches a dir
    let maker = Altar::new("share-super");
    maker
        .rite(&["new", "e1", &Altar::fixture("e1m1.wad")])
        .says("Made e1");
    let mods = maker.dir.join("mods");
    fs::create_dir_all(&mods).unwrap();
    fs::copy(Altar::fixture("e1m1.wad"), mods.join("e1m1.wad")).unwrap();
    maker
        .rite(&["watch", &mods.display().to_string()])
        .says("Made mods, watching")
        .says("(1 wad in it)");
    maker.rite(&["settings", "music", "kept"]).says("Kept");
    let scroll = maker.dir.join("chosen").display().to_string();
    maker
        .rite(&["seal", &scroll, "e1", "mods", "--with-settings"])
        .says("Sealed 2 profiles and your global settings");
    let taker = Altar::new("share-super-taker");
    taker.rite(&["new", "e1"]).says("Made e1");
    unseal(&taker, &format!("{scroll}.scroll"), &["--with-settings"])
        .says("Unsealed 2 profiles")
        .says("Took the scroll's global settings");
    taker.rite(&["profiles"]).says("e1_2").says("mods");
    taker.rite(&["settings"]).says("music         kept playing");
    // 🗑️ garbage: a scroll that is junk, a seal over a file that is there, a dir that isn't
    let garbage = Altar::new("share-garbage");
    garbage.rite(&["new", "e1"]).says("Made e1");
    let junk = garbage.dir.join("junk.scroll");
    fs::write(&junk, b"\x00\xffnot a scroll at all").unwrap();
    garbage
        .rite(&["unseal", &junk.display().to_string()])
        .fails_with("isn't a scroll tdl can read");
    garbage
        .rite(&["seal", &junk.display().to_string()])
        .fails_with("There is already a file at");
    garbage
        .rite(&["seal", "/tmp/x", "nobody"])
        .fails_with("There is no profile called 'nobody'");
    garbage
        .rite(&["watch", "/no/such/dir"])
        .fails_with("There is no dir at");
}

#[test]
fn setting_up() {
    // 🧑 standard: setup.toml in the working folder, found without being named
    let standard = Altar::new("setup-standard");
    fs::write(
        standard.dir.join("setup.toml"),
        format!(
            "uzdoom = {:?}\nbase_wad = {:?}\n",
            uzdoom(),
            Altar::fixture("base.wad")
        ),
    )
    .unwrap();
    standard.tdl(&["setup"]).says("Kept what setup.toml says");
    standard
        .rite(&["settings"])
        .says("fake-uzdoom")
        .says("base.wad");
    // 🦸 super: relative paths, a theme, a loadout set up but not remembered, profiles made and
    // then made again (their wads taken again, not doubled)
    let super_user = Altar::new("setup-super");
    let toml = super_user.dir.join("nacht.toml");
    fs::write(
        &toml,
        format!(
            "theme = \"dracula\"\nloadout = \"local\"\nremember_loadout = false\n\n[[profiles]]\nname = \"e1\"\nwads = [{:?}]\nargs = \"-skill 4\"\n\n[[profiles]]\nname = \"Freedoom\"\nbase_wad = {:?}\n",
            Altar::fixture("e1m1.wad"),
            Altar::fixture("base.wad")
        ),
    )
    .unwrap();
    super_user
        .tdl(&["setup", &toml.display().to_string()])
        .says("Made the profile e1")
        .says("Made the profile Freedoom");
    super_user
        .tdl(&["setup", &toml.display().to_string()])
        .says("Took again the wads of the profile e1");
    // a profile put first in the list, as nacht's setup-waw.toml puts Nacht des Schreckens
    let first = super_user.dir.join("first.toml");
    fs::write(
        &first,
        "loadout = \"local\"\nremember_loadout = false\n\n[[profiles]]\nname = \"Schreckens\"\nplace = 1\n",
    )
    .unwrap();
    super_user
        .tdl(&["setup", &first.display().to_string()])
        .says("Made the profile Schreckens");
    let local = super_user.dir.join("local").display().to_string();
    let output = Command::new(env!("CARGO_BIN_EXE_tdl"))
        .env("HOME", super_user.dir.join("home"))
        .env("XDG_CONFIG_HOME", super_user.dir.join("home/.config"))
        .args(["--session", "--loadout", &local, "cli", "profiles"])
        .output()
        .unwrap();
    let listed = Said { output };
    listed.says("3 profiles").says("e1");
    let out = listed.out();
    let order: Vec<_> = ["Schreckens", "e1", "Freedoom"]
        .iter()
        .map(|name| out.find(name).unwrap())
        .collect();
    assert!(order.is_sorted(), "Schreckens isn't first in\n{out}");
    // 🗑️ garbage: keys that aren't, TOML that isn't, files that aren't there, a profile with no name
    let garbage = Altar::new("setup-garbage");
    let write = |name: &str, text: &str| {
        let path = garbage.dir.join(name);
        fs::write(&path, text).unwrap();
        path.display().to_string()
    };
    let unknown = write("unknown.toml", "colour = \"red\"\n");
    let broken = write("broken.toml", "uzdoom = \"unclosed\n");
    let missing = write("missing.toml", "uzdoom = \"/no/such/uzdoom\"\n");
    let nameless = write("nameless.toml", "[[profiles]]\nwads = []\n");
    let escaping = write("escaping.toml", "[[profiles]]\nname = \"../../escape\"\n");
    let misplaced = write(
        "misplaced.toml",
        "[[profiles]]\nname = \"e1\"\nplace = -1\n",
    );
    garbage
        .tdl(&["setup", &unknown])
        .fails_with_code(2, "unknown field `colour`");
    garbage
        .tdl(&["setup", &broken])
        .fails_with_code(2, "isn't a setup tdl can read");
    garbage
        .tdl(&["setup", &missing])
        .fails_with_code(2, "there is no file at");
    garbage
        .tdl(&["setup", &nameless])
        .fails_with_code(2, "missing field `name`");
    garbage
        .tdl(&["setup", &misplaced])
        .fails_with_code(2, "invalid value");
    garbage
        .tdl(&["setup", &escaping])
        .fails_with("can't be a profile's name");
    garbage
        .tdl(&["setup", "/no/such.toml"])
        .fails_with_code(2, "Could not read");
    garbage
        .tdl(&["setup"])
        .fails_with_code(2, "There is no setup.toml");
}

#[test]
fn launching() {
    // 🧑 standard: set UZDoom and the base wad, launch
    let standard = Altar::new("launch-standard");
    standard
        .rite(&["new", "e1", &Altar::fixture("e1m1.wad")])
        .says("Made e1");
    standard
        .rite(&[
            "settings",
            "uzdoom",
            &uzdoom(),
            "base-wad",
            &Altar::fixture("base.wad"),
        ])
        .says("Kept");
    standard
        .rite(&["launch", "e1"])
        .says("Launching e1")
        .says("The game is over");
    let log = fs::read_to_string(standard.dir.join("loadout/data/e1/uzdoom.log")).unwrap();
    assert!(log.contains("e1m1.wad"), "{log}");
    // 🦸 super: a profile's own UZDoom and args, found in another case, and hosted with options
    let super_user = Altar::new("launch-super");
    super_user.rite(&["new", "E1"]).says("Made E1");
    super_user
        .rite(&[
            "set",
            "E1",
            "uzdoom",
            &uzdoom(),
            "base-wad",
            &Altar::fixture("base.wad"),
            "args",
            "-fast",
        ])
        .says("-fast");
    super_user.rite(&["launch", "e1"]).says("Launching E1");
    super_user
        .rite(&[
            "host",
            "e1",
            "--port",
            "18766",
            "--code",
            "abc234",
            "--deathmatch",
        ])
        .says("The game is over");
    let log = fs::read_to_string(super_user.dir.join("loadout/data/E1/uzdoom.log")).unwrap();
    assert!(
        log.contains("-host") && log.contains("-fast") && log.contains("-deathmatch"),
        "{log}"
    );
    // 🗑️ garbage: no such profile, a join with no code, a port that can't be
    let garbage = Altar::new("launch-garbage");
    garbage
        .rite(&["launch", "nobody"])
        .fails_with("There is no profile called 'nobody'");
    garbage
        .rite(&["join", "10.0.0.2"])
        .fails_with("join --headless needs the host's join code");
    garbage.rite(&["new", "e1"]).says("Made e1");
    garbage
        .rite(&["host", "e1", "--port", "0"])
        .fails_with_code(2, "0");
}
