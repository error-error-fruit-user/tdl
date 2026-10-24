//! `tdl-install`: puts UZDoom and Freedoom 2 next to tdl, so a new player has something to play
//! with, and sets tdl up to use them. It asks (or `--mode` says) where tdl lives:
//!
//! - **in place**: where this program is (or `--dir`); nothing else changes;
//! - **for you**: tdl is copied to `~/.local/opt/tdl` (`%LOCALAPPDATA%\Programs\tdl` on Windows)
//!   and the `tdl` command is put on your PATH;
//! - **for everyone**: the same in `/opt/tdl` (`%ProgramFiles%\tdl`), with the command in
//!   `/usr/local/bin` (the machine's PATH on Windows); this needs root (administrator).
//!
//! tdl is the payload; everything else it makes goes in an `artifacts/` folder: `artifacts/` beside
//! tdl, or, when it runs in place from a project's `published/releases/`, that project's
//! `published/artifacts/` (`--artifacts` says another):
//!
//! ```text
//! artifacts/uzdoom/            UZDoom's own folder: the Windows release unpacked, or the AppImage
//! artifacts/wads/base_wads/    freedoom2.wad, with Freedoom's license
//! artifacts/setup.toml         what `tdl setup` keeps
//! artifacts/tdl-install.log    the journal
//! ```
//!
//! tdl then offers `artifacts/wads` and `artifacts/wads/base_wads` as wad pools when it starts. Each download is a
//! pinned release, checked against its size and BLAKE3 hash before anything is unpacked. What is
//! already there is left alone.
//!
//! One thing happens outside those folders, and `--no-link` leaves it out: the UZDoom and the base
//! wad are written in `artifacts/setup.toml`, and kept as tdl's global ones by running the `tdl`
//! there with `setup` on it.

#[macro_use]
mod journal;

use std::{
    fs,
    io::{self, IsTerminal, Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{Parser, ValueEnum};

/// A file to download: where from, and what it must be.
struct Download {
    name: &'static str,
    url: &'static str,
    size: u64,
    blake3: &'static str,
}

/// Freedoom 0.13.0, whose zip holds both Freedoom wads; only Phase 2 is taken. (Its SHA-256
/// matches the project's signed `freedoom-0.13.0-CHECKSUM`.)
const FREEDOOM: Download = Download {
    name: "freedoom-0.13.0.zip",
    url: "https://github.com/freedoom/freedoom/releases/download/v0.13.0/freedoom-0.13.0.zip",
    size: 24_143_781,
    blake3: "5f947833ecce402da78af8f63e9d647191b48f97dfa0ebd16b464548492d9719",
};
/// Freedoom 2 and its license inside [`FREEDOOM`], and what each is called once it is out.
const FREEDOOM_FILES: [(&str, &str); 2] = [
    ("freedoom-0.13.0/freedoom2.wad", "freedoom2.wad"),
    ("freedoom-0.13.0/COPYING.txt", "freedoom-COPYING.txt"),
];

/// UZDoom 5.0.3 for Windows: a zip of its folder, licenses included.
const UZDOOM_WINDOWS: Download = Download {
    name: "Windows-UZDoom-Release-x86_64.zip",
    url: "https://github.com/UZDoom/UZDoom/releases/download/5.0.3/Windows-UZDoom-Release-x86_64.zip",
    size: 29_337_585,
    blake3: "408b8017240488e648bbf5a3b99f05105f224b5960e31531ef626461f150baf1",
};
/// UZDoom 5.0.3 for Linux: one AppImage, which runs from wherever it is.
const UZDOOM_LINUX: Download = Download {
    name: "Linux-UZDoom-Release-x86_64.AppImage",
    url: "https://github.com/UZDoom/UZDoom/releases/download/5.0.3/Linux-UZDoom-Release-x86_64.AppImage",
    size: 34_560_504,
    blake3: "0f1d5177e2bf034b535ddf3fcee5755852378ef44533538b56c17e313a3b802d",
};
/// What the AppImage is called in `uzdoom/`.
const APPIMAGE: &str = "uzdoom.AppImage";

/// Puts UZDoom and Freedoom 2 next to tdl: `uzdoom/` and `wads/base_wads/freedoom2.wad`, beside
/// this program unless `--dir` says where. What is already there is left alone.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Where tdl lives: in place (next to this program), for you, or for everyone. Asked if not
    /// given (and in place if there is no terminal to ask on).
    #[arg(long, value_enum)]
    mode: Option<Mode>,
    /// The folder tdl is in. Defaults to the one this program is in (in place) or the mode's own
    /// folder.
    #[arg(long, value_name = "DIR")]
    dir: Option<PathBuf>,
    /// Where everything but tdl goes (UZDoom, Freedoom 2, setup.toml, the journal). Defaults to
    /// `artifacts/` in tdl's folder, or a project's `published/artifacts/` when this runs in place
    /// from its `published/releases/`.
    #[arg(long, value_name = "DIR")]
    artifacts: Option<PathBuf>,
    /// Leaves UZDoom out.
    #[arg(long)]
    no_uzdoom: bool,
    /// Leaves Freedoom 2 out.
    #[arg(long)]
    no_freedoom: bool,
    /// Doesn't point tdl at the UZDoom and the base wad.
    #[arg(long)]
    no_link: bool,
    /// Says what it does plainly and formally, rather than as a descent through hell
    #[arg(long)]
    computer_speak: bool,
}

/// One of installation-groans' voices, as this was built with it (see `build.rs`).
macro_rules! const_groan {
    ($voice:literal) => {
        include_str!(concat!(env!("OUT_DIR"), "/groans/", $voice))
    };
}

/// Toiling Through Hell, the voice the install speaks in unless told `--computer-speak`.
const TOILING_THROUGH_HELL: &str = const_groan!["toiling-through-hell.toml"];
/// Computer Speak, for `--computer-speak`.
const COMPUTER_SPEAK: &str = const_groan!["computer-speak.toml"];

/// What went wrong, for the user.
type Outcome<T> = Result<T, String>;

/// Where tdl lives.
#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
enum Mode {
    /// Next to this program: nothing is copied and no command is added.
    InPlace,
    /// Copied into your own folder, with the `tdl` command on your PATH.
    User,
    /// Copied into the system's folder, with the `tdl` command on everyone's PATH. Needs root.
    System,
}

/// The folder this program is in. An AppImage runs from a mount that goes away with it, so for
/// one it is the folder of the AppImage file itself (`$APPIMAGE`).
fn program_dir() -> Option<PathBuf> {
    let exe = std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| std::env::current_exe().ok())?;
    exe.parent().map(Path::to_path_buf)
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    journal::speak_in(if cli.computer_speak {
        COMPUTER_SPEAK
    } else {
        TOILING_THROUGH_HELL
    });
    let Some(here) = program_dir() else {
        warn!("Couldn't tell where this program is");
        return ExitCode::FAILURE;
    };
    let mode = cli.mode.unwrap_or_else(|| {
        if io::stdin().is_terminal() {
            ask_mode(&here)
        } else {
            Mode::InPlace
        }
    });
    let dir = match cli
        .dir
        .clone()
        .map(|dir| std::path::absolute(&dir).unwrap_or(dir))
    {
        Some(dir) => dir,
        None => match install_dir(mode, &here) {
            Ok(dir) => dir,
            Err(problem) => {
                warn!("{problem}");
                return ExitCode::FAILURE;
            }
        },
    };
    if mode == Mode::InPlace && !dir.is_dir() {
        warn!("{} isn't a folder", dir.display());
        return ExitCode::FAILURE;
    }
    let art = artifacts_dir(cli.artifacts.as_deref(), &dir, &here);
    // 📓 from here on the journal is kept with the artifacts
    let journal = journal::open(&art, "tdl-install.log");
    note!("this program is in {}", here.display());
    note!(
        "mode {mode:?}, installing in {}, artifacts in {}",
        dir.display(),
        art.display()
    );
    note!(
        "leaving out: uzdoom {}, freedoom {}, linking {}",
        cli.no_uzdoom,
        cli.no_freedoom,
        cli.no_link
    );
    if mode == Mode::System
        && let Err(problem) = check_privileges()
    {
        warn!("{problem}");
        return ExitCode::FAILURE;
    }
    match mode {
        Mode::InPlace => journal::tell(
            "begin",
            &[("what", "tdl"), ("place", &dir.display().to_string())],
            &format!("Installing next to tdl, in {}", dir.display()),
        ),
        Mode::User => journal::tell(
            "begin",
            &[("what", "tdl"), ("place", &dir.display().to_string())],
            &format!("Installing tdl for you, in {}", dir.display()),
        ),
        Mode::System => journal::tell(
            "begin",
            &[("what", "tdl"), ("place", &dir.display().to_string())],
            &format!("Installing tdl for everyone, in {}", dir.display()),
        ),
    }
    if mode != Mode::InPlace
        && let Err(problem) = place_tdl(&here, &dir)
    {
        warn!("tdl wasn't installed: {problem}");
        return ExitCode::FAILURE;
    }
    let scratch = Scratch::new();
    let mut failed = false;
    let mut report = |what: &str, outcome: Outcome<Option<PathBuf>>| match outcome {
        Ok(Some(path)) => say!("{what}: {}", path.display()),
        Ok(None) => {}
        Err(problem) => {
            warn!("{what} wasn't installed: {problem}");
            failed = true;
        }
    };
    if !cli.no_uzdoom {
        report("UZDoom", install_uzdoom(&art, &scratch));
    }
    if !cli.no_freedoom {
        report("Freedoom 2", install_freedoom(&art, &scratch));
    }
    if !cli.no_link {
        if mode == Mode::System {
            // 🔒 tdl setup would keep them in root's tdl settings, which nobody else uses
            match write_setup(
                &art,
                uzdoom_in(&art).as_deref(),
                Some(art.join("wads").join("base_wads").join(FREEDOOM_FILES[0].1))
                    .filter(|path| path.is_file())
                    .as_deref(),
            ) {
                Ok(setup) => say!(
                    "Each user points tdl at them in tdl (U and B), or runs: tdl setup {}",
                    setup.display()
                ),
                Err(problem) => warn!("{problem}"),
            }
        } else {
            link(&art, &dir);
        }
    }
    if mode != Mode::InPlace {
        match put_on_path(mode, &dir) {
            Ok(note) => journal::tell("path", &[("place", &bin_dir(mode, &dir))], &note),
            Err(problem) => {
                warn!("The tdl command wasn't added: {problem}");
                failed = true;
            }
        }
    }
    if let Some(journal) = &journal {
        journal::tell(
            "journal",
            &[("place", &journal.display().to_string())],
            &format!("Everything this install did is in {}", journal.display()),
        );
    }
    if failed {
        return ExitCode::FAILURE;
    }
    journal::tell(
        "done",
        &[("what", "tdl")],
        "Done. tdl offers the wads folders as wad pools when it starts.",
    );
    ExitCode::SUCCESS
}

/// Asks where tdl should live. Enter takes in place; input that ends takes it too.
fn ask_mode(here: &Path) -> Mode {
    say!("Where should tdl be installed?");
    say!(
        "  1) In place: in {}, nothing else changes (default)",
        here.display()
    );
    for (number, mode, who) in [
        (2, Mode::User, "for you"),
        (3, Mode::System, "for everyone"),
    ] {
        let folder = install_dir(mode, here)
            .map(|dir| dir.display().to_string())
            .unwrap_or_else(|_| "?".into());
        let note = if mode == Mode::System {
            "; needs root"
        } else {
            ""
        };
        say!(
            "  {number}) {}: in {folder}, with the tdl command on your PATH{note}",
            capital(who)
        );
    }
    loop {
        print!("Choice [1]: ");
        let _ = io::stdout().flush();
        let mut line = String::new();
        if io::stdin().read_line(&mut line).unwrap_or(0) == 0 {
            say!("");
            return Mode::InPlace;
        }
        match parse_choice(&line) {
            Some(mode) => return mode,
            None => say!("Type 1, 2 or 3."),
        }
    }
}

fn capital(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// What the user typed at the question, if it is an answer. Nothing at all is in place.
fn parse_choice(line: &str) -> Option<Mode> {
    match line.trim().to_lowercase().as_str() {
        "" | "1" | "in place" | "in-place" | "i" => Some(Mode::InPlace),
        "2" | "user" | "u" | "you" | "for you" => Some(Mode::User),
        "3" | "system" | "s" | "everyone" | "for everyone" => Some(Mode::System),
        _ => None,
    }
}

/// The folder tdl is installed to in `mode`; `here` is where this program is.
fn install_dir(mode: Mode, here: &Path) -> Outcome<PathBuf> {
    let var = |name: &str| {
        std::env::var_os(name)
            .map(PathBuf::from)
            .filter(|path| !path.as_os_str().is_empty())
            .ok_or_else(|| format!("${name} isn't set; pass --dir"))
    };
    Ok(match (mode, cfg!(windows)) {
        (Mode::InPlace, _) => here.to_path_buf(),
        (Mode::User, true) => var("LOCALAPPDATA")?.join("Programs").join("tdl"),
        (Mode::User, false) => var("HOME")?.join(".local").join("opt").join("tdl"),
        (Mode::System, true) => var("ProgramFiles")?.join("tdl"),
        (Mode::System, false) => PathBuf::from("/opt/tdl"),
    })
}

/// A system install writes where only root can: says so before anything is done.
fn check_privileges() -> Outcome<()> {
    #[cfg(windows)]
    let elevated = std::process::Command::new("net")
        .args(["session"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    #[cfg(not(windows))]
    let elevated = std::process::Command::new("id")
        .arg("-u")
        .output()
        .map(|output| output.stdout.trim_ascii() == b"0")
        .unwrap_or(false);
    if elevated {
        Ok(())
    } else if cfg!(windows) {
        Err("Installing for everyone needs an administrator: run this from an elevated terminal, or choose \"for you\"".into())
    } else {
        Err("Installing for everyone needs root: run this with sudo, or choose \"for you\"".into())
    }
}

/// The names tdl may have beside this program.
fn tdl_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["tdl.exe"]
    } else {
        &["tdl", "tdl.AppImage"]
    }
}

/// The `tdl` in `dir`, if there is one.
fn tdl_in(dir: &Path) -> Option<PathBuf> {
    tdl_names()
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
}

/// Copies the `tdl` beside this program (and its licenses) into `dir`, as `tdl`, unless the same
/// file is there already.
fn place_tdl(here: &Path, dir: &Path) -> Outcome<()> {
    let from = tdl_in(here).ok_or_else(|| {
        format!(
            "there is no tdl next to this program, in {}",
            here.display()
        )
    })?;
    let problem = |error: io::Error| format!("{}: {error}", dir.display());
    fs::create_dir_all(dir).map_err(problem)?;
    let to = dir.join(if cfg!(windows) { "tdl.exe" } else { "tdl" });
    note!("tdl: copying {} to {}", from.display(), to.display());
    if from != to {
        // 🧱 copied beside the real one, which is only replaced once it is whole (and a tdl that
        // is running can be replaced this way)
        let mut part = to.as_os_str().to_os_string();
        part.push(".part");
        let part = PathBuf::from(part);
        fs::copy(&from, &part).map_err(problem)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&part, fs::Permissions::from_mode(0o755)).map_err(problem)?;
        }
        fs::rename(&part, &to).map_err(problem)?;
    }
    for license in ["LICENSE", "THIRD_PARTY_LICENSES.txt"] {
        if here.join(license).is_file() && here != dir {
            fs::copy(here.join(license), dir.join(license)).map_err(problem)?;
            note!("copied {} into {}", license, dir.display());
        }
    }
    journal::tell(
        "copy",
        &[("what", "tdl"), ("place", &to.display().to_string())],
        &format!("tdl: {}", to.display()),
    );
    Ok(())
}

/// Where the `tdl` command goes for `mode`, as it is shown.
fn bin_dir(mode: Mode, dir: &Path) -> String {
    match (mode, std::env::var_os("HOME")) {
        _ if cfg!(windows) => dir.display().to_string(),
        (Mode::System, _) => "/usr/local/bin".into(),
        (_, Some(home)) => Path::new(&home)
            .join(".local")
            .join("bin")
            .display()
            .to_string(),
        (_, None) => dir.display().to_string(),
    }
}

/// Makes `tdl` a command anywhere in the terminal. Returns what to tell the user.
fn put_on_path(mode: Mode, dir: &Path) -> Outcome<String> {
    #[cfg(windows)]
    {
        windows_path(mode, dir)
    }
    #[cfg(not(windows))]
    {
        unix_path(mode, dir)
    }
}

#[cfg(not(windows))]
fn unix_path(mode: Mode, dir: &Path) -> Outcome<String> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let bin = match (mode, &home) {
        (Mode::System, _) => PathBuf::from("/usr/local/bin"),
        (_, Some(home)) => home.join(".local").join("bin"),
        (_, None) => return Err("$HOME isn't set".into()),
    };
    link_command(&bin, &dir.join("tdl"))?;
    let mut note = format!("The tdl command: {}", bin.join("tdl").display());
    let on_path = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).any(|entry| entry == bin))
        .unwrap_or(false);
    if !on_path
        && mode == Mode::User
        && let Some(home) = home
    {
        let profile = home.join(".profile");
        match add_to_profile(&profile, &bin)? {
                true => note.push_str(&format!(
                    "\n{} wasn't on your PATH, so a line adding it is now in {}. Open a new terminal (or run `. ~/.profile`) to use tdl anywhere.",
                    bin.display(),
                    profile.display()
                )),
                false => note.push_str(&format!(
                    "\n{} isn't on this terminal's PATH yet; {} adds it, so open a new terminal.",
                    bin.display(),
                    profile.display()
                )),
            }
    }
    Ok(note)
}

/// Makes `<bin>/tdl` a symlink to `target`, replacing an old symlink but never a real file.
#[cfg(not(windows))]
fn link_command(bin: &Path, target: &Path) -> Outcome<()> {
    let link = bin.join("tdl");
    let problem = |error: io::Error| format!("{}: {error}", link.display());
    fs::create_dir_all(bin).map_err(problem)?;
    match fs::symlink_metadata(&link) {
        Ok(meta) if meta.file_type().is_symlink() => fs::remove_file(&link).map_err(problem)?,
        Ok(_) => {
            return Err(format!(
                "{} is a file that isn't a link, so it is left alone",
                link.display()
            ));
        }
        Err(_) => {}
    }
    std::os::unix::fs::symlink(target, &link).map_err(problem)?;
    note!("linked {} to {}", link.display(), target.display());
    Ok(())
}

/// Adds a line to `profile` that puts `bin` on the PATH, unless this program put one there already.
/// Returns whether it wrote one.
#[cfg(not(windows))]
fn add_to_profile(profile: &Path, bin: &Path) -> Outcome<bool> {
    const MARK: &str = "# added by tdl-install";
    let problem = |error: io::Error| format!("{}: {error}", profile.display());
    let old = fs::read_to_string(profile).unwrap_or_default();
    if old.contains(MARK) {
        return Ok(false);
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(profile)
        .map_err(problem)?;
    let separator = if old.is_empty() || old.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    write!(
        file,
        "{separator}{MARK}\nexport PATH=\"{}:$PATH\"\n",
        bin.display()
    )
    .map_err(problem)?;
    note!(
        "added a line putting {} on the PATH to {}",
        bin.display(),
        profile.display()
    );
    Ok(true)
}

/// Adds `dir` to the user's (or the machine's) PATH, through PowerShell, which is how the
/// environment is changed so that every program sees it.
#[cfg(windows)]
fn windows_path(mode: Mode, dir: &Path) -> Outcome<String> {
    let scope = if mode == Mode::System {
        "Machine"
    } else {
        "User"
    };
    let script = format!(
        "$d = $env:TDL_DIR; $p = [Environment]::GetEnvironmentVariable('Path', '{scope}'); \
         if (($p -split ';') -notcontains $d) {{ \
           [Environment]::SetEnvironmentVariable('Path', ($p.TrimEnd(';') + ';' + $d), '{scope}') }}"
    );
    note!(
        "adding {} to the {scope} PATH through powershell",
        dir.display()
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .env("TDL_DIR", dir)
        .status()
        .map_err(|error| format!("couldn't run powershell: {error}"))?;
    if !status.success() {
        return Err(format!("powershell failed ({status})"));
    }
    Ok(format!(
        "{} is on the {} PATH now. Open a new terminal to use tdl anywhere.",
        dir.display(),
        scope.to_lowercase()
    ))
}

/// Where UZDoom is inside `<dir>/uzdoom/` on this system, if it is there.
/// Where everything but tdl goes: `given`, or, when this runs in place from a project's
/// `published/releases/`, that project's `published/artifacts/`, or else `<dir>/artifacts/`.
fn artifacts_dir(given: Option<&Path>, dir: &Path, here: &Path) -> PathBuf {
    if let Some(given) = given {
        return std::path::absolute(given).unwrap_or_else(|_| given.to_path_buf());
    }
    match here.parent() {
        Some(published)
            if dir == here
                && here.file_name().is_some_and(|name| name == "releases")
                && published
                    .file_name()
                    .is_some_and(|name| name == "published") =>
        {
            published.join("artifacts")
        }
        _ => dir.join("artifacts"),
    }
}

fn uzdoom_in(dir: &Path) -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "uzdoom.exe"
    } else {
        APPIMAGE
    };
    Some(dir.join("uzdoom").join(name)).filter(|path| path.is_file())
}

/// `setup.toml` in `dir` with `uzdoom` and `base_wad` set to the ones given, as paths relative to
/// `dir`, and a Freedoom profile that plays the base wad; whatever else a `setup.toml` already
/// there says is kept. `Err` if one is there and isn't
/// TOML, which is never written over.
fn write_setup(dir: &Path, uzdoom: Option<&Path>, base_wad: Option<&Path>) -> Outcome<PathBuf> {
    let path = dir.join("setup.toml");
    let mut table = match fs::read_to_string(&path) {
        Ok(text) => text.parse::<toml::Table>().map_err(|error| {
            format!(
                "{} is there and isn't TOML, so it is left alone: {error}",
                path.display()
            )
        })?,
        Err(_) => toml::Table::new(),
    };
    let relative = |file: &Path| file.strip_prefix(dir).unwrap_or(file).display().to_string();
    for (key, file) in [("uzdoom", uzdoom), ("base_wad", base_wad)] {
        if let Some(file) = file {
            table.insert(key.into(), toml::Value::String(relative(file)));
        }
    }
    // 🎮 Freedoom as a profile of its own, ready to play, unless the TOML has one by that name
    if let Some(freedoom) = base_wad {
        let profiles = table
            .entry("profiles")
            .or_insert_with(|| toml::Value::Array(Vec::new()));
        if let Some(profiles) = profiles.as_array_mut()
            && !profiles.iter().any(|profile| {
                profile.get("name").and_then(toml::Value::as_str) == Some("Freedoom")
            })
        {
            let mut profile = toml::Table::new();
            profile.insert("name".into(), "Freedoom".into());
            profile.insert("wads".into(), toml::Value::Array(Vec::new()));
            profile.insert("base_wad".into(), relative(freedoom).into());
            profiles.push(toml::Value::Table(profile));
        }
    }
    let text = format!(
        "# What tdl keeps, with no windows: `tdl setup` reads this (see tdl setup --help).\n# Paths are relative to this folder.\n{}",
        toml::to_string(&table).map_err(|error| error.to_string())?
    );
    fs::write(&path, text).map_err(|error| format!("{}: {error}", path.display()))?;
    note!("wrote {}: {}", path.display(), table);
    Ok(path)
}

/// Points tdl at the UZDoom and the base wad in `art`, the artifacts: writes them in
/// `art/setup.toml` and runs the `tdl` in `dir` with `setup` on it. Without a `tdl` there, or if it fails, says what to run instead.
/// Nothing is an error: the files are in place, and tdl can pick them (`U` and `B`).
fn link(art: &Path, dir: &Path) {
    let uzdoom = uzdoom_in(art);
    let base_wad = Some(art.join("wads").join("base_wads").join(FREEDOOM_FILES[0].1))
        .filter(|path| path.is_file());
    if uzdoom.is_none() && base_wad.is_none() {
        return;
    }
    let setup = match write_setup(art, uzdoom.as_deref(), base_wad.as_deref()) {
        Ok(setup) => setup,
        Err(problem) => {
            warn!("{problem}");
            say!("Point tdl at them in tdl (U and B) instead.");
            return;
        }
    };
    let tdl = tdl_in(dir).unwrap_or_else(|| dir.join("tdl"));
    let manually = || say!("To point tdl at them, run: tdl setup {}", setup.display());
    if !tdl.is_file() {
        say!(
            "tdl isn't in {}, so it isn't linked to them yet.",
            dir.display()
        );
        manually();
        return;
    }
    let command = format!("tdl setup {}", setup.display());
    journal::tell(
        "run",
        &[("command", &command)],
        &format!("running {}", command),
    );
    match std::process::Command::new(&tdl)
        .arg("setup")
        .arg(&setup)
        .status()
    {
        Ok(status) if status.success() => {
            if uzdoom.is_some() {
                journal::tell(
                    "link",
                    &[("what", "UZDoom"), ("place", "tdl's settings")],
                    "tdl now uses that UZDoom",
                );
            }
            if base_wad.is_some() {
                journal::tell(
                    "link",
                    &[("what", "the base wad"), ("place", "tdl's settings")],
                    "tdl now uses that base wad",
                );
            }
        }
        Ok(status) => {
            warn!("tdl setup failed ({status}), so it isn't linked to them yet.");
            manually();
        }
        Err(error) => {
            warn!("Couldn't run {}: {error}", tdl.display());
            manually();
        }
    }
}

/// Unpacks or places UZDoom in `<dir>/uzdoom/`, unless that is there already. Returns the
/// executable to pick in tdl, or `None` if nothing was done.
fn install_uzdoom(dir: &Path, scratch: &Scratch) -> Outcome<Option<PathBuf>> {
    let folder = dir.join("uzdoom");
    if folder.exists() {
        journal::tell(
            "skip",
            &[("what", "UZDoom"), ("place", &folder.display().to_string())],
            &format!(
                "UZDoom: {} is there already, so it is left alone",
                folder.display()
            ),
        );
        return Ok(None);
    }
    let download = if cfg!(all(windows, target_arch = "x86_64")) {
        &UZDOOM_WINDOWS
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        &UZDOOM_LINUX
    } else {
        return Err(
            "UZDoom has no release here for this system; get it from https://github.com/UZDoom/UZDoom/releases"
                .into(),
        );
    };
    let file = fetch(download, scratch)?;
    journal::tell(
        "unpack",
        &[
            ("name", download.name),
            ("place", &folder.display().to_string()),
        ],
        &format!("unpacking {} into {}", download.name, folder.display()),
    );
    // 🧱 built beside the real one, which only appears once it is whole
    let part = dir.join("uzdoom.part");
    let _ = fs::remove_dir_all(&part);
    let built = if download.name.ends_with(".zip") {
        unpack_all(&file, &part).map(|()| part.join("uzdoom.exe"))
    } else {
        place_appimage(&file, &part)
    };
    let executable = match built {
        Ok(executable) => executable,
        Err(problem) => {
            let _ = fs::remove_dir_all(&part);
            return Err(problem);
        }
    };
    fs::rename(&part, &folder).map_err(|error| {
        let _ = fs::remove_dir_all(&part);
        format!("couldn't make {}: {error}", folder.display())
    })?;
    Ok(Some(
        folder.join(executable.file_name().unwrap_or_default()),
    ))
}

/// Copies the AppImage into `folder` and lets it run. Returns where it is.
fn place_appimage(file: &Path, folder: &Path) -> Outcome<PathBuf> {
    let made = |error: io::Error| format!("couldn't make {}: {error}", folder.display());
    fs::create_dir_all(folder).map_err(made)?;
    let appimage = folder.join(APPIMAGE);
    fs::copy(file, &appimage).map_err(made)?;
    note!(
        "copied {} to {}, and made it runnable",
        file.display(),
        appimage.display()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&appimage, fs::Permissions::from_mode(0o755)).map_err(made)?;
    }
    Ok(appimage)
}

/// Takes Freedoom 2 and its license out of the Freedoom zip into `<dir>/wads/base_wads/`, unless
/// the wad is there already. Returns the wad, or `None` if nothing was done.
fn install_freedoom(dir: &Path, scratch: &Scratch) -> Outcome<Option<PathBuf>> {
    let base_wads = dir.join("wads").join("base_wads");
    let wad = base_wads.join(FREEDOOM_FILES[0].1);
    if wad.exists() {
        journal::tell(
            "skip",
            &[
                ("what", "Freedoom 2"),
                ("place", &wad.display().to_string()),
            ],
            &format!(
                "Freedoom 2: {} is there already, so it is left alone",
                wad.display()
            ),
        );
        return Ok(None);
    }
    let file = fetch(&FREEDOOM, scratch)?;
    note!(
        "taking {} and its license out of {}",
        FREEDOOM_FILES[0].1,
        FREEDOOM.name
    );
    fs::create_dir_all(&base_wads)
        .map_err(|error| format!("couldn't make {}: {error}", base_wads.display()))?;
    let mut archive = open_zip(&file)?;
    // 📜 the license first, so the wad never arrives without it
    for (inside, name) in FREEDOOM_FILES.iter().rev() {
        let mut entry = archive
            .by_name(inside)
            .map_err(|error| format!("{inside} isn't in {}: {error}", FREEDOOM.name))?;
        write_whole(&mut entry, &base_wads.join(name))?;
    }
    Ok(Some(wad))
}

/// Downloads `download` into the scratch folder, checking its size and hash on the way. Returns
/// where it is.
fn fetch(download: &Download, scratch: &Scratch) -> Outcome<PathBuf> {
    let path = scratch.dir()?.join(download.name);
    journal::tell(
        "download",
        &[
            ("name", download.name),
            ("size", &journal::size(download.size)),
        ],
        &format!(
            "Downloading {} ({})",
            download.name,
            journal::size(download.size)
        ),
    );
    note!(
        "downloading {} ({} bytes, BLAKE3 {}) to {}",
        download.url,
        download.size,
        download.blake3,
        path.display()
    );
    let failed = |error: &dyn std::fmt::Display| format!("downloading {}: {error}", download.url);
    let mut reader = ureq::get(download.url)
        .call()
        .map_err(|error| failed(&error))?
        .into_body()
        .into_reader();
    let mut file = fs::File::create(&path).map_err(|error| failed(&error))?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0; 256 * 1024];
    let mut total = 0;
    let mut shown = u64::MAX;
    loop {
        let read = reader.read(&mut buf).map_err(|error| failed(&error))?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > download.size {
            return Err(failed(&"it is bigger than it should be"));
        }
        hasher.update(&buf[..read]);
        file.write_all(&buf[..read])
            .map_err(|error| failed(&error))?;
        let percent = total * 100 / download.size;
        if percent != shown {
            shown = percent;
            eprint!("\r{}: {percent:>3}%", download.name);
        }
    }
    eprintln!();
    file.sync_all().map_err(|error| failed(&error))?;
    if total != download.size {
        return Err(failed(&"it ended early"));
    }
    let hash = hasher.finalize().to_hex();
    if hash.as_str() != download.blake3 {
        note!("{} came with BLAKE3 {hash}", download.name);
        journal::tell(
            "corrupted",
            &[("name", download.name)],
            &format!("{} isn't the file it should be", download.name),
        );
        return Err(failed(&"it isn't the file it should be (its hash differs)"));
    }
    note!(
        "{}: {total} bytes, and its hash is the one it should be",
        download.name
    );
    journal::tell(
        "verified",
        &[("name", download.name)],
        &format!("{}: checked", download.name),
    );
    Ok(path)
}

fn open_zip(file: &Path) -> Outcome<zip::ZipArchive<fs::File>> {
    let opened = fs::File::open(file).map_err(|error| format!("{}: {error}", file.display()))?;
    zip::ZipArchive::new(opened).map_err(|error| format!("{}: {error}", file.display()))
}

/// Unpacks every file in the zip at `file` into `folder`. A name that would land outside it is
/// refused.
fn unpack_all(file: &Path, folder: &Path) -> Outcome<()> {
    let mut archive = open_zip(file)?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("{}: {error}", file.display()))?;
        let Some(name) = entry.enclosed_name() else {
            return Err(format!(
                "{} has a file named {:?}",
                file.display(),
                entry.name()
            ));
        };
        let to = folder.join(name);
        if entry.is_dir() {
            fs::create_dir_all(&to).map_err(|error| format!("{}: {error}", to.display()))?;
        } else {
            write_whole(&mut entry, &to)?;
        }
    }
    Ok(())
}

/// Writes all of `from` to `to`, by way of `<to>.part`, so a file cut short is never left as if it
/// were whole.
fn write_whole(from: &mut impl Read, to: &Path) -> Outcome<()> {
    let problem = |error: io::Error| format!("{}: {error}", to.display());
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(problem)?;
    }
    let mut part = to.as_os_str().to_os_string();
    part.push(".part");
    let part = PathBuf::from(part);
    let written = (|| {
        let mut file = fs::File::create(&part)?;
        let bytes = io::copy(from, &mut file)?;
        file.sync_all()?;
        fs::rename(&part, to)?;
        Ok::<u64, io::Error>(bytes)
    })();
    match written {
        Ok(bytes) => note!("wrote {} ({bytes} bytes)", to.display()),
        Err(error) => {
            let _ = fs::remove_file(&part);
            return Err(problem(error));
        }
    }
    Ok(())
}

/// A folder for the downloads, in the system's temporary folder rather than next to tdl, made
/// the first time it is needed and deleted with everything in it at the end.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("tdl-install-{}", std::process::id())))
    }

    fn dir(&self) -> Outcome<&Path> {
        fs::create_dir_all(&self.0)
            .map_err(|error| format!("couldn't make {}: {error}", self.0.display()))?;
        Ok(&self.0)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
