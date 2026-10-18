//! The ini a host plays with, served at `/<code>/ini` when the host shares it.

use std::{
    ffi::OsStr,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

/// The biggest ini that is served. A well-used `uzdoom.ini` is a few hundred KB at most.
pub const MAX_INI: u64 = 1024 * 1024;

/// The ini UZDoom plays the profile with: the profile's own (`profile_ini`), or else UZDoom's
/// usual one, if one of the places it keeps it has one. `uzdoom` is the UZDoom that will run,
/// since on Windows the ini can be beside it.
pub fn in_use(profile_ini: Option<&OsStr>, uzdoom: &OsStr) -> Option<PathBuf> {
    match profile_ini {
        Some(ini) => Some(PathBuf::from(ini)).filter(|ini| ini.is_file()),
        None => usual_places(Path::new(uzdoom))
            .into_iter()
            .find(|ini| ini.is_file()),
    }
}

/// Where UZDoom keeps its ini when it isn't given one, in the order it looks.
#[cfg(all(unix, not(target_os = "macos")))]
fn usual_places(_uzdoom: &Path) -> Vec<PathBuf> {
    directories::BaseDirs::new()
        .map(|dirs| dirs.config_dir().join("uzdoom/uzdoom.ini"))
        .into_iter()
        .collect()
}

/// Where UZDoom keeps its ini when it isn't given one, in the order it looks.
#[cfg(target_os = "macos")]
fn usual_places(_uzdoom: &Path) -> Vec<PathBuf> {
    directories::BaseDirs::new()
        .map(|dirs| dirs.home_dir().join("Library/Preferences/uzdoom.ini"))
        .into_iter()
        .collect()
}

/// Where UZDoom keeps its ini when it isn't given one, in the order it looks: beside it for a
/// portable install, then in the documents.
#[cfg(windows)]
fn usual_places(uzdoom: &Path) -> Vec<PathBuf> {
    let beside = uzdoom.parent().unwrap_or(Path::new(""));
    let documents = directories::UserDirs::new().and_then(|dirs| {
        dirs.document_dir()
            .map(|dir| dir.join("My Games/UZDoom/uzdoom.ini"))
    });
    [beside.join("uzdoom_portable.ini")]
        .into_iter()
        .chain(documents)
        .chain([beside.join("uzdoom.ini")])
        .collect()
}

/// The ini at `path`, to be served. One bigger than [`MAX_INI`] is turned down.
pub fn read(path: &Path) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_INI + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_INI {
        return Err(io::Error::other("the ini is too big to share"));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::scribe::tests::Altar;

    #[test]
    fn the_profiles_own_ini_is_the_one_in_use_if_it_is_there() {
        let altar = Altar::new("ini-in-use");
        let ini = altar.file("uzdoom.ini");
        fs::write(&ini, b"[Doom.Player]").unwrap();
        let uzdoom = OsStr::new("/usr/bin/uzdoom");
        assert_eq!(in_use(Some(ini.as_os_str()), uzdoom), Some(ini.clone()));
        // a missing one isn't swapped for UZDoom's usual one, since it isn't what UZDoom plays with
        assert_eq!(
            in_use(Some(altar.file("gone.ini").as_os_str()), uzdoom),
            None
        );
    }

    #[test]
    fn an_ini_too_big_is_not_served() {
        let altar = Altar::new("ini-too-big");
        let ini = altar.file("uzdoom.ini");
        fs::write(&ini, b"[Doom.Player]\nname=Player\n").unwrap();
        assert_eq!(read(&ini).unwrap(), b"[Doom.Player]\nname=Player\n");
        fs::write(&ini, vec![b'x'; MAX_INI as usize + 1]).unwrap();
        assert!(read(&ini).is_err());
    }
}
