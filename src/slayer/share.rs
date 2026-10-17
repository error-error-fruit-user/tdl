//! Sharing profiles as `.scroll` files: sealing some (or a whole vault) up to send, and breaking
//! the seal on one received, finding their wads in the wad dirs.

use std::path::{Path, PathBuf};

use crate::{
    cli::Ask,
    error::Result,
    picker,
    popups::{dialog::Dialog, hashing, progress::Kind, warning::Warning},
    ritual::Grimoire,
    screens::profile_namer::{ProfileNamer, ask_copy_name},
    vault::{
        Vault,
        edition::Edition,
        global::Shared,
        profile::Profile,
        scroll::{self, Scroll, UnsealedProfile},
    },
};

const FILTER: picker::Filter = ("scroll", &[scroll::EXTENSION]);

/// Saves the profiles at `rows` (in that order) as one scroll, wherever the user picks, offered
/// as `<name>.scroll`. `settings` (the user's global settings, never their song) go on it too only
/// if the user says so, so whoever opens it isn't handed settings they didn't ask for.
pub fn seal(
    grimoire: &mut Grimoire,
    vault: &Vault,
    rows: &[usize],
    name: &str,
    settings: Shared,
) -> Result<()> {
    let mut scroll = match Scroll::of(rows.iter().map(|&row| &vault.profiles[row])) {
        Ok(scroll) => scroll,
        Err(problem) => return Warning::summon(grimoire, &problem),
    };
    let question = format!(
        "Seal your global settings in too? ({}; never your song)",
        settings.summary()
    );
    if Dialog::summon(grimoire, &question)? {
        scroll.settings = Some(settings);
    }
    let Some(path) = picker::save(grimoire, Some(FILTER), &Scroll::file_name(name))? else {
        return Ok(());
    };
    let path = with_extension(path);
    if scroll.save(&path).is_err() {
        return Warning::save_failed(grimoire);
    }
    let what = match rows {
        [row] => format!("{} is", vault.profiles[*row].name),
        _ => format!("{} profiles are", rows.len()),
    };
    Warning::announce(
        grimoire,
        "sealed",
        &[format!("{what} sealed in"), path.display().to_string()],
    )
}

/// `path`, ending in `.scroll` even if the user typed a name without it.
pub fn with_extension(path: PathBuf) -> PathBuf {
    if path
        .extension()
        .is_some_and(|extension| extension == scroll::EXTENSION)
    {
        path
    } else {
        let mut named = path.into_os_string();
        named.push(".");
        named.push(scroll::EXTENSION);
        named.into()
    }
}

/// Reads a scroll the user picks and adds its profiles at `index`. Their wads are looked for in the
/// session's wad dirs only; with none, the user is asked for a dir to look in.
///
/// If the scroll carries its maker's global settings and they differ from `current`, they are
/// returned, for the caller to put in place of the user's; a scroll without any leaves the user's
/// as they are.
pub fn unseal(
    grimoire: &mut Grimoire,
    vault: &mut Vault,
    index: &mut usize,
    current: &Shared,
) -> Result<Option<Shared>> {
    let Some(path) = picker::file(grimoire, Some(FILTER), None)? else {
        return Ok(None);
    };
    unseal_file(grimoire, vault, index, &path, current, None)
}

/// [`unseal`], for the scroll at `path` rather than one picked, as `--unseal` asks for. With
/// `replacing` (`--unseal-delete`), every profile in the vault is deleted to make room for the
/// scroll's, once they are ready to go in, after asking if it says to.
pub fn unseal_file(
    grimoire: &mut Grimoire,
    vault: &mut Vault,
    index: &mut usize,
    path: &Path,
    current: &Shared,
    replacing: Option<Ask>,
) -> Result<Option<Shared>> {
    let Some(scroll) = read(grimoire, path)? else {
        return Ok(None);
    };
    let Some(dirs) = dirs_to_look_in(grimoire, vault)? else {
        return Ok(None);
    };
    let total = scroll.wad_count();
    let Some(unsealed) = hashing::watch_as(grimoire, Kind::Unsealing, total, |tally| {
        scroll.unseal(&dirs, tally)
    })?
    else {
        return Ok(None);
    };
    if unsealed.missing > 0 {
        Warning::summon_lines(
            grimoire,
            &[
                format!("Found {} of {total} wads.", total - unsealed.missing),
                "The rest are marked in the editor: Space on one says where it was looked for, and R picks it."
                    .to_string(),
            ],
        )?;
    }
    let profiles: Vec<Profile> = unsealed
        .profiles
        .into_iter()
        .map(|unsealed| into_profile(vault, unsealed))
        .collect();
    let added = match <[Profile; 1]>::try_from(profiles) {
        // 📜 one profile is named as it always was, starting from the scroll's name
        Ok([profile]) => {
            // 🧹 the profiles being replaced take no names from it
            let taken = if replacing.is_some() {
                &[][..]
            } else {
                &vault.profiles[..]
            };
            let Some(name) = ProfileNamer::summon_suggesting(grimoire, taken, &profile.name)?
            else {
                return Ok(None);
            };
            if let Some(ask) = replacing
                && !clear(grimoire, vault, index, ask)?
            {
                return Ok(None);
            }
            if vault.insert(index, Profile { name, ..profile }).is_err() {
                Warning::save_failed(grimoire)?;
                return Ok(None);
            }
            1
        }
        // 📚 several keep their names, and only the ones that are taken are asked about
        Err(profiles) => {
            if let Some(ask) = replacing
                && !clear(grimoire, vault, index, ask)?
            {
                return Ok(None);
            }
            let inserted = vault.insert_copies(index, profiles, |profiles, name, tagged| {
                ask_copy_name(grimoire, profiles, name, tagged)
            })?;
            if inserted.failed {
                Warning::save_failed(grimoire)?;
            }
            inserted.count
        }
    };
    if added == 0 {
        return Ok(None);
    }
    // ⚙️ only once a profile is in: backing out of the import shouldn't change any settings
    Ok(scroll.settings.filter(|settings| settings != current))
}

/// For `--unseal-delete`: deletes every profile in `vault`, with its saves, to make room for a
/// scroll's, asking first if `ask` says to, and puts `index` at the top. Returns whether they are
/// gone; the downloaded wads and the global settings always stay.
fn clear(grimoire: &mut Grimoire, vault: &mut Vault, index: &mut usize, ask: Ask) -> Result<bool> {
    if ask == Ask::First {
        let question = match vault.profiles.as_slice() {
            [] => "Unseal the scroll into this empty vault?".to_string(),
            [only] => format!(
                "Delete '{}' and its saves, and put the scroll's profiles in its place?",
                only.name
            ),
            all => format!(
                "Delete the {} profiles in this vault and their saves, and put the scroll's in \
                 their place?",
                all.len()
            ),
        };
        if !Dialog::summon(grimoire, &question)? {
            return Ok(false);
        }
    }
    if vault.remove_all().is_err() {
        Warning::save_failed(grimoire)?;
        return Ok(false);
    }
    *index = 0;
    Ok(true)
}

/// A profile off a scroll, as it goes into `vault`. A base wad that is the global one already is
/// left to the global setting.
fn into_profile(vault: &Vault, unsealed: UnsealedProfile) -> Profile {
    let base_wad = unsealed.base_wad.filter(|wad| {
        vault
            .global
            .base_wad
            .as_ref()
            .is_none_or(|global| global.hash != wad.hash)
    });
    Profile {
        base_wad,
        host_defaults: unsealed.host_defaults,
        ..Profile::new(unsealed.name, unsealed.wads)
    }
}

/// The scroll at `path`, checked as safe to break, or `None` after telling the user why not.
fn read(grimoire: &mut Grimoire, path: &Path) -> Result<Option<Scroll>> {
    if !path.is_file() {
        Warning::summon(
            grimoire,
            &format!("There is no scroll at {}", path.display()),
        )?;
        return Ok(None);
    }
    if Scroll::stored_version(path).is_some_and(|version| version > Scroll::current()) {
        Warning::summon(grimoire, "The scroll is from a newer tdl, please update!")?;
        return Ok(None);
    }
    let Some(scroll) = Scroll::open(path) else {
        Warning::summon(grimoire, "That file isn't a scroll tdl can read")?;
        return Ok(None);
    };
    if let Err(problem) = scroll.check() {
        Warning::summon(grimoire, problem)?;
        return Ok(None);
    }
    Ok(Some(scroll))
}

/// The session's wad dirs, or, with none, one dir the user picks for this scroll.
/// `None` if they back out.
fn dirs_to_look_in(grimoire: &mut Grimoire, vault: &Vault) -> Result<Option<Vec<PathBuf>>> {
    let dirs: Vec<_> = vault
        .wad_dirs
        .iter()
        .filter(|dir| dir.is_dir())
        .cloned()
        .collect();
    if !dirs.is_empty() {
        return Ok(Some(dirs));
    }
    if !Dialog::summon(
        grimoire,
        "No wad dirs were given with --wads. Pick a dir to look for the wads in?",
    )? {
        return Ok(None);
    }
    Ok(picker::dir(grimoire, None)?.map(|dir| vec![dir]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scroll_always_ends_in_its_extension() {
        assert_eq!(
            with_extension("/tmp/brutal".into()),
            PathBuf::from("/tmp/brutal.scroll")
        );
        assert_eq!(
            with_extension("/tmp/brutal.scroll".into()),
            PathBuf::from("/tmp/brutal.scroll")
        );
        assert_eq!(
            with_extension("/tmp/brutal.v2".into()),
            PathBuf::from("/tmp/brutal.v2.scroll")
        );
    }
}
