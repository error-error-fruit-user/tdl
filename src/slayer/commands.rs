//! `tdl cli`: the launcher's work from the command line, one rite at a time, on the loadout the
//! launcher would open, with no windows. Everything it says goes through the séance in
//! [`ritual`](crate::ritual), in the theme's colors. Playing (`launch`, `host`, `join`) is the
//! `--headless` flows, run by [`Launcher::rite`](crate::screens::launcher::Launcher::rite); the
//! rest is here. Each rite saves what it changes, the way the launcher does, and an `Err` says what
//! stopped it.
//!
//! [`shell`] runs a shell command for the launcher's command line (`sh ...`) and a headless
//! script's `` `sh ... ``, behind guard rails: nothing that needs root or administrator, and
//! nothing that looks made to do harm (see [`forbidden`]). Those are rails, not a sandbox: they
//! stop accidents and the obvious, not someone set on getting around them.

use std::{
    ffi::OsString,
    io::{self, BufRead, IsTerminal, Write},
    path::{Path, PathBuf},
};

use crate::{
    cli::{PoolRite, Rite, WadRite},
    loadout::{
        Loadout,
        edition::Edition,
        profile::{self, Profile},
        scroll::Scroll,
        uac::{Music, SplashQuip},
        wad::{Fault, Wad},
    },
    quips,
    ritual::{aside, murmur, proclaim, wail},
    screens::launcher::find_profile,
    slayer::{share, watch},
    theme::{Dressing, Theme, splash::Splash},
};

/// What stopped a rite, said as it is.
type Outcome = Result<(), String>;

/// Does `rite` on `loadout`. Playing never comes here (see the module's docs).
pub fn perform(loadout: &mut Loadout, rite: Rite) -> Outcome {
    match rite {
        Rite::Profiles => list(loadout),
        Rite::Show { profile } => show(loadout, &profile),
        Rite::New { name, wads } => new(loadout, &name, &wads),
        Rite::Rename { profile, name } => rename(loadout, &profile, &name),
        Rite::Copy { profile, name } => copy(loadout, &profile, name.as_deref()),
        Rite::Delete { profile, yes } => delete(loadout, &profile, yes),
        Rite::Wads(rite) => wads(loadout, rite),
        Rite::Set { profile, changes } => set(loadout, &profile, &changes),
        Rite::Settings { changes } => settings(loadout, &changes),
        Rite::Pools(rite) => pools(loadout, rite),
        Rite::Seal {
            scroll,
            profiles,
            with_settings,
        } => seal(loadout, &scroll, &profiles, with_settings),
        Rite::Unseal {
            scroll,
            with_settings,
            replace,
            yes,
        } => unseal(loadout, &scroll, with_settings, replace, yes),
        Rite::Watch { dir, name } => watch(loadout, &dir, name.as_deref()),
        Rite::Launch { .. } | Rite::Host { .. } | Rite::Join { .. } => {
            Err("launch, host and join are run by the launcher".into())
        }
    }
}

/// Where the profile called `name` is.
fn find(loadout: &Loadout, name: &str) -> Result<usize, String> {
    find_profile(&loadout.profiles, name)
        .ok_or_else(|| format!("There is no profile called '{name}'"))
}

/// What a failed save says.
fn unsaved(error: impl std::fmt::Display) -> String {
    format!("Could not save it: {error}")
}

/// `given` as a new profile's name: cleaned (see [`profile::clean_name`]), and not one another
/// profile has (`except` is the profile being renamed).
fn free_name(loadout: &Loadout, given: &str, except: Option<usize>) -> Result<String, String> {
    let name = profile::clean_name(given).ok_or_else(|| {
        format!(
            "'{given}' can't be a profile's name: letters, digits, _, - and spaces, at most {}, \
             not starting with -",
            profile::MAX_NAME
        )
    })?;
    let taken = loadout
        .profiles
        .iter()
        .enumerate()
        .any(|(index, profile)| Some(index) != except && profile.name == name);
    if taken {
        return Err(format!("There is a profile called '{name}' already"));
    }
    Ok(name)
}

/// The wad for the file (or mod folder) at `path`, hashed.
fn wad_at(path: &Path) -> Result<Wad, String> {
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    if path.is_dir() {
        return Ok(Wad::folder(&path));
    }
    if !path.is_file() {
        return Err(format!("There is no file at {}", path.display()));
    }
    aside(format!("hashing {}", path.display()));
    Wad::hashed(&path, &mut ())
        .map_err(|error| format!("Could not read {}: {error}", path.display()))
}

/// Which of `profile`'s wads `given` means: its number in the load order (from 1), or its file
/// name (case doesn't matter if only one fits).
fn which_wad(profile: &Profile, given: &str) -> Result<usize, String> {
    if let Ok(number) = given.parse::<usize>() {
        return (1..=profile.wads.len())
            .contains(&number)
            .then(|| number - 1)
            .ok_or_else(|| format!("'{}' has no wad {number}", profile.name));
    }
    let names: Vec<String> = profile
        .wads
        .iter()
        .map(|wad| wad.file_name().into_owned())
        .collect();
    if let Some(exact) = names.iter().position(|name| name == given) {
        return Ok(exact);
    }
    let mut alike = names
        .iter()
        .enumerate()
        .filter(|(_, name)| name.eq_ignore_ascii_case(given));
    match (alike.next(), alike.next()) {
        (Some((index, _)), None) => Ok(index),
        _ => Err(format!("'{}' has no wad called '{given}'", profile.name)),
    }
}

/// What is wrong with a wad, in a few words, if anything.
fn fault(wad: &Wad) -> Option<String> {
    Some(match wad.check()? {
        Fault::Missing => "missing".into(),
        Fault::NotAFile => "not a file".into(),
        Fault::NotAFolder => "not a folder".into(),
        Fault::Unreadable(why) => format!("unreadable: {why}"),
        Fault::Changed { .. } => "changed".into(),
    })
}

/// Asks `question` on the terminal. With no terminal to ask on, the answer is no, and so is
/// anything but y or yes.
fn ask(question: &str) -> bool {
    // 🫣 from the launcher's command line, the terminal is the launcher's
    if crate::ritual::curtained() {
        wail(format!("{question} Add --yes to say yes from here"));
        return false;
    }
    if !io::stdin().is_terminal() {
        wail(format!("{question} No one to ask, so no (--yes says yes)"));
        return false;
    }
    crate::ritual::hush();
    print!("{question} [y/n] ");
    let _ = io::stdout().flush();
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer).is_ok()
        && matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// A path as it is shown.
fn shown(path: &OsString) -> String {
    Path::new(path).display().to_string()
}

fn list(loadout: &Loadout) -> Outcome {
    if loadout.profiles.is_empty() {
        murmur("There are no profiles yet: tdl cli new NAME [WAD...] makes one");
        return Ok(());
    }
    proclaim(format!("{} profiles", loadout.profiles.len()));
    let width = loadout
        .profiles
        .iter()
        .map(|p| p.name.chars().count())
        .max()
        .unwrap_or(0);
    for profile in &loadout.profiles {
        let mut notes = vec![format!(
            "{} wad{}",
            profile.wads.len(),
            if profile.wads.len() == 1 { "" } else { "s" }
        )];
        let faults = profile
            .wads
            .iter()
            .filter(|wad| wad.check().is_some())
            .count();
        if faults > 0 {
            notes.push(format!("{faults} not right"));
        }
        let volatile = profile.wads.iter().filter(|wad| wad.volatile).count();
        if volatile > 0 {
            notes.push(format!("{volatile} volatile"));
        }
        if let Some(dir) = &profile.watched {
            notes.push(format!("watches {}", shown(dir)));
        }
        if profile
            .uzdoom
            .as_ref()
            .is_some_and(|uzdoom| !Path::new(uzdoom).is_file())
        {
            notes.push("its UZDoom is missing".into());
        }
        murmur(format!("  {:<width$}  {}", profile.name, notes.join(" · ")));
    }
    Ok(())
}

fn show(loadout: &Loadout, name: &str) -> Outcome {
    let profile = &loadout.profiles[find(loadout, name)?];
    proclaim(&profile.name);
    if profile.wads.is_empty() {
        aside("  no wads");
    }
    for (number, wad) in profile.wads.iter().enumerate() {
        let mut notes = Vec::new();
        if wad.folder {
            notes.push("mod folder".to_string());
        }
        if wad.volatile {
            notes.push("volatile".into());
        }
        notes.extend(fault(wad));
        let notes = if notes.is_empty() {
            String::new()
        } else {
            format!("  ({})", notes.join(", "))
        };
        murmur(format!("  {:>2}. {}{notes}", number + 1, shown(&wad.path)));
    }
    let uac = &loadout.uac;
    let own_or_global = |own: Option<String>, global: Option<String>| match (own, global) {
        (Some(own), _) => own,
        (None, Some(global)) => format!("{global} (global)"),
        (None, None) => "none".into(),
    };
    murmur(format!(
        "  uzdoom    {}",
        own_or_global(
            profile.uzdoom.as_ref().map(shown),
            uac.uzdoom_in_effect().map(shown)
        )
    ));
    murmur(format!(
        "  base wad  {}",
        own_or_global(
            profile.base_wad.as_ref().map(|wad| shown(&wad.path)),
            uac.base_wad_in_effect().map(|wad| shown(&wad.path))
        )
    ));
    murmur(format!(
        "  ini       {}",
        own_or_global(
            profile.ini.as_ref().map(shown),
            uac.ini_in_effect().map(shown)
        )
    ));
    murmur(format!(
        "  args      {}",
        own_or_global(
            profile.uzdoom_args.clone(),
            Some(uac.uzdoom_args.clone()).filter(|args| !args.is_empty())
        )
    ));
    if let Some(dir) = &profile.watched {
        murmur(format!("  watches   {}", shown(dir)));
    }
    Ok(())
}

fn new(loadout: &mut Loadout, name: &str, wads: &[PathBuf]) -> Outcome {
    let name = free_name(loadout, name, None)?;
    let wads = wads
        .iter()
        .map(|path| wad_at(path))
        .collect::<Result<Vec<_>, _>>()?;
    let count = wads.len();
    let mut index = loadout.profiles.len();
    loadout
        .insert(&mut index, Profile::new(name.clone(), wads))
        .map_err(unsaved)?;
    murmur(format!(
        "Made {name}, with {count} wad{}",
        if count == 1 { "" } else { "s" }
    ));
    Ok(())
}

fn rename(loadout: &mut Loadout, name: &str, to: &str) -> Outcome {
    let index = find(loadout, name)?;
    let to = free_name(loadout, to, Some(index))?;
    let old = loadout.profiles[index].name.clone();
    let profile = Profile {
        name: to.clone(),
        ..loadout.profiles[index].clone()
    };
    loadout.replace(index, profile).map_err(unsaved)?;
    murmur(format!("Renamed {old} to {to}"));
    Ok(())
}

fn copy(loadout: &mut Loadout, name: &str, to: Option<&str>) -> Outcome {
    let index = find(loadout, name)?;
    let original = loadout.profiles[index].clone();
    let to = match to {
        Some(to) => free_name(loadout, to, None)?,
        None => profile::tagged_name(&original.name, &loadout.profiles),
    };
    let copy = original
        .copied_as(to.clone(), &loadout.data_dir)
        .map_err(unsaved)?;
    let mut after = index + 1;
    loadout.insert(&mut after, copy).map_err(unsaved)?;
    murmur(format!("Copied {} as {to}", original.name));
    Ok(())
}

fn delete(loadout: &mut Loadout, name: &str, yes: bool) -> Outcome {
    let mut index = find(loadout, name)?;
    let name = loadout.profiles[index].name.clone();
    if !yes && !ask(&format!("Delete {name} and its saves?")) {
        murmur("Nothing was deleted");
        return Ok(());
    }
    loadout.remove(&mut index).map_err(unsaved)?;
    murmur(format!("Deleted {name}"));
    Ok(())
}

fn wads(loadout: &mut Loadout, rite: WadRite) -> Outcome {
    let name = match &rite {
        WadRite::Add { profile, .. }
        | WadRite::Remove { profile, .. }
        | WadRite::Move { profile, .. }
        | WadRite::Replace { profile, .. }
        | WadRite::Volatile { profile, .. } => profile.clone(),
    };
    let index = find(loadout, &name)?;
    let before = loadout.profiles[index].clone();
    let mut profile = before.clone();
    let said = match rite {
        WadRite::Add { wads, at, .. } => {
            let added = wads
                .iter()
                .map(|path| wad_at(path))
                .collect::<Result<Vec<_>, _>>()?;
            let at = match at {
                Some(at) if (1..=profile.wads.len() + 1).contains(&at) => at - 1,
                Some(at) => return Err(format!("{} has no place {at}", profile.name)),
                None => profile.wads.len(),
            };
            let count = added.len();
            profile.wads.splice(at..at, added);
            format!(
                "Added {count} wad{} to {}",
                if count == 1 { "" } else { "s" },
                profile.name
            )
        }
        WadRite::Remove { wads, .. } => {
            let mut gone = wads
                .iter()
                .map(|given| which_wad(&profile, given))
                .collect::<Result<Vec<_>, _>>()?;
            gone.sort_unstable();
            gone.dedup();
            for at in gone.iter().rev() {
                profile.wads.remove(*at);
            }
            format!(
                "Took {} wad{} out of {}",
                gone.len(),
                if gone.len() == 1 { "" } else { "s" },
                profile.name
            )
        }
        WadRite::Move { wad, to, .. } => {
            let from = which_wad(&profile, &wad)?;
            if !(1..=profile.wads.len()).contains(&to) {
                return Err(format!("{} has no place {to}", profile.name));
            }
            let moved = profile.wads.remove(from);
            let file = moved.file_name().into_owned();
            profile.wads.insert(to - 1, moved);
            format!("Moved {file} to place {to} in {}", profile.name)
        }
        WadRite::Replace { wad, file, .. } => {
            let at = which_wad(&profile, &wad)?;
            let replacement = wad_at(&file)?;
            let old = profile.wads[at].file_name().into_owned();
            profile.wads[at] = Wad {
                volatile: profile.wads[at].volatile,
                ..replacement
            };
            format!(
                "Put {} in place of {old} in {}",
                file.display(),
                profile.name
            )
        }
        WadRite::Volatile { wad, off, .. } => {
            let at = which_wad(&profile, &wad)?;
            if profile.wads[at].folder {
                return Err("A mod folder has no hash to hold to, so it can't be volatile".into());
            }
            profile.wads[at].volatile = !off;
            format!(
                "{} is {}volatile in {}",
                profile.wads[at].file_name(),
                if off { "no longer " } else { "" },
                profile.name
            )
        }
    };
    // 🙈 a wad taken out of a watched dir's profile stays out, as in the editor
    profile.note_dismissed(&before);
    loadout.replace(index, profile).map_err(unsaved)?;
    murmur(said);
    Ok(())
}

/// `changes` as NAME VALUE pairs, each NAME one of `names`, in the order given.
fn pairs<'a>(changes: &'a [String], names: &[&str]) -> Result<Vec<(&'a str, &'a str)>, String> {
    if changes.len() % 2 == 1 {
        return Err(format!(
            "{} has no value after it",
            changes[changes.len() - 1]
        ));
    }
    changes
        .chunks(2)
        .map(|pair| {
            let (name, value) = (pair[0].as_str(), pair[1].as_str());
            if names.contains(&name) {
                Ok((name, value))
            } else {
                Err(format!("{name} isn't one of {}", names.join(", ")))
            }
        })
        .collect()
}

fn set(loadout: &mut Loadout, name: &str, changes: &[String]) -> Outcome {
    if changes.is_empty() {
        return show(loadout, name);
    }
    let changes = pairs(changes, &["uzdoom", "base-wad", "ini", "args"])?;
    let index = find(loadout, name)?;
    let mut profile = loadout.profiles[index].clone();
    let global = |given: &str| given.eq_ignore_ascii_case("global");
    let file = |given: &str| -> Result<OsString, String> {
        let path = std::path::absolute(given).unwrap_or_else(|_| given.into());
        if !path.is_file() {
            return Err(format!("There is no file at {}", path.display()));
        }
        Ok(path.into_os_string())
    };
    for (name, value) in changes {
        match name {
            "uzdoom" => {
                profile.uzdoom = if global(value) {
                    None
                } else {
                    Some(file(value)?)
                }
            }
            "ini" => {
                profile.ini = if global(value) {
                    None
                } else {
                    Some(file(value)?)
                }
            }
            "base-wad" => {
                profile.base_wad = if global(value) {
                    None
                } else {
                    Some(wad_at(Path::new(value))?)
                }
            }
            _ => {
                let value = value.trim();
                profile.uzdoom_args =
                    (!global(value) && !value.is_empty()).then(|| value.to_string());
            }
        }
    }
    let name = profile.name.clone();
    loadout.replace(index, profile).map_err(unsaved)?;
    murmur(format!("Set {name}'s own"));
    show(loadout, &name)
}

fn settings(loadout: &mut Loadout, changes: &[String]) -> Outcome {
    if changes.is_empty() {
        return show_settings(loadout);
    }
    let changes = pairs(
        changes,
        &[
            "uzdoom",
            "base-wad",
            "ini",
            "args",
            "theme",
            "splash",
            "splash-screen",
            "splash-quip",
            "quips",
            "music",
            "song",
            "pride",
        ],
    )?;
    if !loadout.uac_writable {
        return Err("The settings are from a newer tdl, so they are never written over".into());
    }
    let config_dir = loadout
        .uac_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let uac = &mut loadout.uac;
    let file = |given: &str| -> Result<OsString, String> {
        let path = std::path::absolute(given).unwrap_or_else(|_| given.into());
        if !path.is_file() {
            return Err(format!("There is no file at {}", path.display()));
        }
        Ok(path.into_os_string())
    };
    let none = |given: &str| given.eq_ignore_ascii_case("none");
    let default = |given: &str| given.eq_ignore_ascii_case("default");
    let on = |name: &str, given: &str| match given.to_ascii_lowercase().as_str() {
        "on" | "yes" | "true" => Ok(true),
        "off" | "no" | "false" => Ok(false),
        _ => Err(format!("{name} is on or off, not {given}")),
    };
    for (name, value) in changes {
        match name {
            "uzdoom" => uac.uzdoom = Some(file(value)?),
            "base-wad" => uac.base_wad = Some(wad_at(Path::new(value))?),
            "ini" => {
                uac.ini = if none(value) {
                    None
                } else {
                    Some(file(value)?)
                }
            }
            "args" => uac.uzdoom_args = value.trim().to_string(),
            // 👗 a theme or a splash is a name or a file, and must be one that can be worn
            "theme" => {
                uac.theme = if default(value) {
                    None
                } else {
                    Theme::load(OsString::from(value).as_os_str(), &config_dir)
                        .map_err(|problem| format!("theme: {problem}"))?;
                    Some(value.into())
                }
            }
            "splash" => {
                uac.splash = if default(value) {
                    None
                } else {
                    Splash::load(OsString::from(value).as_os_str(), &config_dir)
                        .map_err(|problem| format!("splash: {problem}"))?;
                    Some(value.into())
                }
            }
            "splash-screen" => uac.skip_splash = !on(name, value)?,
            "splash-quip" => {
                uac.splash_quip = if value.eq_ignore_ascii_case("random") {
                    SplashQuip::Random
                } else {
                    quips::expand(value).map_err(|problem| format!("splash-quip: {problem}"))?;
                    SplashQuip::Custom(value.to_string())
                }
            }
            "quips" => uac.no_quips = !on(name, value)?,
            "music" => {
                uac.music = match value {
                    "off" => Music::Off,
                    "splash" => Music::Splash,
                    "kept" => Music::Kept,
                    _ => return Err(format!("music is off, splash or kept, not {value}")),
                }
            }
            "song" => {
                uac.song = if none(value) {
                    None
                } else {
                    crate::music::use_song(Path::new(value))
                        .map_err(|problem| format!("song: {problem}"))?;
                    let song = file(value)?;
                    if !uac.songs.contains(&song) {
                        uac.songs.push(song.clone());
                    }
                    Some(song)
                }
            }
            _ => uac.pride = on(name, value)?,
        }
    }
    loadout.keep_uac().map_err(unsaved)?;
    murmur("Kept the settings");
    show_settings(loadout)
}

fn show_settings(loadout: &Loadout) -> Outcome {
    let uac = &loadout.uac;
    let or = |value: Option<String>, otherwise: &str| value.unwrap_or_else(|| otherwise.into());
    let on = |on: bool| if on { "on" } else { "off" };
    proclaim(if loadout.config.own_settings {
        "This loadout's own settings"
    } else {
        "The global settings"
    });
    let lines = [
        ("uzdoom", or(uac.uzdoom.as_ref().map(shown), "none")),
        (
            "base wad",
            or(uac.base_wad.as_ref().map(|wad| shown(&wad.path)), "none"),
        ),
        ("ini", or(uac.ini.as_ref().map(shown), "UZDoom's own")),
        (
            "args",
            Some(uac.uzdoom_args.clone())
                .filter(|args| !args.is_empty())
                .unwrap_or("none".into()),
        ),
        ("theme", or(uac.theme.as_ref().map(shown), "default")),
        ("splash", or(uac.splash.as_ref().map(shown), "default")),
        ("splash screen", on(!uac.skip_splash).into()),
        (
            "splash quip",
            match &uac.splash_quip {
                SplashQuip::Random => "random".into(),
                SplashQuip::Custom(words) => words.clone(),
            },
        ),
        ("quips", on(!uac.no_quips).into()),
        (
            "music",
            match uac.music {
                Music::Off => "off",
                Music::Splash => "on the splash",
                Music::Kept => "kept playing",
            }
            .into(),
        ),
        (
            "song",
            or(uac.song.as_ref().map(shown), "the built-in track"),
        ),
        ("pride", on(uac.pride).into()),
    ];
    for (name, value) in lines {
        murmur(format!("  {name:<14}{value}"));
    }
    for (place, song) in uac.songs.iter().enumerate() {
        let name = if place == 0 { "songs known" } else { "" };
        murmur(format!("  {name:<14}{}", shown(song)));
    }
    Ok(())
}

fn pools(loadout: &mut Loadout, rite: PoolRite) -> Outcome {
    let dir_of = |dir: &Path| {
        std::path::absolute(dir)
            .unwrap_or_else(|_| dir.to_path_buf())
            .into_os_string()
    };
    match rite {
        PoolRite::List => {
            proclaim("Wad pools");
            // 📂 in the wads screen's order, saying which it has closed
            let shelf = loadout.shelf();
            let global: Vec<PathBuf> = loadout.uac.wad_dirs.iter().map(PathBuf::from).collect();
            let here: Vec<PathBuf> = loadout.config.wad_dirs.iter().map(PathBuf::from).collect();
            let sources = shelf.sources();
            for dir in &sources {
                let kept = if *dir == shelf.downloads {
                    "the downloads"
                } else if global.contains(dir) {
                    "kept for every loadout"
                } else if here.contains(dir) {
                    "kept for this loadout"
                } else {
                    "this run"
                };
                let closed = if shelf.closed.contains(dir) {
                    ", closed"
                } else {
                    ""
                };
                murmur(format!("  {}  ({kept}{closed})", dir.display()));
            }
            // 🗄️ and the kept ones this run doesn't look in (--wads gave others)
            for dir in global
                .iter()
                .chain(&here)
                .filter(|dir| !sources.contains(dir))
            {
                let kept = if global.contains(dir) {
                    "kept for every loadout"
                } else {
                    "kept for this loadout"
                };
                murmur(format!("  {}  ({kept}, not this run)", dir.display()));
            }
            Ok(())
        }
        PoolRite::Add {
            dir,
            this_loadout: here,
        } => {
            if !dir.is_dir() {
                return Err(format!("There is no dir at {}", dir.display()));
            }
            let mut shelf = loadout.shelf();
            let path = PathBuf::from(dir_of(&dir));
            let kept = if here {
                &mut shelf.kept_loadout
            } else {
                &mut shelf.kept_global
            };
            if kept.contains(&path) {
                murmur(format!("{} is a wad pool already", path.display()));
                return Ok(());
            }
            if !here && !shelf.uac_writable {
                return Err(
                    "The settings are from a newer tdl, so no pool can be kept for every loadout"
                        .into(),
                );
            }
            kept.push(path.clone());
            loadout.restock(shelf).map_err(unsaved)?;
            murmur(format!(
                "Kept {} as a wad pool for {}",
                path.display(),
                if here {
                    "this loadout"
                } else {
                    "every loadout"
                }
            ));
            Ok(())
        }
        PoolRite::Remove { dir } => {
            let mut shelf = loadout.shelf();
            let path = PathBuf::from(dir_of(&dir));
            let before = shelf.kept_global.len() + shelf.kept_loadout.len();
            shelf.kept_global.retain(|kept| kept != &path);
            shelf.kept_loadout.retain(|kept| kept != &path);
            if shelf.kept_global.len() + shelf.kept_loadout.len() == before {
                return Err(format!("{} isn't a kept wad pool", path.display()));
            }
            loadout.restock(shelf).map_err(unsaved)?;
            murmur(format!("{} is no longer a wad pool", path.display()));
            Ok(())
        }
    }
}

fn seal(loadout: &Loadout, scroll: &Path, names: &[String], with_settings: bool) -> Outcome {
    let profiles: Vec<Profile> = if names.is_empty() {
        loadout.profiles.clone()
    } else {
        names
            .iter()
            .map(|name| find(loadout, name).map(|index| loadout.profiles[index].clone()))
            .collect::<Result<_, _>>()?
    };
    if profiles.is_empty() {
        return Err("There are no profiles to seal".into());
    }
    let path = share::with_extension(scroll.to_path_buf());
    if path.exists() {
        return Err(format!("There is already a file at {}", path.display()));
    }
    let mut sealed = Scroll::of(&profiles)?;
    if with_settings {
        sealed.settings = Some(loadout.uac.shared());
    }
    sealed
        .save(&path)
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    let count = profiles.len();
    murmur(format!(
        "Sealed {count} profile{}{} in {}",
        if count == 1 { "" } else { "s" },
        if with_settings {
            " and your global settings"
        } else {
            ""
        },
        path.display()
    ));
    Ok(())
}

fn unseal(
    loadout: &mut Loadout,
    path: &Path,
    with_settings: bool,
    replace: bool,
    yes: bool,
) -> Outcome {
    let scroll = share::read_plain(path)?;
    let mut dirs: Vec<PathBuf> = loadout
        .wad_dirs
        .iter()
        .filter(|dir| dir.is_dir())
        .cloned()
        .collect();
    dirs.push(loadout.downloads_dir());
    let total = scroll.wad_count();
    aside(format!("looking for {total} wads in the wad pools"));
    let unsealed = scroll
        .unseal(&dirs, &mut ())
        .map_err(|error| error.to_string())?;
    if unsealed.missing > 0 {
        wail(format!(
            "Found {} of {total} wads; the rest are marked missing (tdl cli show says which)",
            total - unsealed.missing
        ));
    }
    if replace {
        let count = loadout.profiles.len();
        if count > 0
            && !yes
            && !ask(&format!(
                "Delete the {count} profiles in this loadout and their saves first?"
            ))
        {
            murmur("Nothing was unsealed");
            return Ok(());
        }
        loadout.remove_all().map_err(unsaved)?;
    }
    let profiles: Vec<Profile> = unsealed
        .profiles
        .into_iter()
        .map(|profile| share::into_profile(loadout, profile))
        .collect();
    let mut index = loadout.profiles.len();
    // 🏷️ a name that is taken is tagged (brutal_2), with no one to ask
    let inserted = loadout
        .insert_copies(&mut index, profiles, |_, _, tagged| {
            Ok(Some(tagged.to_string()))
        })
        .map_err(unsaved)?;
    if inserted.failed {
        wail("Some of the scroll's profiles couldn't be saved");
    }
    murmur(format!(
        "Unsealed {} profile{}",
        inserted.count,
        if inserted.count == 1 { "" } else { "s" }
    ));
    match scroll.settings {
        Some(settings) if with_settings && settings != loadout.uac.shared() => {
            if !loadout.uac_writable {
                return Err(
                    "The settings are from a newer tdl, so the scroll's can't be kept".into(),
                );
            }
            loadout.uac.take_shared(settings);
            loadout.keep_uac().map_err(unsaved)?;
            murmur("Took the scroll's global settings");
        }
        Some(settings) if settings != loadout.uac.shared() => {
            aside("the scroll carries its maker's global settings: --with-settings takes them");
        }
        _ => {}
    }
    Ok(())
}

fn watch(loadout: &mut Loadout, dir: &Path, name: Option<&str>) -> Outcome {
    let dir = std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf());
    if !dir.is_dir() {
        return Err(format!("There is no dir at {}", dir.display()));
    }
    let name = match name {
        Some(name) => free_name(loadout, name, None)?,
        None => {
            let base: String = dir
                .file_name()
                .map(|name| {
                    name.to_string_lossy()
                        .chars()
                        .filter(|&c| profile::is_name_character(c))
                        .collect()
                })
                .filter(|name: &String| !name.trim().is_empty())
                .unwrap_or_else(|| "watched".into());
            let base = profile::clean_name(&base).unwrap_or_else(|| "watched".into());
            if loadout.profiles.iter().any(|profile| profile.name == base) {
                profile::tagged_name(&base, &loadout.profiles)
            } else {
                base
            }
        }
    };
    // 📍 it goes in last, and `insert` moves the index past it, to the new-profile row
    let at = loadout.profiles.len();
    let mut index = at;
    let profile = Profile {
        watched: Some(dir.clone().into_os_string()),
        ..Profile::new(name.clone(), Vec::new())
    };
    loadout.insert(&mut index, profile).map_err(unsaved)?;
    watch::refresh_plain(loadout, at)?;
    let index = at;
    let count = loadout.profiles[index].wads.len();
    murmur(format!(
        "Made {name}, watching {} ({count} wad{} in it)",
        dir.display(),
        if count == 1 { "" } else { "s" }
    ));
    Ok(())
}

/// How long a shell command may run before it is stopped.
const SHELL_TIME: std::time::Duration = std::time::Duration::from_secs(30);
/// The most lines of a shell command's output that are kept.
const SHELL_LINES: usize = 500;

/// Runs `command` the way bash would (sh where there is no bash), from the working directory, with
/// no input, for at most [`SHELL_TIME`], saying what it prints (its errors as wails), unless it is
/// [`forbidden`] or tdl is running as root or administrator. `Err` if it was refused, didn't finish
/// or failed.
pub fn shell(command: &str) -> Outcome {
    use std::process::{Command, Stdio};
    let command = command.trim();
    if command.is_empty() {
        return Err("sh needs a command after it, like sh ls".into());
    }
    if let Some(why) = forbidden(command) {
        return Err(format!("Not run: {why}"));
    }
    if elevated() {
        return Err(
            "Not run: tdl is running as root or administrator, and runs no shell commands so"
                .into(),
        );
    }
    let program = ["bash", "sh"]
        .into_iter()
        .find(|program| which(program))
        .ok_or("Not run: there is no bash or sh to run it with")?;
    let mut shell = Command::new(program);
    shell
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // 🔑 nothing that could answer a password prompt for it
        .env_remove("SUDO_ASKPASS")
        .env_remove("SSH_ASKPASS");
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut shell, 0);
    let mut child = shell
        .spawn()
        .map_err(|error| format!("Could not run {program}: {error}"))?;
    let read = |pipe: Option<Box<dyn io::Read + Send>>| {
        std::thread::spawn(move || {
            let mut text = String::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_string(&mut text);
            }
            text
        })
    };
    let out = read(
        child
            .stdout
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn io::Read + Send>),
    );
    let err = read(
        child
            .stderr
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn io::Read + Send>),
    );
    let began = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            break Some(status);
        }
        if began.elapsed() > SHELL_TIME {
            // 🛑 the whole group, so nothing it started lives on
            #[cfg(unix)]
            let _ = Command::new("kill")
                .args(["-s", "KILL", "--", &format!("-{}", child.id())])
                .status();
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let (out, err) = (
        out.join().unwrap_or_default(),
        err.join().unwrap_or_default(),
    );
    for line in out.lines().take(SHELL_LINES) {
        murmur(line);
    }
    for line in err.lines().take(SHELL_LINES) {
        wail(line);
    }
    match status {
        None => Err(format!("Stopped after {}s", SHELL_TIME.as_secs())),
        Some(status) if status.success() => Ok(()),
        Some(status) => Err(match status.code() {
            Some(code) => format!("It ended with code {code}"),
            None => "It was stopped by a signal".into(),
        }),
    }
}

/// Whether `program` is on the PATH.
fn which(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path)
            .any(|dir| dir.join(program).is_file() || dir.join(format!("{program}.exe")).is_file())
    })
}

/// Whether tdl itself runs as root (or, on Windows, as administrator).
fn elevated() -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("id")
            .arg("-u")
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).trim() == "0")
            .unwrap_or(true)
    }
    #[cfg(not(unix))]
    {
        std::process::Command::new("net")
            .arg("session")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(true)
    }
}

/// Programs a command line can never start: they take root or administrator, run the system,
/// rewrite disks or users, or hide what is really being run.
const DENIED: &[(&str, &str)] = &[
    ("sudo", "it needs root"),
    ("sudoedit", "it needs root"),
    ("su", "it needs root"),
    ("doas", "it needs root"),
    ("pkexec", "it needs root"),
    ("run0", "it needs root"),
    ("gksu", "it needs root"),
    ("gksudo", "it needs root"),
    ("kdesu", "it needs root"),
    ("runas", "it needs administrator"),
    ("passwd", "it changes users"),
    ("chpasswd", "it changes users"),
    ("useradd", "it changes users"),
    ("userdel", "it changes users"),
    ("usermod", "it changes users"),
    ("groupadd", "it changes users"),
    ("groupdel", "it changes users"),
    ("visudo", "it changes who may be root"),
    ("chown", "it changes who owns files"),
    ("chgrp", "it changes who owns files"),
    ("mkfs", "it rewrites disks"),
    ("mke2fs", "it rewrites disks"),
    ("fdisk", "it rewrites disks"),
    ("sfdisk", "it rewrites disks"),
    ("cfdisk", "it rewrites disks"),
    ("gdisk", "it rewrites disks"),
    ("parted", "it rewrites disks"),
    ("wipefs", "it rewrites disks"),
    ("shred", "it destroys files"),
    ("dd", "it writes disks byte by byte"),
    ("mount", "it changes the system's disks"),
    ("umount", "it changes the system's disks"),
    ("losetup", "it changes the system's disks"),
    ("cryptsetup", "it changes the system's disks"),
    ("swapon", "it changes the system's memory"),
    ("swapoff", "it changes the system's memory"),
    ("shutdown", "it stops the system"),
    ("reboot", "it stops the system"),
    ("poweroff", "it stops the system"),
    ("halt", "it stops the system"),
    ("init", "it runs the system"),
    ("telinit", "it runs the system"),
    ("systemctl", "it runs the system"),
    ("service", "it runs the system"),
    ("iptables", "it changes the firewall"),
    ("ip6tables", "it changes the firewall"),
    ("nft", "it changes the firewall"),
    ("ufw", "it changes the firewall"),
    ("firewall-cmd", "it changes the firewall"),
    ("crontab", "it schedules commands"),
    ("at", "it schedules commands"),
    ("insmod", "it changes the kernel"),
    ("rmmod", "it changes the kernel"),
    ("modprobe", "it changes the kernel"),
    ("sysctl", "it changes the kernel"),
    ("killall", "it stops every process of a kind"),
    ("pkill", "it stops processes by pattern"),
    ("eval", "it hides the command it runs"),
    ("source", "it hides the commands it runs"),
    (".", "it hides the commands it runs"),
    ("bash", "a shell inside the shell hides what it runs"),
    ("sh", "a shell inside the shell hides what it runs"),
    ("zsh", "a shell inside the shell hides what it runs"),
    ("dash", "a shell inside the shell hides what it runs"),
    ("ksh", "a shell inside the shell hides what it runs"),
    ("fish", "a shell inside the shell hides what it runs"),
    ("csh", "a shell inside the shell hides what it runs"),
    ("tcsh", "a shell inside the shell hides what it runs"),
    ("busybox", "a shell inside the shell hides what it runs"),
    ("cmd", "a shell inside the shell hides what it runs"),
    ("powershell", "a shell inside the shell hides what it runs"),
    ("pwsh", "a shell inside the shell hides what it runs"),
    ("format", "it rewrites disks"),
    ("diskpart", "it rewrites disks"),
    ("bcdedit", "it changes how the system starts"),
    ("reg", "it changes the system's registry"),
    ("takeown", "it changes who owns files"),
    ("icacls", "it changes who may use files"),
    ("cacls", "it changes who may use files"),
    ("net", "it changes users and the network"),
    ("netsh", "it changes the network"),
    ("sc", "it runs the system's services"),
    ("schtasks", "it schedules commands"),
    ("wmic", "it runs the system"),
];

/// Wrappers that run the command after them, which is the one that is looked at.
const WRAPPERS: &[&str] = &[
    "command", "exec", "env", "nohup", "nice", "time", "builtin", "xargs", "stdbuf", "ionice",
    "setsid", "timeout", "watch", "strace", "chroot",
];

/// Interpreters whose `-c` or `-e` runs code given on the command line, which hides what it does.
const INTERPRETERS: &[&str] = &[
    "python",
    "python3",
    "python2",
    "perl",
    "ruby",
    "node",
    "php",
    "lua",
    "awk",
    "gawk",
    "osascript",
];

/// Why `command` must not run, if it mustn't: it starts a [`DENIED`] program (looking past
/// [`WRAPPERS`] and paths), hides what it runs, or reaches for what only harm needs (the whole
/// disk, the system's files, everything at once).
pub fn forbidden(command: &str) -> Option<String> {
    let lower = command.to_lowercase();
    if lower.contains(":(){") || lower.replace(' ', "").contains(":(){:|:&};:") {
        return Some("it is a fork bomb".into());
    }
    if lower.contains("--no-preserve-root") {
        return Some("it would delete everything".into());
    }
    // 💽 writing into the system's folders or straight onto a disk
    let systems = [
        "/etc",
        "/boot",
        "/usr",
        "/bin",
        "/sbin",
        "/lib",
        "/sys",
        "/proc",
        "/dev/sd",
        "/dev/nvme",
        "/dev/mmcblk",
        "/dev/disk",
        "/dev/mem",
        "/dev/port",
        "c:\\windows",
    ];
    let squeezed: String = lower.split_whitespace().collect::<Vec<_>>().join(" ");
    for system in systems {
        for writer in [">", ">>", "tee ", "tee -a ", "cp -t ", "of="] {
            if squeezed.contains(&format!("{writer}{system}"))
                || squeezed.contains(&format!("{writer} {system}"))
            {
                return Some(format!("it writes into {system}"));
            }
        }
    }
    // ✂️ every command of a pipeline or a list, each looked at on its own
    let segments = lower.split([';', '|', '&', '\n', '`', '(', ')', '{', '}']);
    for segment in segments {
        let segment = segment.trim();
        let mut words = segment.split_whitespace().peekable();
        // 🏷️ VAR=value in front, and the wrappers that run what comes after them
        while let Some(word) = words.peek() {
            let bare = word.rsplit('/').next().unwrap_or(word);
            if word.contains('=') && !word.starts_with('=')
                || WRAPPERS.contains(&bare)
                || word.starts_with('-')
                || word.parse::<f64>().is_ok()
            {
                words.next();
            } else {
                break;
            }
        }
        let Some(first) = words.next() else { continue };
        let rest: Vec<&str> = words.collect();
        if first.starts_with('$')
            || first.contains("$(")
            || first.starts_with('"')
            || first.starts_with('\'')
        {
            return Some("a command chosen by a variable or a quote hides what it runs".into());
        }
        let program = first.rsplit(['/', '\\']).next().unwrap_or(first);
        let program = program.trim_end_matches(".exe");
        if let Some((_, why)) = DENIED
            .iter()
            .find(|(denied, _)| program == *denied || program.starts_with("mkfs."))
        {
            return Some(format!("{program}: {why}"));
        }
        let interpreter = program.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
        if (INTERPRETERS.contains(&program) || INTERPRETERS.contains(&interpreter))
            && rest.iter().any(|word| {
                matches!(*word, "-c" | "-e" | "-r" | "--eval" | "-E") || program.ends_with("awk")
            })
        {
            return Some(format!(
                "{program}: code given on the command line hides what it does"
            ));
        }
        // 🗑️ everything at once, or the system's or your whole home
        let sweeping = |word: &&str| {
            let word = word.trim_matches(|c| c == '"' || c == '\'');
            matches!(
                word,
                "/" | "/*"
                    | "~"
                    | "~/"
                    | "~/*"
                    | "$home"
                    | "${home}"
                    | "*"
                    | "."
                    | ".."
                    | "./*"
                    | "../*"
            ) || (word.starts_with('/') && word.trim_end_matches('/').matches('/').count() <= 1)
                || word == "/home"
                || word.starts_with("/home/")
                    && word.trim_end_matches('/').matches('/').count() <= 2
        };
        let recursive = rest.iter().any(|word| {
            word.starts_with('-')
                && !word.starts_with("--")
                && (word.contains('r') || word.contains('R'))
                || *word == "--recursive"
        });
        match program {
            "rm" | "rmdir" | "del" | "rd"
                if (recursive || program != "rm") && rest.iter().any(sweeping) =>
            {
                return Some(format!("{program} would delete too much"));
            }
            "rm" if rest.iter().any(sweeping)
                && rest
                    .iter()
                    .any(|word| word.starts_with('-') && word.contains('f')) =>
            {
                return Some("rm would delete too much".into());
            }
            "chmod"
                if rest.iter().any(|word| {
                    word.contains('s') && (word.contains('+') || word.contains('='))
                }) || rest.iter().any(|word| {
                    word.len() == 4
                        && word.starts_with(['2', '4', '6', '7'])
                        && word.parse::<u16>().is_ok()
                }) =>
            {
                return Some("chmod would set a setuid bit".into());
            }
            "chmod" if recursive && rest.iter().any(sweeping) => {
                return Some("chmod would change too much".into());
            }
            "find"
                if rest
                    .iter()
                    .any(|word| *word == "-delete" || *word == "-exec")
                    && rest.iter().any(sweeping) =>
            {
                return Some("find would change too much".into());
            }
            "kill" if rest.iter().any(|word| *word == "-1" || *word == "1") => {
                return Some("kill would stop every process".into());
            }
            _ => {}
        }
    }
    None
}
