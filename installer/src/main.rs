//! `install-tdl`: installs tdl, with UZDoom and Freedoom 2 beside it so a new player has something
//! to play, and sets tdl up to use them. Built with installation-groans, the installers' library.
//!
//! `--locally` (in place, where this program is, or `--from`) or `--user-wide` (copied to
//! `~/.local/opt/tdl`, `%LOCALAPPDATA%\Programs\tdl` on Windows, with the `tdl` command on your
//! PATH) says where, and one of them is needed. tdl is the payload; everything else it makes goes
//! in the artifacts, `artifacts/` beside tdl (or a project's `published/artifacts/` when it runs in
//! place from its `published/releases/`, or `--artifacts`):
//!
//! ```text
//! artifacts/uzdoom/            UZDoom's own folder: the Windows release unpacked, or the AppImage
//! artifacts/wads/base_wads/    freedoom2.wad, with Freedoom's license
//! artifacts/setup.toml         what `tdl setup` keeps
//! artifacts/install.log        the journal
//! ```
//!
//! Each download is a pinned release, checked against its size and BLAKE3 hash before anything is
//! unpacked. Then UZDoom and the base wad are written in `artifacts/setup.toml` (kept with
//! whatever else is in it: install-nacht's part) and kept as tdl's by running `tdl setup` on it,
//! once (`--no-setup` leaves it out); and tdl opens, unless `--no-play`.

use installation_groans::{Installation, toml};

const TDL: Installation = Installation {
    git: Some("https://github.com/error-error-fruit-user/tdl.git"),
    // (the latest release: there is no pinning it before it is made)
    download: Some(if cfg!(windows) {
        "https://github.com/error-error-fruit-user/tdl/releases/latest/download/tdl.exe"
    } else {
        "https://github.com/error-error-fruit-user/tdl/releases/latest/download/tdl.AppImage"
    }),
    ..Installation::default()
};

const UZDOOM: Installation = if cfg!(windows) {
    Installation {
        name: "UZDoom",
        version: "5.0.3",
        download: Some(
            "https://github.com/UZDoom/UZDoom/releases/download/5.0.3/Windows-UZDoom-Release-x86_64.zip",
        ),
        size: 29_337_585,
        blake3: Some("408b8017240488e648bbf5a3b99f05105f224b5960e31531ef626461f150baf1"),
        file: "uzdoom.exe",
        ..Installation::default()
    }
} else {
    Installation {
        name: "UZDoom",
        version: "5.0.3",
        download: Some(
            "https://github.com/UZDoom/UZDoom/releases/download/5.0.3/Linux-UZDoom-Release-x86_64.AppImage",
        ),
        size: 34_560_504,
        blake3: Some("0f1d5177e2bf034b535ddf3fcee5755852378ef44533538b56c17e313a3b802d"),
        file: "uzdoom.AppImage",
        executable: true,
        ..Installation::default()
    }
};

/// Freedoom 0.13.0, whose zip holds both Freedoom wads; only Phase 2 is taken. (Its SHA-256
/// matches the project's signed `freedoom-0.13.0-CHECKSUM`.)
const FREEDOOM: Installation = Installation {
    name: "Freedoom 2",
    version: "0.13.0",
    download: Some(
        "https://github.com/freedoom/freedoom/releases/download/v0.13.0/freedoom-0.13.0.zip",
    ),
    size: 24_143_781,
    blake3: Some("5f947833ecce402da78af8f63e9d647191b48f97dfa0ebd16b464548492d9719"),
    into: "wads/base_wads",
    take: &[
        ("freedoom-0.13.0/freedoom2.wad", "freedoom2.wad"),
        ("freedoom-0.13.0/COPYING.txt", "freedoom-COPYING.txt"),
    ],
    ..Installation::default()
};

/// Terminal launcher for UZDoom
#[installation_groans::cli]
struct Cli {
    /// Installs tdl
    #[installation_groans::Release(TDL, "tdl")]
    tdl: (),
    /// Installs UZDoom
    #[installation_groans::Artifact(UZDOOM)]
    uzdoom: bool,
    /// Installs Freedoom
    #[installation_groans::Artifact(FREEDOOM)]
    freedoom: bool,
    /// Keeps UZDoom and Freedoom 2 as tdl's (tdl setup), with a Freedoom profile
    #[installation_groans::On]
    setup: bool,
    /// Opens tdl at the end
    #[installation_groans::On]
    play: bool,
}

#[installation_groans::location(Cli)]
struct Location;

fn main() {
    installation_groans::run(|terminal| {
        let mut cli = Cli::new();
        if let Some(location) = installation_groans::prompt_user(terminal, &mut cli) {
            location.install(terminal, &cli);
            let artifacts = &location.artifacts;
            if cli.setup && (artifacts.uzdoom.exists() || artifacts.freedoom.exists()) {
                installation_groans::journal::section("setup");
                if write_setup(&location) {
                    location
                        .releases
                        .tdl
                        .run_with(&[&"setup", &location.artifact("setup.toml")]);
                }
            }
            if cli.play {
                location.releases.tdl.run_alias();
            }
        }
    });
}

/// `setup.toml` in the artifacts, for `tdl setup`: UZDoom and Freedoom 2 as tdl's, and a Freedoom
/// profile unless it has one; whatever else it says is kept (install-nacht's part among it), and
/// one that isn't TOML is never written over. Returns whether it was written.
fn write_setup(location: &Location) -> bool {
    let path = location.artifact("setup.toml");
    let mut setup = match std::fs::read_to_string(&path) {
        Ok(text) => match text.parse::<toml::Table>() {
            Ok(setup) => setup,
            Err(problem) => {
                installation_groans::fail(&format!(
                    "{} is there and isn't TOML, so it is left alone: {problem}",
                    path.display()
                ));
                return false;
            }
        },
        Err(_) => toml::Table::new(),
    };
    let artifacts = &location.artifacts;
    if artifacts.uzdoom.exists() {
        setup.insert("uzdoom".into(), artifacts.uzdoom.relative().into());
    }
    if artifacts.freedoom.exists() {
        setup.insert("base_wad".into(), artifacts.freedoom.relative().into());
        // 🎮 Freedoom as a profile of its own, ready to play
        let profiles = setup
            .entry("profiles")
            .or_insert_with(|| toml::Value::Array(Vec::new()));
        if let Some(profiles) = profiles.as_array_mut()
            && !profiles
                .iter()
                .any(|profile| profile.get("name").and_then(toml::Value::as_str) == Some("Freedoom"))
        {
            let mut profile = toml::Table::new();
            profile.insert("name".into(), "Freedoom".into());
            profile.insert("wads".into(), toml::Value::Array(Vec::new()));
            profile.insert("base_wad".into(), artifacts.freedoom.relative().into());
            profiles.push(toml::Value::Table(profile));
        }
    }
    let text = format!(
        "# What tdl keeps, with no windows: `tdl setup` reads this (see tdl setup --help).\n\
         # Paths are relative to this folder.\n{}",
        toml::to_string(&setup).unwrap_or_default()
    );
    match std::fs::write(&path, text) {
        Ok(()) => true,
        Err(problem) => {
            installation_groans::fail(&format!("{}: {problem}", path.display()));
            false
        }
    }
}
