//! Sharing profiles as `.scroll` files: sealing one up to send, and breaking the seal on one
//! received, finding its wads in the wad folders.

use std::path::{Path, PathBuf};

use crate::{
    error::Result,
    picker,
    popups::{dialog::Dialog, hashing, progress::Kind, warning::Warning},
    ritual::Grimoire,
    screens::profile_namer::ProfileNamer,
    vault::{
        Vault,
        edition::Edition,
        profile::Profile,
        scroll::{self, Scroll},
    },
};

const FILTER: picker::Filter = ("scroll", &[scroll::EXTENSION]);

/// Saves the profile at `index` as a scroll, wherever the user picks.
pub fn seal(grimoire: &mut Grimoire, vault: &Vault, index: usize) -> Result<()> {
    let profile = &vault.profiles[index];
    let scroll = match Scroll::of(profile) {
        Ok(scroll) => scroll,
        Err(problem) => return Warning::summon(grimoire, &problem),
    };
    let Some(path) = picker::save(grimoire, Some(FILTER), &Scroll::file_name(&profile.name))?
    else {
        return Ok(());
    };
    let path = with_extension(path);
    if scroll.save(&path).is_err() {
        return Warning::save_failed(grimoire);
    }
    Warning::announce(
        grimoire,
        "sealed",
        &[
            format!("{} is sealed in", profile.name),
            path.display().to_string(),
        ],
    )
}

/// `path`, ending in `.scroll` even if the user typed a name without it.
fn with_extension(path: PathBuf) -> PathBuf {
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

/// Reads a scroll the user picks and makes a profile from it at `index`. Its wads are looked for
/// in the session's wad folders only; with none, the user is asked for a folder to look in.
pub fn unseal(grimoire: &mut Grimoire, vault: &mut Vault, index: &mut usize) -> Result<()> {
    let Some(path) = picker::file(grimoire, Some(FILTER), None)? else {
        return Ok(());
    };
    unseal_file(grimoire, vault, index, &path)
}

/// [`unseal`], for the scroll at `path` rather than one picked, as `--unseal` asks for.
pub fn unseal_file(
    grimoire: &mut Grimoire,
    vault: &mut Vault,
    index: &mut usize,
    path: &Path,
) -> Result<()> {
    let Some(scroll) = read(grimoire, path)? else {
        return Ok(());
    };
    let Some(folders) = folders_to_look_in(grimoire, vault)? else {
        return Ok(());
    };
    let total = scroll.wads.len() + usize::from(scroll.base_wad.is_some());
    let Some(unsealed) = hashing::watch_as(grimoire, Kind::Unsealing, total, |tally| {
        scroll.unseal(&folders, tally)
    })?
    else {
        return Ok(());
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
    let Some(name) = ProfileNamer::summon_suggesting(grimoire, &vault.profiles, &scroll.name)?
    else {
        return Ok(());
    };
    // 🧭 a base wad that is the global one already is left to the global setting
    let base_wad = unsealed.base_wad.filter(|wad| {
        vault
            .config
            .base_wad
            .as_ref()
            .is_none_or(|global| global.hash != wad.hash)
    });
    let profile = Profile {
        base_wad,
        host_defaults: scroll.host_defaults,
        ..Profile::new(name, unsealed.wads)
    };
    if vault.insert(index, profile).is_err() {
        Warning::save_failed(grimoire)?;
    }
    Ok(())
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

/// The session's wad folders, or, with none, one folder the user picks for this scroll.
/// `None` if they back out.
fn folders_to_look_in(grimoire: &mut Grimoire, vault: &Vault) -> Result<Option<Vec<PathBuf>>> {
    let folders: Vec<_> = vault
        .wad_folders
        .iter()
        .filter(|folder| folder.is_dir())
        .cloned()
        .collect();
    if !folders.is_empty() {
        return Ok(Some(folders));
    }
    if !Dialog::summon(
        grimoire,
        "No wad folders were given with --wads. Pick a folder to look for the wads in?",
    )? {
        return Ok(None);
    }
    Ok(picker::folder(grimoire, None)?.map(|folder| vec![folder]))
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
