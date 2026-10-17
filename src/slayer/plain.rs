//! `tdl host --headless` and `tdl join --headless`: the same as hosting and joining from the
//! launcher, but with no windows at all, only plain lines in the terminal. The one question asked
//! is whether to download wads that aren't installed; anything else that would be a question is
//! said and gone along with, and anything that stops the game is an error, which ends tdl with a
//! failure.

use std::{
    ffi::OsStr,
    io::{self, BufRead, IsTerminal, Write},
    ops::Deref,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use ratatui::crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal,
};

use crate::{
    cli::HostOverrides,
    error::{Error, INTERRUPTED_EXIT, Result as TdlResult},
    layout::show_size,
    net::{self, Address, Manifest},
    slayer::{
        join_game::{fill_gaps, first_match},
        share,
    },
    tally::Tally,
    uzdoom::{self, Game},
    vault::{
        Vault, edition::Edition, global::Shared, host_settings::HostSettings, lookup::Local,
        profile::Profile, scroll::Scroll, wad::Wad,
    },
};

/// What went wrong, said to the user as it is.
pub type Outcome = std::result::Result<(), String>;

/// The keyboard while a game or a download is going, in raw mode for as long as this is held, so
/// Ctrl+C arrives as a key to look for, just as with windows, rather than as a signal that would
/// end tdl and leave the game running. Other keys are thrown away. With no terminal there are no
/// keys, and nothing to look for. Before and after, Ctrl+C simply ends tdl.
struct Keys {
    raw: bool,
}

impl Keys {
    fn take() -> Self {
        Keys {
            raw: terminal::enable_raw_mode().is_ok(),
        }
    }

    /// Whether Ctrl+C has been pressed.
    fn interrupted(&self) -> bool {
        while self.raw && event::poll(Duration::ZERO).unwrap_or(false) {
            if let Ok(Event::Key(key)) = event::read()
                && key.is_press()
                && key.code == KeyCode::Char('c')
                && key.modifiers.contains(KeyModifiers::CONTROL)
            {
                return true;
            }
        }
        false
    }

    /// Prints `line`: raw mode doesn't go back to the start of the line by itself.
    fn say(&self, line: &str) {
        if self.raw {
            print!("{line}\r\n");
            let _ = io::stdout().flush();
        } else {
            println!("{line}");
        }
    }

    /// Gives the terminal back as it was and ends tdl, saying what Ctrl+C says with windows.
    fn end_interrupted(self) -> ! {
        drop(self);
        eprintln!("{}", Error::Interrupted);
        std::process::exit(INTERRUPTED_EXIT);
    }
}

impl Drop for Keys {
    fn drop(&mut self) {
        if self.raw {
            let _ = terminal::disable_raw_mode();
        }
    }
}

/// Waits for `game` to end, or stops it if Ctrl+C is pressed (and then ends tdl).
fn wait_for(mut game: Game) -> Outcome {
    let keys = Keys::take();
    loop {
        if game.has_quit().map_err(|error| error.to_string())? {
            drop(keys);
            return game.wait().map_err(|error| error.to_string());
        }
        if keys.interrupted() {
            let _ = game.stop();
            keys.end_interrupted();
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Hosts the profile at `index` with its host defaults, changed by `overrides`, and returns when
/// the game ends.
pub fn host(vault: &mut Vault, index: usize, overrides: &HostOverrides) -> Outcome {
    let profile = vault.profiles[index].clone();
    let defaults = profile
        .host_defaults
        .clone()
        .unwrap_or_else(|| HostSettings::new(vault.config.host_port));
    let settings = overrides.apply(defaults);
    let port = settings
        .port()
        .ok_or("The port must be a number from 1 to 65535")?;
    let uzdoom = ready_to_play(vault, &profile)?;
    // 💾 only a port other than the default is worth remembering, as the host form does
    let remembered = (port != net::DEFAULT_PORT).then_some(port);
    if vault.config.host_port != remembered {
        vault.config.host_port = remembered;
        if vault
            .config
            .write(&vault.config_dir, &vault.profiles)
            .is_err()
        {
            eprintln!("Could not save the port for next time, is the disk full?");
        }
    }
    // 🌀 volatile wads are hashed fresh, so what is served is what is described
    let wads: Vec<Wad> = profile
        .wads
        .iter()
        .map(|wad| wad.refreshed(&mut ()))
        .collect::<Result<_, _>>()
        .map_err(|_| "One of the profile's wads can't be read")?;
    let base_wad = profile.base_wad(&vault.config);
    let manifest = Manifest::build(&wads, base_wad.map(|w| w.hash.clone()).unwrap_or_default())
        .map_err(|_| "One of the profile's wads can't be read")?;
    let code = settings.code();
    let server = manifest
        .serve(
            port,
            code.clone(),
            wads.iter().map(|w| w.path.clone()).collect(),
        )
        .map_err(|_| format!("Could not serve the wads on port {port}: is it in use?"))?;
    println!(
        "Hosting {}: players join with port {}, code {code}",
        profile.name, server.port
    );
    let game = uzdoom::host(
        &uzdoom,
        base_wad.map(Wad::deref),
        &profile,
        &settings,
        profile.uzdoom_args(&vault.config),
        &vault.data_dir.join(&profile.name),
    )
    .map_err(|error| error.to_string())?;
    let ended = wait_for(game);
    // 🔌 the wads are served for exactly as long as the game runs
    drop(server);
    ended?;
    println!("The game is over");
    Ok(())
}

/// Joins the game at `address` with the first profile that matches the host's wads, or a new one
/// made from the wads already here and downloads of the rest (after asking, unless
/// `always_download`), and returns when the game ends.
pub fn join(vault: &mut Vault, address: &Address, always_download: bool) -> Outcome {
    let where_to = format!("{}:{}", address.host, address.port);
    let manifest = match Manifest::fetch(address) {
        Ok(manifest) => manifest,
        Err(Error::Refused) => return Err(format!("The host at {where_to} refused the join code")),
        Err(_) => return Err(format!("Couldn't reach a host at {where_to}")),
    };
    let profile = match vault
        .profiles
        .iter()
        .find(|profile| manifest.is_compatible_with(profile))
    {
        Some(profile) => {
            println!("Joining with {}", profile.name);
            profile.clone()
        }
        None => match make_profile(vault, address, &manifest, always_download)? {
            Some(profile) => profile,
            None => {
                println!("Not joining");
                return Ok(());
            }
        },
    };
    let uzdoom = ready_to_play(vault, &profile)?;
    // 🌀 the questions the windows would ask are said instead, and gone along with
    let differing = manifest.differing_wads(&profile);
    if !differing.is_empty() {
        eprintln!(
            "These volatile wads differ from the host's, joining anyway: {}",
            differing.join(", ")
        );
    }
    let base_wad = profile.base_wad(&vault.config);
    if !manifest.base_wad_matches(base_wad) {
        eprintln!("Your base wad isn't the host's, joining anyway");
    }
    let game = uzdoom::join(
        &uzdoom,
        base_wad.map(Wad::deref),
        &profile,
        &address.host,
        profile.uzdoom_args(&vault.config),
        &vault.data_dir.join(&profile.name),
    )
    .map_err(|error| error.to_string())?;
    wait_for(game)?;
    println!("The game is over");
    Ok(())
}

/// The UZDoom to play `profile` with, if it has one and nothing it needs is broken.
fn ready_to_play(
    vault: &Vault,
    profile: &Profile,
) -> std::result::Result<std::ffi::OsString, String> {
    let uzdoom = profile
        .uzdoom(&vault.config)
        .cloned()
        .ok_or("No UZDoom is selected: pick one in tdl with U, or pass --uzdoom")?;
    let broken: Vec<_> = profile
        .faults(&vault.config)
        .into_iter()
        .filter(|(_, fault)| fault.is_blocking())
        .collect();
    if !broken.is_empty() {
        return Err(format!(
            "'{}' can't run: {} of the files it needs {}. Open it in tdl to see which",
            profile.name,
            broken.len(),
            if broken.len() == 1 {
                "is broken"
            } else {
                "are broken"
            }
        ));
    }
    Ok(uzdoom)
}

/// A new profile for the host's wads: the ones already here where they are, and the rest
/// downloaded, but only if the user says yes. Named after the host, so it is easy to find and
/// rename in tdl. `None` if the user says no.
fn make_profile(
    vault: &mut Vault,
    address: &Address,
    manifest: &Manifest,
    always_download: bool,
) -> std::result::Result<Option<Profile>, String> {
    if vault.config.uzdoom_in_effect().is_none() {
        return Err("No UZDoom is selected: pick one in tdl with U, or pass --uzdoom".to_string());
    }
    let lookup = vault.lookup();
    let checks: Vec<_> = manifest
        .entries
        .iter()
        .map(|entry| lookup.check(entry))
        .collect();
    let local: Vec<Option<Wad>> = checks
        .into_iter()
        .zip(&manifest.entries)
        .map(|(check, entry)| match check {
            Local::Intact(wad) => Ok(Some(wad)),
            Local::Maybe(paths) => first_match(paths, entry, &mut ()),
        })
        .collect::<Result<_, _>>()
        .map_err(|error| error.to_string())?;
    let wanted: Vec<_> = manifest
        .entries
        .iter()
        .zip(&local)
        .filter(|(_, wad)| wad.is_none())
        .map(|(entry, _)| entry.clone())
        .collect();
    if !wanted.is_empty() {
        let size: u64 = wanted.iter().map(|entry| entry.size).sum();
        let question = format!(
            "{} of {} wads aren't installed ({}). Download them and make a profile?",
            wanted.len(),
            manifest.entries.len(),
            show_size(size)
        );
        if always_download {
            println!("{question} Downloading, as --always-download says");
        } else if !ask(&question) {
            return Ok(None);
        }
    }
    let downloaded = if wanted.is_empty() {
        Vec::new()
    } else {
        let mut lines = Lines(Keys::take());
        match net::download(address, &wanted, &vault.downloads_dir(), &mut lines) {
            Ok(wads) => wads,
            // 🧹 the partial file is gone already; finished ones stay, so a retry picks up there
            Err(Error::Cancelled) => lines.0.end_interrupted(),
            Err(error) => return Err(format!("The download failed: {error}")),
        }
    };
    let wads = fill_gaps(local, downloaded).ok_or("Fewer wads arrived than were asked for")?;
    let name = joined_name(&address.host, &vault.profiles);
    let profile = Profile::new(name, wads);
    let mut index = vault.profiles.len();
    vault
        .insert(&mut index, profile.clone())
        .map_err(|_| "Could not save the new profile, is the disk full?")?;
    println!("Made the profile {}", profile.name);
    Ok(Some(profile))
}

/// `tdl --seal`: every profile in `vault`, in the launcher's order, and the global `settings` if
/// there are any, sealed into `scroll` (`.scroll` added if it doesn't end in it). An existing file
/// is never written over. Returns where the scroll went.
pub fn seal(
    vault: &Vault,
    settings: Option<Shared>,
    scroll: &Path,
) -> std::result::Result<PathBuf, String> {
    if vault.profiles.is_empty() {
        return Err("There are no profiles in the vault to seal".to_string());
    }
    let path = share::with_extension(scroll.to_path_buf());
    if path.exists() {
        return Err(format!("There is already a file at {}", path.display()));
    }
    let mut sealed = Scroll::of(&vault.profiles)?;
    let with = if settings.is_some() {
        " and your global settings"
    } else {
        ""
    };
    sealed.settings = settings;
    sealed
        .save(&path)
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    let count = vault.profiles.len();
    let profiles = if count == 1 { "profile" } else { "profiles" };
    println!("Sealed {count} {profiles}{with} in {}", path.display());
    Ok(path)
}

/// The name for a profile made to join `host`: `joined_` and the host with anything a profile
/// name can't hold made `_`. If that is taken, `_v2`, `_v3` and so on go after it: a plain number
/// would read as part of the address (`joined_10_0_0_3` for a second `10.0.0.2`).
fn joined_name(host: &str, profiles: &[Profile]) -> String {
    let host: String = host
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let name = format!("joined_{host}");
    let free = |candidate: &String| profiles.iter().all(|p| &p.name != candidate);
    std::iter::once(name.clone())
        .chain((2..).map(|n| format!("{name}_v{n}")))
        .find(free)
        .expect("there are always more numbers than profiles")
}

/// Asks `question` on the terminal and says whether the answer is yes. Anything but y or yes is
/// no, and so is having no terminal to ask on, so nothing is downloaded unless someone said so.
fn ask(question: &str) -> bool {
    if !io::stdin().is_terminal() {
        eprintln!("{question} No one to ask, so no");
        return false;
    }
    print!("{question} [y/N] ");
    let _ = io::stdout().flush();
    let mut answer = String::new();
    if io::stdin().lock().read_line(&mut answer).is_err() {
        return false;
    }
    is_yes(&answer)
}

/// Whether a typed answer means yes.
fn is_yes(answer: &str) -> bool {
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// A download's progress as plain lines: one per wad as it starts. Ctrl+C cancels it.
struct Lines(Keys);

impl Lines {
    fn check(&self) -> TdlResult<()> {
        if self.0.interrupted() {
            return Err(Error::Cancelled);
        }
        Ok(())
    }
}

impl Tally for Lines {
    fn count_file(&mut self, name: &OsStr, size: u64) -> TdlResult<()> {
        self.check()?;
        let size = show_size(size);
        self.0
            .say(&format!("Downloading {} ({size})", name.to_string_lossy()));
        Ok(())
    }

    /// Stops the download after this chunk if Ctrl+C was pressed.
    fn count_bytes(&mut self, _: u64) -> TdlResult<()> {
        self.check()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        testing,
        vault::{edition::Loaded, scribe::tests::Altar},
    };

    /// The example vault in `altar`.
    fn example(altar: &Altar) -> Vault {
        let root = testing::example_vault(altar);
        Vault::new(root.join("config"), root.join("data"))
    }

    #[test]
    fn sealing_puts_every_profile_and_the_settings_in_a_new_scroll() {
        let altar = Altar::new("plain-seal");
        let vault = example(&altar);
        let settings = Shared::default();
        let path = seal(&vault, Some(settings.clone()), &altar.file("all")).unwrap();
        assert_eq!(path, altar.file("all.scroll"), ".scroll is added");
        let Loaded::Fine(scroll) = Scroll::load(&path) else {
            panic!("the scroll should read back");
        };
        let names: Vec<_> = scroll.profiles.iter().map(|p| p.name.clone()).collect();
        let expected: Vec<_> = vault.profiles.iter().map(|p| p.name.clone()).collect();
        assert_eq!(names, expected);
        assert_eq!(scroll.settings, Some(settings));
        let bare = seal(&vault, None, &altar.file("bare")).unwrap();
        let Loaded::Fine(bare) = Scroll::load(&bare) else {
            panic!("the scroll should read back");
        };
        assert_eq!(bare.settings, None, "settings only go in when asked for");
        // 🚫 never over a file that is already there
        assert!(seal(&vault, None, &path).is_err());
    }

    #[test]
    fn only_y_or_yes_is_yes() {
        for yes in ["y", "Y", "yes", " YES \n"] {
            assert!(is_yes(yes), "{yes:?}");
        }
        for no in ["", "\n", "n", "no", "sure", "yess"] {
            assert!(!is_yes(no), "{no:?}");
        }
    }

    #[test]
    fn a_joined_profile_is_named_after_the_host_and_tagged_if_taken() {
        assert_eq!(joined_name("10.0.0.2", &[]), "joined_10_0_0_2");
        let taken = [Profile::new("joined_10_0_0_2".into(), Vec::new())];
        assert_eq!(joined_name("10.0.0.2", &taken), "joined_10_0_0_2_v2");
    }
}
