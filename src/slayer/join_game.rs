use std::{
    ffi::OsString,
    ops::Deref,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::RecvTimeoutError,
    },
};

use crate::{
    error::{Error, Result},
    loadout::{Loadout, lookup::Local, profile::Profile, wad::Wad},
    net::{self, Address, Entry, Manifest, Report, ini::HostIni},
    popups::{
        cancel::CancelHandler,
        dialog::Dialog,
        hashing,
        progress::{Kind, Progress},
        running, status,
        three_way_dialog::ThreeWayDialog,
        warning::Warning,
    },
    ritual::Grimoire,
    screens::{profile_namer::ProfileNamer, profile_picker::ProfilePicker, wad_namer::WadNamer},
    slayer::upgrade::{self, Choice, Outdated, Settled},
    tag,
    tally::Tally,
    theme, uzdoom,
};

/// Fetches the host's manifest and picks a profile to join with: the only compatible one, a choice
/// among several, or a new profile downloaded from the host when none match. With
/// `always_download`, the wads that aren't installed are downloaded without asking first.
/// `host_ini` says whether the game is played with the host's ini, if it shares it, and its keys
/// (see [`net::ini`]).
pub fn summon(
    grimoire: &mut Grimoire,
    loadout: &mut Loadout,
    index: &mut usize,
    address: &Address,
    always_download: bool,
    host_ini: HostIni,
) -> Result<()> {
    status::summon(
        grimoire,
        "Joining",
        "Joining...",
        theme::worn().colors.netplay,
    )?;
    let manifest = match Manifest::fetch(address) {
        Ok(manifest) => manifest,
        Err(Error::Refused) => return Warning::code_refused(grimoire),
        Err(_) => return Warning::host_unreachable(grimoire),
    };
    let Some(profile) = resolve_profile(
        grimoire,
        loadout,
        index,
        address,
        &manifest,
        always_download,
    )?
    else {
        return Ok(());
    };
    if Warning::faults(grimoire, &profile.faults(&loadout.uac))? {
        return Ok(());
    }
    let Some(profile) = upgrade_volatile(grimoire, loadout, address, &manifest, profile)? else {
        return Ok(());
    };
    let Some(uzdoom) = profile.uzdoom(&loadout.uac) else {
        return Warning::uzdoom_not_selected(grimoire);
    };
    let yours = profile.ini(&loadout.uac).cloned();
    let ini = if host_ini != HostIni::Yours {
        status::summon(
            grimoire,
            "Joining",
            "Getting the host's ini...",
            theme::worn().colors.netplay,
        )?;
        let keys = host_ini == HostIni::WithKeys;
        match with_host_ini(loadout, address, &profile, uzdoom, keys) {
            Ok(Some((hosts, warnings))) => {
                if !warnings.is_empty() {
                    Warning::summon_lines(grimoire, &warnings)?;
                }
                Some(hosts)
            }
            Ok(None) => {
                Warning::summon(grimoire, "The host doesn't share its ini, so yours is used")?;
                yours
            }
            Err(problem) => {
                Warning::summon(grimoire, &format!("{problem}, so yours is used"))?;
                yours
            }
        }
    } else {
        yours
    };
    let base_wad = profile.base_wad(&loadout.uac);
    if manifest.base_wad_matches(base_wad)
        || Dialog::summon(
            grimoire,
            "Selected base wad does not match, do you want to continue?",
        )?
    {
        let details = [format!("host {}", address.host)];
        let result = uzdoom::join(
            uzdoom,
            base_wad.map(Wad::deref),
            ini.as_deref(),
            &profile,
            &address.host,
            profile.uzdoom_args(&loadout.uac),
            &loadout.data_dir.join(&profile.name),
        )
        .and_then(|game| running::summon(grimoire, "joining", &details, game));
        Warning::report_spawn(grimoire, result)
    } else {
        Ok(())
    }
}

/// The ini `profile` joins with when it takes the host's (see [`net::ini::take`]), with its keys if
/// `keys`, kept in its own data dir as `host.ini` (nothing is saved to the profile), and the
/// warnings about what was left out. `None` if the host shares no ini.
pub fn with_host_ini(
    loadout: &Loadout,
    address: &Address,
    profile: &Profile,
    uzdoom: &std::ffi::OsStr,
    keys: bool,
) -> std::result::Result<Option<(OsString, Vec<String>)>, String> {
    let own = net::ini::in_use(profile.ini(&loadout.uac).map(OsString::as_os_str), uzdoom);
    let dir = loadout.data_dir.join(&profile.name);
    let taken = net::ini::take(address, own.as_deref(), keys, &dir)?;
    Ok(taken.map(|taken| (taken.path.into_os_string(), taken.warnings)))
}

/// The profile to join with: the single compatible one, a pick among several, or a fresh download.
/// With none, the joiner's own wads with the names of the host's but other contents are offered
/// the host's version first, after which a profile may be compatible. `None` if the user backs
/// out.
fn resolve_profile(
    grimoire: &mut Grimoire,
    loadout: &mut Loadout,
    index: &mut usize,
    address: &Address,
    manifest: &Manifest,
    always_download: bool,
) -> Result<Option<Profile>> {
    if let Some(picked) = pick_compatible(grimoire, loadout, manifest)? {
        return Ok(picked);
    }
    if loadout.uac.uzdoom_in_effect().is_none() {
        Warning::uzdoom_not_selected(grimoire)?;
        return Ok(None);
    }
    let Some(mut local) = find_local(grimoire, loadout, manifest)? else {
        return Ok(None);
    };
    let outdated = upgrade::outdated(loadout, manifest, &local);
    if !outdated.is_empty() {
        let Some(choices) = choose(grimoire, loadout, &outdated)? else {
            return Ok(None);
        };
        let Some(settled) =
            bring_up_to_date(grimoire, loadout, address, manifest, &outdated, &choices)?
        else {
            return Ok(None);
        };
        for (wad, placed) in local.iter_mut().zip(settled.placed) {
            if wad.is_none() {
                *wad = placed;
            }
        }
        // ⬆️ a profile whose wads were just brought up to date may be the host's now
        if let Some(picked) = pick_compatible(grimoire, loadout, manifest)? {
            return Ok(picked);
        }
    }
    download_profile(
        grimoire,
        loadout,
        index,
        address,
        manifest,
        local,
        always_download,
    )
}

/// The compatible profile to join with: the only one, or a pick among several (`Some(None)` if
/// the user backs out). `None` if there are none.
fn pick_compatible(
    grimoire: &mut Grimoire,
    loadout: &Loadout,
    manifest: &Manifest,
) -> Result<Option<Option<Profile>>> {
    let compatible_profiles: Vec<_> = loadout
        .profiles
        .iter()
        .filter(|p| manifest.is_compatible_with(p))
        .collect();
    Ok(match compatible_profiles.len() {
        0 => None,
        1 => Some(Some(compatible_profiles[0].clone())),
        _ => Some(ProfilePicker::summon(grimoire, &compatible_profiles)?),
    })
}

/// What the joiner wants done with each of their wads that isn't the host's version: a volatile
/// one is replaced without asking, a download is swapped for the host's or not, and any other is
/// replaced, renamed or skipped. `None` if Esc was pressed.
fn choose(
    grimoire: &mut Grimoire,
    loadout: &Loadout,
    outdated: &[Outdated],
) -> Result<Option<Vec<Choice>>> {
    let mut choices = Vec::with_capacity(outdated.len());
    for wad in outdated {
        let choice = if wad.volatile {
            Choice::Replace
        } else if wad.is_download(loadout) {
            let question = format!(
                "Your download of {} isn't the host's version. Use the host's in its place?",
                wad.file_name()
            );
            if Dialog::summon(grimoire, &question)? {
                Choice::Replace
            } else {
                Choice::Skip
            }
        } else {
            match ask_replace(grimoire, wad)? {
                Some(choice) => choice,
                None => return Ok(None),
            }
        };
        choices.push(choice);
    }
    Ok(Some(choices))
}

/// Asks whether to replace the joiner's own `wad` with the host's, rename theirs to keep both, or
/// skip it. Esc while naming goes back to the question; `None` if Esc was pressed on it.
fn ask_replace(grimoire: &mut Grimoire, wad: &Outdated) -> Result<Option<Choice>> {
    let name = wad.file_name();
    let question = format!(
        "{} isn't the host's {name}. Replace it with the host's (yours kept as {name}.bak), or \
         rename yours to keep both?",
        wad.path.display()
    );
    let dir = wad.path.parent().unwrap_or(Path::new(""));
    let is_free = |name: &str| !dir.join(name).exists();
    loop {
        match ThreeWayDialog::summon(grimoire, &question, ["Replace", "Name Mine", "Skip"])? {
            Some(0) => return Ok(Some(Choice::Replace)),
            Some(1) => {
                let suggested = tag::tagged_file(&name, is_free);
                if let Some(renamed) = WadNamer::summon_renaming(grimoire, &suggested, is_free)? {
                    return Ok(Some(Choice::Rename(renamed)));
                }
            }
            Some(_) => return Ok(Some(Choice::Skip)),
            None => return Ok(None),
        }
    }
}

/// `profile` with each volatile wad that differs from the host's brought up to the host's version
/// without asking: hashed again first, since its stored hash is often only out of date, and only
/// replaced (the old file kept as `.bak`) if it really differs. If that fails, the user is told
/// and asked whether to join with their own. `None` if they back out.
fn upgrade_volatile(
    grimoire: &mut Grimoire,
    loadout: &mut Loadout,
    address: &Address,
    manifest: &Manifest,
    profile: Profile,
) -> Result<Option<Profile>> {
    let differing = manifest.differing_wads(&profile);
    if differing.is_empty() {
        return Ok(Some(profile));
    }
    let stale: Vec<_> = differing.iter().map(|&i| profile.wads[i].clone()).collect();
    let Some(fresh) = hashing::refresh(grimoire, &stale)? else {
        return Ok(None);
    };
    let mut outdated = Vec::new();
    for (&entry, wad) in differing.iter().zip(fresh) {
        let path = PathBuf::from(&wad.path);
        if wad.hash == manifest.entries[entry].hash {
            // 🟰 it only looked different: the file has become the host's since it was hashed
            if loadout.upgrade_wad(&path, &wad).is_err() {
                Warning::save_failed(grimoire)?;
            }
        } else {
            outdated.push(Outdated {
                entry,
                path,
                volatile: true,
            });
        }
    }
    let choices = vec![Choice::Replace; outdated.len()];
    let Some(settled) =
        bring_up_to_date(grimoire, loadout, address, manifest, &outdated, &choices)?
    else {
        return Ok(None);
    };
    if !settled.problems.is_empty()
        && !Dialog::summon(
            grimoire,
            "Join anyway, with your own versions of those wads?",
        )?
    {
        return Ok(None);
    }
    let upgraded = loadout.profiles.iter().find(|p| p.name == profile.name);
    Ok(Some(upgraded.cloned().unwrap_or(profile)))
}

/// Downloads the host's version of each outdated wad that `choices` doesn't skip, then does each
/// choice, saying whatever went wrong. `None` if Esc was pressed or the download failed (which the
/// user was told).
fn bring_up_to_date(
    grimoire: &mut Grimoire,
    loadout: &mut Loadout,
    address: &Address,
    manifest: &Manifest,
    outdated: &[Outdated],
    choices: &[Choice],
) -> Result<Option<Settled>> {
    let wanted = upgrade::wanted(outdated, choices);
    let needed = wanted
        .iter()
        .map(|&i| manifest.entries[i].clone())
        .collect();
    let Some(downloaded) = fetch(grimoire, address, needed, loadout.downloads_dir())? else {
        return Ok(None);
    };
    if !downloaded.is_empty() {
        // ⏳ a big pk3 takes a moment to copy into place
        status::summon(
            grimoire,
            "copying",
            "Putting the host's wads in place...",
            theme::worn().colors.busy,
        )?;
    }
    let downloaded: Vec<_> = wanted.into_iter().zip(downloaded).collect();
    let settled = upgrade::settle(
        loadout,
        manifest.entries.len(),
        outdated,
        choices,
        &downloaded,
    );
    for problem in &settled.problems {
        Warning::summon(grimoire, problem)?;
    }
    Ok(Some(settled))
}

/// Downloads `entries` from the host behind the download window, or nothing if there are none.
/// `None` if Esc was pressed or the download failed (which the user was told).
fn fetch(
    grimoire: &mut Grimoire,
    address: &Address,
    entries: Vec<Entry>,
    downloads_dir: PathBuf,
) -> Result<Option<Vec<Wad>>> {
    if entries.is_empty() {
        return Ok(Some(Vec::new()));
    }
    status::summon(
        grimoire,
        "Downloading",
        "Downloading...",
        theme::worn().colors.netplay,
    )?;
    // 🚪 Esc goes back to the launcher quietly: partial files are already cleaned up, and
    // finished ones stay in the downloads dir, so trying again picks up where this stopped
    match download_wads(grimoire, address, entries, downloads_dir) {
        Ok(wads) => Ok(Some(wads)),
        Err(Error::Cancelled) => Ok(None),
        Err(Error::Interrupted) => Err(Error::Interrupted),
        Err(_) => {
            Warning::download_failed(grimoire)?;
            Ok(None)
        }
    }
}

/// Offers to make a new profile, named by the user, from the host's wads: the ones already on this
/// machine (`local`) are used where they are, and only the rest are downloaded. With
/// `always_download` there is no asking first when some have to be downloaded.
fn download_profile(
    grimoire: &mut Grimoire,
    loadout: &mut Loadout,
    index: &mut usize,
    address: &Address,
    manifest: &Manifest,
    local: Vec<Option<Wad>>,
    always_download: bool,
) -> Result<Option<Profile>> {
    let total = local.len();
    let found = local.iter().flatten().count();
    let missing = total - found;
    let question = match (found, missing) {
        (0, 0) => "The host plays without wads. Make a profile for it?".to_string(),
        (_, 0) => format!("All {total} wads are already here. Make a profile from them?"),
        (0, _) => "You don't have the profile, would you like to download?".to_string(),
        _ => format!("{found} of {total} wads are already here. Download the other {missing}?"),
    };
    let asked = !(always_download && missing > 0);
    if asked && !Dialog::summon(grimoire, &question)? {
        return Ok(None);
    }
    let Some(profile_name) = ProfileNamer::summon(grimoire, &loadout.profiles)? else {
        return Ok(None);
    };
    let wanted: Vec<_> = manifest
        .entries
        .iter()
        .zip(&local)
        .filter(|(_, wad)| wad.is_none())
        .map(|(entry, _)| entry.clone())
        .collect();
    let Some(downloaded) = fetch(grimoire, address, wanted, loadout.downloads_dir())? else {
        return Ok(None);
    };
    let wads = fill_gaps(local, downloaded)
        .ok_or(Error::Download("Fewer wads arrived than were asked for"))?;
    let profile = Profile::new(profile_name, wads);
    if loadout.insert(index, profile.clone()).is_err() {
        Warning::save_failed(grimoire)?;
        return Ok(None);
    }
    Ok(Some(profile))
}

/// The host's wads in load order: the local ones where they were found, and each gap filled by the
/// next of `downloaded` (which are in the same order). `None` if there weren't enough downloads.
pub(crate) fn fill_gaps(local: Vec<Option<Wad>>, downloaded: Vec<Wad>) -> Option<Vec<Wad>> {
    let mut downloaded = downloaded.into_iter();
    local
        .into_iter()
        .map(|wad| wad.or_else(|| downloaded.next()))
        .collect()
}

/// The host's wads that are already on this machine, in the manifest's order, with `None` for each
/// one that isn't. A file that only might be one is hashed to make sure, in the hashing window.
/// `None` if Esc was pressed.
fn find_local(
    grimoire: &mut Grimoire,
    loadout: &mut Loadout,
    manifest: &Manifest,
) -> Result<Option<Vec<Option<Wad>>>> {
    let lookup = loadout.lookup();
    let checks: Vec<_> = manifest
        .entries
        .iter()
        .map(|entry| lookup.check(entry))
        .collect();
    let to_hash = checks
        .iter()
        .map(|check| match check {
            Local::Maybe(paths) => paths.len(),
            Local::Intact(_) => 0,
        })
        .sum();
    hashing::watch(grimoire, to_hash, |tally| {
        checks
            .into_iter()
            .zip(&manifest.entries)
            .map(|(check, entry)| match check {
                Local::Intact(wad) => Ok(Some(wad)),
                Local::Maybe(paths) => first_match(paths, entry, tally),
            })
            .collect()
    })
}

/// The first of `paths` whose contents hash to `entry`'s hash. A file that can't be read just isn't
/// the one; only Esc or Ctrl+C stop the search.
pub(crate) fn first_match(
    paths: Vec<PathBuf>,
    entry: &Entry,
    tally: &mut impl Tally,
) -> Result<Option<Wad>> {
    for path in paths {
        match Wad::hashed(&path, tally) {
            Ok(wad) if wad.hash == entry.hash => return Ok(Some(wad)),
            Err(error @ (Error::Cancelled | Error::Interrupted)) => return Err(error),
            _ => {}
        }
    }
    Ok(None)
}

/// Downloads the host's wads on a worker thread while this one draws the progress and listens for
/// Esc, so neither a slow host nor a dead one can freeze the screen.
fn download_wads(
    grimoire: &mut Grimoire,
    address: &Address,
    entries: Vec<Entry>,
    downloads_dir: PathBuf,
) -> Result<Vec<Wad>> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let count = entries.len() as u64;
    let reports =
        net::download_in_background(address.clone(), entries, downloads_dir, cancelled.clone());
    let mut progress =
        Progress::new(grimoire, Kind::Downloading, count).with_footer(CancelHandler::HINT);
    loop {
        let step = match reports.recv_timeout(Progress::FRAME) {
            Ok(Report::File(name, size)) => progress.count_file(&name, size),
            Ok(Report::Bytes(amount)) => progress.count_bytes(amount),
            Ok(Report::Done(result)) => return result,
            Err(RecvTimeoutError::Timeout) => progress.tick(),
            Err(RecvTimeoutError::Disconnected) => {
                return Err(Error::Download("The download stopped unexpectedly"));
            }
        };
        // 🛑 the worker notices at its next chunk, and cleans up after itself
        if let Err(error) = step.and_then(|()| CancelHandler::check()) {
            cancelled.store(true, Ordering::SeqCst);
            return Err(error);
        }
    }
}
