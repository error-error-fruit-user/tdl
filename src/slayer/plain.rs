//! `tdl launch --headless`, `tdl host --headless` and `tdl join --headless`: the same as
//! launching, hosting and joining from the launcher, but with no windows at all, only plain lines in the terminal. The questions asked
//! are whether to download wads that aren't installed, and what to do with the joiner's own wads
//! that have the names of the host's but not their contents; anything else that would be a
//! question is said and gone along with, and anything that stops the game is an error, which ends
//! tdl with a failure.

use std::{
    ffi::OsStr,
    ffi::OsString,
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
    loadout::{
        Loadout, edition::Edition, host_settings::HostSettings, lookup::Local, profile::Profile,
        scroll::Scroll, uac::Shared, wad::Wad,
    },
    net::{self, Address, Manifest, ini::HostIni},
    screens::wad_namer,
    slayer::{
        host_game,
        join_game::{self, fill_gaps, first_match},
        share,
        upgrade::{self, Choice, Outdated, Settled},
    },
    tag,
    tally::Tally,
    uzdoom::{self, Game},
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

/// Plays the profile at `index`, and returns when the game ends.
pub fn launch(loadout: &Loadout, index: usize) -> Outcome {
    let profile = &loadout.profiles[index];
    let uzdoom = ready_to_play(loadout, profile)?;
    println!("Launching {}", profile.name);
    let game = uzdoom::launch(
        &uzdoom,
        profile.base_wad(&loadout.uac).map(Wad::deref),
        profile.ini(&loadout.uac).map(OsString::as_os_str),
        profile,
        profile.uzdoom_args(&loadout.uac),
        &loadout.data_dir.join(&profile.name),
    )
    .map_err(|error| error.to_string())?;
    wait_for(game)?;
    println!("The game is over");
    Ok(())
}

/// Hosts the profile at `index` with its host defaults, changed by `overrides`, and returns when
/// the game ends.
pub fn host(loadout: &mut Loadout, index: usize, overrides: &HostOverrides) -> Outcome {
    let profile = loadout.profiles[index].clone();
    let defaults = profile
        .host_defaults
        .clone()
        .unwrap_or_else(|| HostSettings::new(loadout.uac.host_port));
    let settings = overrides.apply(defaults);
    let port = settings
        .port()
        .ok_or("The port must be a number from 1 to 65535")?;
    let uzdoom = ready_to_play(loadout, &profile)?;
    // 💾 only a port other than the default is worth remembering, as the host form does
    let remembered = (port != net::DEFAULT_PORT).then_some(port);
    if loadout.uac.host_port != remembered {
        loadout.uac.host_port = remembered;
        if loadout.keep_uac().is_err() {
            eprintln!("Could not save the port for next time, is the disk full?");
        }
    }
    // 🌀 volatile wads are hashed fresh and folders packed, so what is served is what is described
    let packed_dir = loadout.packed_dir();
    let wads: Vec<Wad> = profile
        .wads
        .iter()
        .map(|wad| wad.served(&packed_dir, &mut ()))
        .collect::<Result<_, _>>()
        .map_err(|error| match error {
            Error::Pack(problem) => problem,
            _ => "One of the profile's wads can't be read".to_string(),
        })?;
    let base_wad = profile.base_wad(&loadout.uac);
    let manifest = Manifest::build(&wads, base_wad.map(|w| w.hash.clone()).unwrap_or_default())
        .map_err(|_| "One of the profile's wads can't be read")?;
    let ini = if settings.share_ini {
        match host_game::shared_ini(profile.ini(&loadout.uac).map(OsString::as_os_str), &uzdoom) {
            Ok(ini) => Some(ini),
            Err(problem) => {
                eprintln!("{problem}, so it isn't shared");
                None
            }
        }
    } else {
        None
    };
    let sharing = ini.is_some();
    let code = settings.code();
    let server = manifest
        .serve(
            port,
            code.clone(),
            wads.iter().map(|w| w.path.clone()).collect(),
            ini,
        )
        .map_err(|_| format!("Could not serve the wads on port {port}: is it in use?"))?;
    println!(
        "Hosting {}: players join with port {}, code {code}",
        profile.name, server.port
    );
    if sharing {
        println!("Your ini is shared");
    }
    let game = uzdoom::host(
        &uzdoom,
        base_wad.map(Wad::deref),
        profile.ini(&loadout.uac).map(OsString::as_os_str),
        &profile,
        &settings,
        profile.uzdoom_args(&loadout.uac),
        &loadout.data_dir.join(&profile.name),
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
/// `always_download`), with the host's ini (and its keys) as `host_ini` says, if it shares one,
/// and returns when the game ends.
pub fn join(
    loadout: &mut Loadout,
    address: &Address,
    always_download: bool,
    host_ini: HostIni,
) -> Outcome {
    let where_to = format!("{}:{}", address.host, address.port);
    let manifest = match Manifest::fetch(address) {
        Ok(manifest) => manifest,
        Err(Error::Refused) => return Err(format!("The host at {where_to} refused the join code")),
        Err(_) => return Err(format!("Couldn't reach a host at {where_to}")),
    };
    let profile = match resolve_profile(loadout, address, &manifest, always_download)? {
        Some(profile) => profile,
        None => {
            println!("Not joining");
            return Ok(());
        }
    };
    let uzdoom = ready_to_play(loadout, &profile)?;
    let profile = upgrade_volatile(loadout, address, &manifest, profile)?;
    let yours = profile.ini(&loadout.uac).cloned();
    let ini = if host_ini != HostIni::Yours {
        let keys = host_ini == HostIni::WithKeys;
        match join_game::with_host_ini(loadout, address, &profile, &uzdoom, keys) {
            Ok(Some((hosts, warnings))) => {
                for warning in &warnings {
                    eprintln!("{warning}");
                }
                println!("Playing with the host's ini");
                Some(hosts)
            }
            Ok(None) => {
                eprintln!("The host doesn't share its ini, so yours is used");
                yours
            }
            Err(problem) => {
                eprintln!("{problem}, so yours is used");
                yours
            }
        }
    } else {
        yours
    };
    let base_wad = profile.base_wad(&loadout.uac);
    if !manifest.base_wad_matches(base_wad) {
        eprintln!("Your base wad isn't the host's, joining anyway");
    }
    let game = uzdoom::join(
        &uzdoom,
        base_wad.map(Wad::deref),
        ini.as_deref(),
        &profile,
        &address.host,
        profile.uzdoom_args(&loadout.uac),
        &loadout.data_dir.join(&profile.name),
    )
    .map_err(|error| error.to_string())?;
    wait_for(game)?;
    println!("The game is over");
    Ok(())
}

/// The first profile that matches the host's wads, or a new one made from the wads already here
/// and downloads of the rest. With none, the joiner's own wads with the names of the host's but
/// other contents are offered the host's version first, after which one may match. `None` if the
/// user says no.
fn resolve_profile(
    loadout: &mut Loadout,
    address: &Address,
    manifest: &Manifest,
    always_download: bool,
) -> std::result::Result<Option<Profile>, String> {
    if let Some(profile) = compatible(loadout, manifest) {
        return Ok(Some(profile));
    }
    if loadout.uac.uzdoom_in_effect().is_none() {
        return Err("No UZDoom is selected: pick one in tdl with U, or pass --uzdoom".to_string());
    }
    let mut local = find_local(loadout, manifest)?;
    let outdated = upgrade::outdated(loadout, manifest, &local);
    if !outdated.is_empty() {
        let choices = choose(loadout, &outdated, always_download);
        let settled = bring_up_to_date(loadout, address, manifest, &outdated, &choices)?;
        for (wad, placed) in local.iter_mut().zip(settled.placed) {
            if wad.is_none() {
                *wad = placed;
            }
        }
        // ⬆️ a profile whose wads were just brought up to date may be the host's now
        if let Some(profile) = compatible(loadout, manifest) {
            return Ok(Some(profile));
        }
    }
    make_profile(loadout, address, manifest, local, always_download)
}

/// The first profile that matches the host's wads, said.
fn compatible(loadout: &Loadout, manifest: &Manifest) -> Option<Profile> {
    let profile = loadout
        .profiles
        .iter()
        .find(|profile| manifest.is_compatible_with(profile))?;
    println!("Joining with {}", profile.name);
    Some(profile.clone())
}

/// The host's wads that are already on this machine, in the manifest's order, with `None` for
/// each one that isn't.
fn find_local(
    loadout: &mut Loadout,
    manifest: &Manifest,
) -> std::result::Result<Vec<Option<Wad>>, String> {
    let lookup = loadout.lookup();
    let checks: Vec<_> = manifest
        .entries
        .iter()
        .map(|entry| lookup.check(entry))
        .collect();
    checks
        .into_iter()
        .zip(&manifest.entries)
        .map(|(check, entry)| match check {
            Local::Intact(wad) => Ok(Some(wad)),
            Local::Maybe(paths) => first_match(paths, entry, &mut ()),
        })
        .collect::<Result<_, _>>()
        .map_err(|error| error.to_string())
}

/// What to do with each of the joiner's wads that isn't the host's version: a volatile one is
/// replaced without asking, and so is any other with `always_download`; otherwise the joiner is
/// asked, and no answer is skip.
fn choose(loadout: &Loadout, outdated: &[Outdated], always_download: bool) -> Vec<Choice> {
    outdated
        .iter()
        .map(|wad| {
            let shown = wad.path.display();
            if wad.volatile {
                println!("Replacing your volatile {shown} with the host's version");
                Choice::Replace
            } else if always_download {
                println!(
                    "Replacing {shown} with the host's version, as --always-download says{}",
                    if wad.is_download(loadout) {
                        String::new()
                    } else {
                        format!(" (yours is kept as {}.bak)", wad.file_name())
                    }
                );
                Choice::Replace
            } else if wad.is_download(loadout) {
                let question = format!(
                    "Your download of {} isn't the host's version. Use the host's in its place?",
                    wad.file_name()
                );
                if ask(&question) {
                    Choice::Replace
                } else {
                    Choice::Skip
                }
            } else {
                ask_replace(wad)
            }
        })
        .collect()
}

/// Asks whether to replace the joiner's own `wad` with the host's, rename theirs to keep both, or
/// skip it. Anything but r or n is skip, and so is having no terminal to ask on.
fn ask_replace(wad: &Outdated) -> Choice {
    let name = wad.file_name();
    let question = format!(
        "{} isn't the host's {name}. Replace it with the host's (yours kept as {name}.bak), \
         rename yours to keep both, or skip it?",
        wad.path.display()
    );
    let answer = answer(&question, "[r/n/S]", "skip").map(|answer| answer.to_ascii_lowercase());
    match answer.as_deref() {
        Some("r" | "replace") => Choice::Replace,
        Some("n" | "rename" | "name") => ask_rename(wad),
        _ => Choice::Skip,
    }
}

/// Asks for a new name for the joiner's own `wad`, offering a tagged one (`brutal_2.pk3`) for an
/// empty answer. The extension stays, so it can be left off. A name that can't be used is said,
/// and the wad skipped.
fn ask_rename(wad: &Outdated) -> Choice {
    let name = wad.file_name();
    let dir = wad.path.parent().unwrap_or(Path::new(""));
    let is_free = |name: &str| !dir.join(name).exists();
    let suggested = tag::tagged_file(&name, is_free);
    let Some(typed) = answer("New name for yours?", &format!("[{suggested}]"), "skip") else {
        return Choice::Skip;
    };
    let (_, extension) = tag::split_extension(&name);
    let renamed = if typed.is_empty() {
        suggested
    } else if typed
        .to_ascii_lowercase()
        .ends_with(&extension.to_ascii_lowercase())
    {
        typed
    } else {
        format!("{typed}{extension}")
    };
    if renamed.starts_with('.') || !renamed.chars().all(wad_namer::allowed) {
        eprintln!("{renamed} can't be a file name everywhere, so skipping {name}");
        Choice::Skip
    } else if !is_free(&renamed) {
        eprintln!("{renamed} is already there, so skipping {name}");
        Choice::Skip
    } else {
        Choice::Rename(renamed)
    }
}

/// `profile` with each volatile wad that differs from the host's brought up to the host's version
/// without asking: hashed again first, since its stored hash is often only out of date, and only
/// replaced (the old file kept as `.bak`) if it really differs. What goes wrong is said, and the
/// game joined with the joiner's own version.
fn upgrade_volatile(
    loadout: &mut Loadout,
    address: &Address,
    manifest: &Manifest,
    profile: Profile,
) -> std::result::Result<Profile, String> {
    let differing = manifest.differing_wads(&profile);
    if differing.is_empty() {
        return Ok(profile);
    }
    let mut outdated = Vec::new();
    for entry in differing {
        let wad = profile.wads[entry].refreshed(&mut ()).map_err(|error| {
            format!("Couldn't read {}: {error}", profile.wads[entry].file_name())
        })?;
        let path = PathBuf::from(&wad.path);
        if wad.hash == manifest.entries[entry].hash {
            // 🟰 it only looked different: the file has become the host's since it was hashed
            if loadout.upgrade_wad(&path, &wad).is_err() {
                eprintln!("Could not save the new hash of {}", path.display());
            }
        } else {
            println!(
                "Replacing your volatile {} with the host's version",
                path.display()
            );
            outdated.push(Outdated {
                entry,
                path,
                volatile: true,
            });
        }
    }
    let choices = vec![Choice::Replace; outdated.len()];
    let settled = bring_up_to_date(loadout, address, manifest, &outdated, &choices)?;
    if !settled.problems.is_empty() {
        eprintln!("Joining anyway, with your own versions of those");
    }
    let upgraded = loadout.profiles.iter().find(|p| p.name == profile.name);
    Ok(upgraded.cloned().unwrap_or(profile))
}

/// Downloads the host's version of each outdated wad that `choices` doesn't skip, then does each
/// choice, saying whatever went wrong.
fn bring_up_to_date(
    loadout: &mut Loadout,
    address: &Address,
    manifest: &Manifest,
    outdated: &[Outdated],
    choices: &[Choice],
) -> std::result::Result<Settled, String> {
    let wanted = upgrade::wanted(outdated, choices);
    let needed: Vec<_> = wanted
        .iter()
        .map(|&i| manifest.entries[i].clone())
        .collect();
    let downloaded = download(loadout, address, &needed)?;
    let downloaded: Vec<_> = wanted.into_iter().zip(downloaded).collect();
    let settled = upgrade::settle(
        loadout,
        manifest.entries.len(),
        outdated,
        choices,
        &downloaded,
    );
    for problem in &settled.problems {
        eprintln!("{problem}");
    }
    Ok(settled)
}

/// Downloads `entries` from the host, a line for each, or nothing if there are none. Ctrl+C ends
/// tdl.
fn download(
    loadout: &Loadout,
    address: &Address,
    entries: &[net::Entry],
) -> std::result::Result<Vec<Wad>, String> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    let mut lines = Lines(Keys::take());
    match net::download(address, entries, &loadout.downloads_dir(), &mut lines) {
        Ok(wads) => Ok(wads),
        // 🧹 the partial file is gone already; finished ones stay, so a retry picks up there
        Err(Error::Cancelled) => lines.0.end_interrupted(),
        Err(error) => Err(format!("The download failed: {error}")),
    }
}

/// The UZDoom to play `profile` with, if it has one and nothing it needs is broken.
fn ready_to_play(
    loadout: &Loadout,
    profile: &Profile,
) -> std::result::Result<std::ffi::OsString, String> {
    let uzdoom = profile
        .uzdoom(&loadout.uac)
        .cloned()
        .ok_or("No UZDoom is selected: pick one in tdl with U, or pass --uzdoom")?;
    let broken: Vec<_> = profile
        .faults(&loadout.uac)
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

/// A new profile for the host's wads: the ones already here (`local`) where they are, and the
/// rest downloaded, but only if the user says yes. Named after the host, so it is easy to find
/// and rename in tdl. `None` if the user says no.
fn make_profile(
    loadout: &mut Loadout,
    address: &Address,
    manifest: &Manifest,
    local: Vec<Option<Wad>>,
    always_download: bool,
) -> std::result::Result<Option<Profile>, String> {
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
    let downloaded = download(loadout, address, &wanted)?;
    let wads = fill_gaps(local, downloaded).ok_or("Fewer wads arrived than were asked for")?;
    let name = joined_name(&address.host, &loadout.profiles);
    let profile = Profile::new(name, wads);
    let mut index = loadout.profiles.len();
    loadout
        .insert(&mut index, profile.clone())
        .map_err(|_| "Could not save the new profile, is the disk full?")?;
    println!("Made the profile {}", profile.name);
    Ok(Some(profile))
}

/// `tdl --seal`: every profile in `loadout`, in the launcher's order, and the global `settings` if
/// there are any, sealed into `scroll` (`.scroll` added if it doesn't end in it). An existing file
/// is never written over. Returns where the scroll went.
pub fn seal(
    loadout: &Loadout,
    settings: Option<Shared>,
    scroll: &Path,
) -> std::result::Result<PathBuf, String> {
    if loadout.profiles.is_empty() {
        return Err("There are no profiles in the loadout to seal".to_string());
    }
    let path = share::with_extension(scroll.to_path_buf());
    if path.exists() {
        return Err(format!("There is already a file at {}", path.display()));
    }
    let mut sealed = Scroll::of(&loadout.profiles)?;
    let with = if settings.is_some() {
        " and your global settings"
    } else {
        ""
    };
    sealed.settings = settings;
    sealed
        .save(&path)
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    let count = loadout.profiles.len();
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
    answer(question, "[y/N]", "no").is_some_and(|answer| is_yes(&answer))
}

/// Asks `question` on the terminal, with the `answers` it takes after it, and returns what was
/// typed, trimmed. `None` if nothing could be read, or if there is no terminal to ask on, which is
/// said along with what is done `instead`.
fn answer(question: &str, answers: &str, instead: &str) -> Option<String> {
    if !io::stdin().is_terminal() {
        eprintln!("{question} No one to ask, so {instead}");
        return None;
    }
    print!("{question} {answers} ");
    let _ = io::stdout().flush();
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer).ok()?;
    Some(answer.trim().to_string())
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
        loadout::{edition::Loaded, scribe::tests::Altar},
        testing,
    };

    /// The example loadout in `altar`.
    fn example(altar: &Altar) -> Loadout {
        let root = testing::example_loadout(altar);
        Loadout::new(root.join("config"), root.join("data"))
    }

    #[test]
    fn sealing_puts_every_profile_and_the_settings_in_a_new_scroll() {
        let altar = Altar::new("plain-seal");
        let loadout = example(&altar);
        let settings = Shared::default();
        let path = seal(&loadout, Some(settings.clone()), &altar.file("all")).unwrap();
        assert_eq!(path, altar.file("all.scroll"), ".scroll is added");
        let Loaded::Fine(scroll) = Scroll::load(&path) else {
            panic!("the scroll should read back");
        };
        let names: Vec<_> = scroll.profiles.iter().map(|p| p.name.clone()).collect();
        let expected: Vec<_> = loadout.profiles.iter().map(|p| p.name.clone()).collect();
        assert_eq!(names, expected);
        assert_eq!(scroll.settings, Some(settings));
        let bare = seal(&loadout, None, &altar.file("bare")).unwrap();
        let Loaded::Fine(bare) = Scroll::load(&bare) else {
            panic!("the scroll should read back");
        };
        assert_eq!(bare.settings, None, "settings only go in when asked for");
        // 🚫 never over a file that is already there
        assert!(seal(&loadout, None, &path).is_err());
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
