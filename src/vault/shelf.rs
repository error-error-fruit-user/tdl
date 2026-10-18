//! What the wads screen works on: the dirs wads are looked for in (the session's wad dirs, then
//! the downloads), the wad files in them, and copying and moving those files between them. The
//! screen changes a [`Shelf`] of its own, and hands it back to the vault (see
//! [`Vault::restock`](crate::vault::Vault::restock)), which keeps the dirs and points every
//! profile's wads at wherever their files were moved.

use std::{
    collections::HashMap,
    ffi::OsString,
    fs, io,
    path::{Component, Path, PathBuf},
};

use crate::{error::Result, tag, vault::wad::Wad};

/// The dirs wads are looked for in, and what happened to them on the wads screen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shelf {
    /// This session's wad dirs, in order: the ones passed with `--wads`, or the kept ones.
    pub dirs: Vec<PathBuf>,
    /// Where downloaded wads are kept, by hash. Always a source, and never taken out.
    pub downloads: PathBuf,
    /// The wad dirs kept for this vault, and for every vault, as they are to be saved.
    pub kept_vault: Vec<PathBuf>,
    pub kept_global: Vec<PathBuf>,
    /// False if `global.rite` is from a newer tdl, so no dir can be kept for every vault.
    pub global_writable: bool,
    /// Every file moved by a cut and paste, from where to where, in the order they moved.
    pub moves: Vec<(PathBuf, PathBuf)>,
}

/// Where a wad dir added on the wads screen is kept: just for this session, for every vault, or
/// for this one, as with `--wads`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Keep {
    Session,
    Global,
    Vault,
}

impl Shelf {
    /// Every place wads are looked for: the wad dirs, then the downloads.
    pub fn sources(&self) -> Vec<PathBuf> {
        self.dirs
            .iter()
            .cloned()
            .chain([self.downloads.clone()])
            .collect()
    }

    /// Whether `dir` is already looked in.
    pub fn has(&self, dir: &Path) -> bool {
        dir == self.downloads || self.dirs.iter().any(|known| known == dir)
    }

    /// Looks in `dir` for wads from now on, after the others, and keeps it as `keep` says. The
    /// kept lists are the ones that are used whenever tdl starts without `--wads`, so a dir kept
    /// for the vault or globally goes there too.
    pub fn add(&mut self, dir: PathBuf, keep: Keep) {
        let kept = match keep {
            Keep::Session => None,
            Keep::Global => Some(&mut self.kept_global),
            Keep::Vault => Some(&mut self.kept_vault),
        };
        if let Some(kept) = kept
            && !kept.contains(&dir)
        {
            kept.push(dir.clone());
        }
        if !self.has(&dir) {
            self.dirs.push(dir);
        }
    }

    /// Stops looking in `dir` for wads, now and in later sessions: it is forgotten wherever it was
    /// kept. The files in it stay where they are. The downloads can't be taken out.
    pub fn remove(&mut self, dir: &Path) {
        self.dirs.retain(|known| known != dir);
        self.kept_vault.retain(|known| known != dir);
        self.kept_global.retain(|known| known != dir);
    }
}

/// Whether `path` names a wad by its extension, whatever its case.
pub fn is_wad(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        Wad::EXTENSIONS
            .iter()
            .any(|known| extension.eq_ignore_ascii_case(known))
    })
}

/// Every wad file under `dir`, at any depth, sorted by path. Dirs that can't be read are passed
/// over, and so are links to dirs, so a link back up can't send the search round forever.
pub fn scan(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut waiting = vec![dir.to_path_buf()];
    while let Some(dir) = waiting.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => waiting.push(path),
                Ok(_) if path.is_file() && is_wad(&path) => found.push(path),
                _ => {}
            }
        }
    }
    found.sort();
    found
}

/// Whether `path` is a wad kept in `downloads`, which stores each in a dir named after its hash.
pub fn is_download(path: &Path, downloads: &Path) -> bool {
    path.parent()
        .and_then(Path::parent)
        .is_some_and(|dir| dir == downloads)
}

/// How a download is told apart in a list: `.downloads` and the start of its hash.
pub fn download_tag(path: &Path) -> String {
    let hash = path
        .parent()
        .and_then(Path::file_name)
        .map(|hash| hash.to_string_lossy().into_owned())
        .unwrap_or_default();
    format!(".downloads {}", hash.get(..8).unwrap_or(&hash))
}

/// The dirs `path` is in, nearest first, as they would be shown: a download's is only
/// [`download_tag`], since the dirs above it say nothing about it.
fn dirs_above(path: &Path, downloads: &Path) -> Vec<String> {
    if is_download(path, downloads) {
        return vec![download_tag(path)];
    }
    let Some(parent) = path.parent() else {
        return Vec::new();
    };
    parent
        .components()
        .rev()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect()
}

/// For each of `paths`, how to tell it from the others with the same file name: the fewest of
/// the dirs it is in, counting up from the nearest, that none of the others ends in too (like
/// `old` for `/wads/old/brutal.pk3` beside `/wads/new/brutal.pk3`). `None` for a file name no
/// other has.
pub fn hints(paths: &[PathBuf], downloads: &Path) -> Vec<Option<String>> {
    let mut by_name: HashMap<&std::ffi::OsStr, Vec<usize>> = HashMap::new();
    for (i, path) in paths.iter().enumerate() {
        if let Some(name) = path.file_name() {
            by_name.entry(name).or_default().push(i);
        }
    }
    let mut hints = vec![None; paths.len()];
    for same in by_name.values().filter(|same| same.len() > 1) {
        let above: Vec<Vec<String>> = same
            .iter()
            .map(|&i| dirs_above(&paths[i], downloads))
            .collect();
        for (n, &i) in same.iter().enumerate() {
            let mine = &above[n];
            // 🔍 the shortest run of dirs that is this one's alone; all of them if none is
            let depth = (1..=mine.len())
                .find(|&depth| {
                    let end = &mine[..depth];
                    above.iter().enumerate().all(|(m, theirs)| {
                        m == n || theirs.get(..depth).is_none_or(|theirs| theirs != end)
                    })
                })
                .unwrap_or(mine.len());
            let shown: Vec<&str> = mine[..depth].iter().rev().map(String::as_str).collect();
            hints[i] = Some(shown.join("/"));
        }
    }
    hints
}

/// Copies the file at `from` to `to`, making `to`'s dir if it is missing. The copy is written
/// beside it as `.part` first and only then renamed, so a failed copy never leaves half a wad
/// under the real name, nor touches a file already there.
pub fn copy(from: &Path, to: &Path) -> io::Result<()> {
    if let Some(dir) = to.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut part = OsString::from(to.as_os_str());
    part.push(".part");
    let part = PathBuf::from(part);
    if let Err(error) = fs::copy(from, &part).and_then(|_| fs::rename(&part, to)) {
        let _ = fs::remove_file(&part);
        return Err(error);
    }
    Ok(())
}

/// Moves the file at `from` to `to`, copying it and deleting the original when they are on
/// different disks. A download's own dir goes too once it is empty, since it only ever held that
/// one wad.
pub fn relocate(from: &Path, to: &Path, downloads: &Path) -> io::Result<()> {
    if let Some(dir) = to.parent() {
        fs::create_dir_all(dir)?;
    }
    if fs::rename(from, to).is_err() {
        copy(from, to)?;
        fs::remove_file(from)?;
    }
    if is_download(from, downloads)
        && let Some(dir) = from.parent()
    {
        let _ = fs::remove_dir(dir);
    }
    Ok(())
}

/// Whether `dir` holds an entry named exactly `name`. Asked of the dir's own list rather than of
/// the path, so a rename that only changes a name's case isn't turned down on a disk that
/// ignores case.
pub fn holds(dir: &Path, name: &str) -> bool {
    fs::read_dir(dir).is_ok_and(|entries| {
        entries
            .flatten()
            .any(|entry| entry.file_name() == std::ffi::OsStr::new(name))
    })
}

/// Renames the wad file at `from` to `name`, in the same dir, and returns where it is now. Turned
/// down if something by that name is already there, so nothing is ever written over.
pub fn rename(from: &Path, name: &str) -> io::Result<PathBuf> {
    let dir = from.parent().unwrap_or(Path::new(""));
    if holds(dir, name) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{name} is already there"),
        ));
    }
    let to = dir.join(name);
    fs::rename(from, &to)?;
    Ok(to)
}

/// Deletes the wad file at `path`, and its download's own dir with it once that is empty.
pub fn delete(path: &Path, downloads: &Path) -> io::Result<()> {
    fs::remove_file(path)?;
    if is_download(path, downloads)
        && let Some(dir) = path.parent()
    {
        let _ = fs::remove_dir(dir);
    }
    Ok(())
}

/// What to do with a file pasted where there already is one by its name.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Clash {
    Replace,
    /// Paste it beside the other, with a tag on its name (`brutal_2.pk3`).
    KeepBoth,
    Skip,
}

/// Where pasted files go: into a dir, or into the downloads, each under its own hash (given in
/// the same order as the files).
pub enum Destination<'a> {
    Dir(&'a Path),
    Downloads(&'a [String]),
}

/// What a paste came to.
#[derive(Debug, Default, PartialEq)]
pub struct Pasted {
    /// Where each file that was pasted went.
    pub placed: Vec<PathBuf>,
    /// For a cut, each file that moved, from where to where.
    pub moves: Vec<(PathBuf, PathBuf)>,
    /// What went wrong, one line each, for the user.
    pub problems: Vec<String>,
}

/// Copies the files at `paths` into `into`, or moves them there if `cut`. `clash` decides what
/// happens to one whose name is already taken in a dir; in the downloads, the same name under the
/// same hash is the same wad, so it is simply put there again. A file pasted onto itself stays as it
/// is, and one that has gone since it was copied is said and left out.
pub fn paste(
    paths: &[PathBuf],
    cut: bool,
    into: Destination,
    downloads: &Path,
    mut clash: impl FnMut(&str, &Path) -> Result<Clash>,
) -> Result<Pasted> {
    let mut pasted = Pasted::default();
    for (n, from) in paths.iter().enumerate() {
        let Some(name) = from.file_name() else {
            continue;
        };
        let shown = name.to_string_lossy();
        if !from.is_file() {
            pasted
                .problems
                .push(format!("{} is gone, so it wasn't pasted", from.display()));
            continue;
        }
        let dir = match into {
            Destination::Dir(dir) => dir.to_path_buf(),
            Destination::Downloads(hashes) => downloads.join(&hashes[n]),
        };
        let mut to = dir.join(name);
        if to == *from {
            continue;
        }
        if to.exists() && matches!(into, Destination::Dir(_)) {
            match clash(&shown, &dir)? {
                Clash::Replace => {}
                Clash::KeepBoth => {
                    to = dir.join(tag::tagged_file(&shown, |tagged| {
                        !dir.join(tagged).exists()
                    }));
                }
                Clash::Skip => continue,
            }
        }
        let done = if cut {
            relocate(from, &to, downloads)
        } else {
            copy(from, &to)
        };
        match done {
            Ok(()) => {
                if cut {
                    pasted.moves.push((from.clone(), to.clone()));
                }
                pasted.placed.push(to);
            }
            Err(error) => {
                let verb = if cut { "move" } else { "copy" };
                pasted
                    .problems
                    .push(format!("Could not {verb} {shown}: {error}"));
            }
        }
    }
    Ok(pasted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::scribe::tests::Altar;

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, path.to_string_lossy().as_bytes()).unwrap();
    }

    #[test]
    fn a_scan_finds_every_wad_at_any_depth_and_nothing_else() {
        let altar = Altar::new("shelf-scan");
        let dir = altar.file("wads");
        for name in [
            "b.wad",
            "a.PK3",
            "maps/deep/c.WAD",
            "notes.txt",
            "d.wad.part",
        ] {
            touch(&dir.join(name));
        }
        // a dir named like a wad is still a dir
        fs::create_dir_all(dir.join("e.wad")).unwrap();
        let found: Vec<_> = scan(&dir)
            .iter()
            .map(|path| path.strip_prefix(&dir).unwrap().to_path_buf())
            .collect();
        let expected: Vec<PathBuf> = ["a.PK3", "b.wad", "maps/deep/c.WAD"]
            .iter()
            .map(PathBuf::from)
            .collect();
        assert_eq!(found, expected);
        assert!(scan(&altar.file("nowhere")).is_empty());
    }

    #[test]
    fn a_rename_stays_in_the_dir_and_never_writes_over_a_file() {
        let altar = Altar::new("shelf-rename");
        let dir = altar.file("wads");
        touch(&dir.join("brutal.pk3"));
        touch(&dir.join("sigil.wad"));

        let renamed = rename(&dir.join("brutal.pk3"), "brutal dev.pk3").unwrap();
        assert_eq!(renamed, dir.join("brutal dev.pk3"));
        assert!(renamed.is_file() && !dir.join("brutal.pk3").exists());

        assert!(rename(&renamed, "sigil.wad").is_err());
        assert_eq!(
            fs::read(dir.join("sigil.wad")).unwrap(),
            dir.join("sigil.wad").to_string_lossy().as_bytes()
        );
        // 🔠 only a case change: the file itself isn't in the way
        assert_eq!(
            rename(&renamed, "Brutal Dev.pk3").unwrap(),
            dir.join("Brutal Dev.pk3")
        );
    }

    #[test]
    fn wads_with_the_same_name_are_told_apart_by_the_fewest_dirs_that_differ() {
        let downloads = PathBuf::from("/data/.downloads");
        let paths: Vec<PathBuf> = [
            "/home/me/wads/old/brutal.pk3",
            "/home/me/wads/new/brutal.pk3",
            "/home/me/mods/new/brutal.pk3",
            "/data/.downloads/0123456789abcdef/brutal.pk3",
            "/home/me/wads/sigil.wad",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        let hints = hints(&paths, &downloads);
        assert_eq!(
            hints,
            [
                Some("old".to_string()),
                Some("wads/new".to_string()),
                Some("mods/new".to_string()),
                Some(".downloads 01234567".to_string()),
                None,
            ]
        );
    }

    #[test]
    fn a_wad_in_a_dir_above_anothers_shows_every_dir_it_has() {
        let paths: Vec<PathBuf> = ["/wads/a.wad", "/mods/wads/a.wad"]
            .iter()
            .map(PathBuf::from)
            .collect();
        let hints = hints(&paths, Path::new("/downloads"));
        assert_eq!(hints, [Some("wads".into()), Some("mods/wads".into())]);
    }

    #[test]
    fn a_dir_added_is_kept_where_asked_and_a_dir_removed_is_forgotten_everywhere() {
        let mut shelf = Shelf {
            dirs: vec!["/wads".into()],
            downloads: "/data/.downloads".into(),
            kept_global: vec!["/wads".into()],
            ..Shelf::default()
        };
        shelf.add("/mods".into(), Keep::Vault);
        shelf.add("/maps".into(), Keep::Session);
        shelf.add("/wads".into(), Keep::Global);
        assert_eq!(
            shelf.dirs,
            [PathBuf::from("/wads"), "/mods".into(), "/maps".into()]
        );
        assert_eq!(shelf.kept_vault, [PathBuf::from("/mods")]);
        assert_eq!(shelf.kept_global, [PathBuf::from("/wads")]);
        assert!(shelf.has(Path::new("/data/.downloads")));
        assert_eq!(
            shelf.sources().last(),
            Some(&PathBuf::from("/data/.downloads"))
        );

        shelf.remove(Path::new("/wads"));
        shelf.remove(Path::new("/mods"));
        assert_eq!(shelf.dirs, [PathBuf::from("/maps")]);
        assert!(shelf.kept_vault.is_empty() && shelf.kept_global.is_empty());
    }

    #[test]
    fn pasting_copies_or_moves_and_a_taken_name_is_asked_about() {
        let altar = Altar::new("shelf-paste");
        let downloads = altar.file("data/.downloads");
        let (wads, mods) = (altar.file("wads"), altar.file("mods"));
        for name in ["a.wad", "b.wad"] {
            touch(&wads.join(name));
        }
        touch(&mods.join("b.wad"));
        let paths = [
            wads.join("a.wad"),
            wads.join("b.wad"),
            wads.join("gone.wad"),
        ];

        // b.wad is taken in mods: keep both
        let mut asked = Vec::new();
        let copied = paste(
            &paths,
            false,
            Destination::Dir(&mods),
            &downloads,
            |name, dir| {
                asked.push((name.to_string(), dir.to_path_buf()));
                Ok(Clash::KeepBoth)
            },
        )
        .unwrap();
        assert_eq!(asked, [("b.wad".to_string(), mods.clone())]);
        assert_eq!(copied.placed, [mods.join("a.wad"), mods.join("b_2.wad")]);
        assert!(copied.moves.is_empty());
        assert_eq!(copied.problems.len(), 1, "the gone one is said");
        assert!(wads.join("a.wad").is_file(), "a copy leaves the original");

        // cut into mods again: a.wad is replaced, b.wad skipped
        let moved = paste(
            &paths[..2],
            true,
            Destination::Dir(&mods),
            &downloads,
            |name, _| {
                Ok(if name == "a.wad" {
                    Clash::Replace
                } else {
                    Clash::Skip
                })
            },
        )
        .unwrap();
        assert_eq!(moved.moves, [(wads.join("a.wad"), mods.join("a.wad"))]);
        assert!(!wads.join("a.wad").exists());
        assert!(wads.join("b.wad").is_file());

        // pasted onto itself, nothing happens and nothing is asked
        let same = paste(
            &[mods.join("a.wad")],
            true,
            Destination::Dir(&mods),
            &downloads,
            |_, _| panic!("nothing to ask about"),
        )
        .unwrap();
        assert_eq!(same, Pasted::default());

        // into the downloads, under its hash
        let hashes = ["feedface".to_string()];
        let stored = paste(
            &[mods.join("a.wad")],
            false,
            Destination::Downloads(&hashes),
            &downloads,
            |_, _| panic!("the downloads never ask"),
        )
        .unwrap();
        assert_eq!(stored.placed, [downloads.join("feedface").join("a.wad")]);
        assert!(is_download(&stored.placed[0], &downloads));
    }

    #[test]
    fn deleting_a_download_takes_its_hash_dir_but_never_the_downloads() {
        let altar = Altar::new("shelf-delete");
        let downloads = altar.file("data/.downloads");
        let download = downloads.join("abcdef0123456789").join("got.wad");
        let wad = altar.file("wads/a.wad");
        touch(&download);
        touch(&wad);
        delete(&download, &downloads).unwrap();
        delete(&wad, &downloads).unwrap();
        assert!(!download.parent().unwrap().exists());
        assert!(downloads.is_dir());
        assert!(
            !wad.exists() && altar.file("wads").is_dir(),
            "a wad dir stays, empty or not"
        );
        assert!(delete(&wad, &downloads).is_err(), "nothing left to delete");
    }

    #[test]
    fn a_copy_leaves_the_original_and_a_move_takes_an_emptied_download_dir_with_it() {
        let altar = Altar::new("shelf-files");
        let downloads = altar.file("data/.downloads");
        let download = downloads.join("abcdef0123456789").join("got.wad");
        touch(&download);

        let copied = altar.file("wads/sub/got.wad");
        copy(&download, &copied).unwrap();
        assert_eq!(fs::read(&copied).unwrap(), fs::read(&download).unwrap());
        assert!(download.is_file());
        assert!(!altar.file("wads/sub/got.wad.part").exists());

        let moved = altar.file("mods/got.wad");
        relocate(&download, &moved, &downloads).unwrap();
        assert!(moved.is_file());
        assert!(!download.exists());
        assert!(
            !download.parent().unwrap().exists(),
            "its hash dir went with it"
        );
        assert!(downloads.is_dir(), "but never the downloads themselves");

        // a failed copy leaves nothing behind
        assert!(copy(&altar.file("missing.wad"), &altar.file("wads/x.wad")).is_err());
        assert!(!altar.file("wads/x.wad").exists());
        assert!(!altar.file("wads/x.wad.part").exists());
    }
}
