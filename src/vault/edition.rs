//! Every saved file says which edition of the app wrote it, so newer builds can read older files
//! and older builds know to stay away from newer ones.
//!
//! # Making a breaking change
//! Adding a field with `#[serde(default)]` is not breaking. Anything that changes what existing
//! data means is: renaming or retyping a field, splitting one in two, changing a unit or a
//! default that old files relied on. For that:
//! 1. Write a function `vN_to_vM(old: Self) -> Self` that fixes up the data.
//! 2. Append it to the type's [`Edition::UPGRADES`]. The version number is the length of that list,
//!    so appending is what bumps it. Never reorder, edit or remove an entry.
//! 3. Add a test that loads a file saved by the previous edition.
//!
//! An upgrade can only see data the current struct can still hold. If a change makes old files
//! impossible to read into it at all, keep the old struct around and read that in the upgrade.

use std::{fs, io, path::Path};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    error::{Error, Result},
    vault::scribe,
};

/// Just enough of a file to read its edition, even if the rest isn't understood anymore.
#[derive(Deserialize)]
struct Glimpse {
    version: u32,
}

/// What actually gets written: the data, wearing its edition number.
#[derive(Deserialize, Serialize)]
struct Sealed<D> {
    version: u32,
    data: D,
}

/// What loading a file turned up. Nothing here ever deletes anything: a file that can't be read is
/// moved aside, never overwritten.
#[derive(Debug, PartialEq)]
pub enum Loaded<T> {
    /// Read normally and upgraded in memory to the current edition.
    Fine(T),
    /// There is no file, as on a first run.
    Missing,
    /// Saved by a newer tdl. Left exactly as it is.
    TooNew,
    /// The file was unreadable, so it was moved aside to `.corrupt` and its `.bak` put in its place.
    Restored(T),
    /// The file was unreadable and there was no usable backup. It was moved aside to `.corrupt`.
    Lost,
}

pub trait Edition: Serialize + DeserializeOwned + PartialEq + Sized + 'static {
    /// One function per breaking change. Entry `n` turns edition `n` into edition `n + 1`.
    /// Edition 0 is how files were saved before editions existed: bare positional arrays.
    const UPGRADES: &[fn(Self) -> Self];

    /// The edition this build writes.
    fn current() -> u32 {
        Self::UPGRADES.len() as u32
    }

    /// The edition a file was saved as, and its contents if they could be understood.
    /// `None` if the file is missing or isn't one of ours.
    fn read(path: &Path) -> Option<(u32, Option<Self>)> {
        let bytes = scribe::peruse(path).ok()?;
        match rmp_serde::from_slice::<Glimpse>(&bytes) {
            Ok(Glimpse { version }) => Some((
                version,
                rmp_serde::from_slice::<Sealed<Self>>(&bytes)
                    .ok()
                    .map(|sealed| sealed.data),
            )),
            // 📜 before editions, files were bare arrays with no version to glimpse
            Err(_) => rmp_serde::from_slice(&bytes)
                .ok()
                .map(|data| (0, Some(data))),
        }
    }

    /// The edition a file was saved as, without needing to understand the rest of it.
    fn stored_version(path: &Path) -> Option<u32> {
        Self::read(path).map(|(version, _)| version)
    }

    /// The file's contents, upgraded in memory to the current edition. Nothing is written.
    /// `None` if it is unreadable or from a newer edition than this build knows.
    fn open(path: &Path) -> Option<Self> {
        let (version, data) = Self::read(path)?;
        if version > Self::current() {
            return None;
        }
        Some(Self::upgraded(version, data?))
    }

    /// Runs every upgrade from `version` up to the current edition.
    fn upgraded(version: u32, data: Self) -> Self {
        Self::UPGRADES[version as usize..]
            .iter()
            .fold(data, |data, upgrade| upgrade(data))
    }

    /// Like [`Edition::open`], but says what went wrong and deals with it safely: an unreadable
    /// file is moved aside to `.corrupt` (so a later save can't overwrite it) and its `.bak`, if
    /// that reads, takes its place.
    fn load(path: &Path) -> Loaded<Self> {
        match Self::read(path) {
            Some((version, _)) if version > Self::current() => return Loaded::TooNew,
            Some((version, Some(data))) => return Loaded::Fine(Self::upgraded(version, data)),
            _ => {}
        }
        // 📭 nothing to read, or something there that couldn't be read?
        let old_path = path.with_extension(scribe::OLD_EXTENSION);
        let Some(bad) = [path, &old_path].into_iter().find(|p| p.exists()) else {
            return Loaded::Missing;
        };
        // 🪦 here lies a file nobody could read: rest in .corrupt
        let _ = fs::rename(bad, scribe::suffixed(bad, ".corrupt"));
        let backup = scribe::suffixed(path, ".bak");
        match Self::open(&backup) {
            Some(data) => {
                // 🧟 rise from your .bak
                let _ = fs::copy(&backup, path);
                Loaded::Restored(data)
            }
            None => Loaded::Lost,
        }
    }

    /// Writes the data as the current edition.
    fn save(&self, path: &Path) -> io::Result<()> {
        let sealed = Sealed {
            version: Self::current(),
            data: self,
        };
        scribe::inscribe(
            path,
            &rmp_serde::to_vec_named(&sealed).map_err(io::Error::other)?,
        )
    }

    /// Rewrites an outdated file as the current edition, returning whether it did.
    ///
    /// `self` must be the file's contents already upgraded (what [`Edition::open`] returns).
    /// The original is never deleted here: it is kept as `.bak` or in place, and the new file is
    /// read back and compared to `self` first. If it doesn't match, everything is put back as it
    /// was and an error is returned.
    fn upgrade(&self, path: &Path) -> Result<bool> {
        if Self::stored_version(path).is_none_or(|version| version >= Self::current()) {
            return Ok(false);
        }
        let existed = path.exists();
        self.save(path)?;
        if Self::stored_version(path) == Some(Self::current())
            && Self::open(path).is_some_and(|reloaded| reloaded == *self)
        {
            return Ok(true);
        }
        // 🕯️ the new page didn't dry right: put the old one back
        if existed {
            fs::copy(scribe::suffixed(path, ".bak"), path)?;
        } else {
            fs::remove_file(path)?;
        }
        Err(Error::Upgrade(
            "The upgraded file did not read back the same",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::scribe::tests::Altar;

    #[derive(Debug, Default, PartialEq, Deserialize, Serialize)]
    struct Note {
        text: String,
        #[serde(default)]
        loud: bool,
    }

    fn shout(mut note: Note) -> Note {
        note.text += "!";
        note
    }

    fn whisper(mut note: Note) -> Note {
        note.text += "?";
        note
    }

    impl Edition for Note {
        const UPGRADES: &[fn(Self) -> Self] = &[shout, whisper];
    }

    /// Claims to never equal itself, so a verification can never pass.
    #[derive(Deserialize, Serialize)]
    struct Liar(String);

    impl PartialEq for Liar {
        fn eq(&self, _: &Self) -> bool {
            false
        }
    }

    impl Edition for Liar {
        const UPGRADES: &[fn(Self) -> Self] = &[|liar| liar];
    }

    fn note(text: &str) -> Note {
        Note {
            text: text.into(),
            loud: false,
        }
    }

    /// How files were saved before editions: a bare positional array.
    fn legacy(note: &Note) -> Vec<u8> {
        rmp_serde::to_vec(note).unwrap()
    }

    #[test]
    fn current_is_the_number_of_upgrades() {
        assert_eq!(Note::current(), 2);
    }

    #[test]
    fn a_saved_file_reads_back_as_the_current_edition() {
        let altar = Altar::new("edition-roundtrip");
        let path = altar.file("a.rite");
        note("hi").save(&path).unwrap();
        assert_eq!(Note::stored_version(&path), Some(2));
        assert_eq!(Note::open(&path), Some(note("hi")));
    }

    #[test]
    fn a_legacy_file_is_edition_zero_and_runs_every_upgrade() {
        let altar = Altar::new("edition-legacy");
        fs::write(altar.file("a.doom"), legacy(&note("hi"))).unwrap();
        let path = altar.file("a.rite");
        assert_eq!(Note::stored_version(&path), Some(0));
        assert_eq!(Note::open(&path), Some(note("hi!?")));
    }

    #[test]
    fn a_file_from_the_middle_only_runs_the_upgrades_after_it() {
        let altar = Altar::new("edition-middle");
        let path = altar.file("a.rite");
        let sealed = Sealed {
            version: 1,
            data: note("hi"),
        };
        fs::write(&path, rmp_serde::to_vec_named(&sealed).unwrap()).unwrap();
        assert_eq!(Note::open(&path), Some(note("hi?")));
    }

    #[test]
    fn a_newer_file_is_recognised_but_never_opened_or_upgraded() {
        let altar = Altar::new("edition-newer");
        let path = altar.file("a.rite");
        let sealed = Sealed {
            version: 99,
            data: note("from the future"),
        };
        let bytes = rmp_serde::to_vec_named(&sealed).unwrap();
        fs::write(&path, &bytes).unwrap();
        assert_eq!(Note::stored_version(&path), Some(99));
        assert_eq!(Note::open(&path), None);
        assert!(!note("x").upgrade(&path).unwrap());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }

    #[test]
    fn a_newer_file_the_current_struct_cannot_hold_is_still_recognised() {
        let altar = Altar::new("edition-newer-shape");
        let path = altar.file("a.rite");
        #[derive(Serialize)]
        struct Future {
            version: u32,
            data: Vec<u8>,
        }
        let bytes = rmp_serde::to_vec_named(&Future {
            version: 7,
            data: vec![1, 2, 3],
        })
        .unwrap();
        fs::write(&path, bytes).unwrap();
        assert_eq!(Note::stored_version(&path), Some(7));
        assert_eq!(Note::open(&path), None);
    }

    /// A file that is a valid wrapper at the current edition, but whose data is not a `Note`.
    fn gibberish() -> Vec<u8> {
        #[derive(Serialize)]
        struct Wrong {
            version: u32,
            data: Vec<u8>,
        }
        let version = Note::current();
        rmp_serde::to_vec_named(&Wrong {
            version,
            data: vec![1, 2, 3],
        })
        .unwrap()
    }

    #[test]
    fn loading_a_good_file_is_fine_and_touches_nothing() {
        let altar = Altar::new("load-fine");
        let path = altar.file("a.rite");
        note("hi").save(&path).unwrap();
        assert_eq!(Note::load(&path), Loaded::Fine(note("hi")));
        assert_eq!(fs::read_dir(&altar.0).unwrap().count(), 1);
    }

    #[test]
    fn loading_an_old_file_upgrades_it_in_memory_only() {
        let altar = Altar::new("load-old");
        let original = legacy(&note("hi"));
        fs::write(altar.file("a.doom"), &original).unwrap();
        assert_eq!(
            Note::load(&altar.file("a.rite")),
            Loaded::Fine(note("hi!?"))
        );
        assert_eq!(fs::read(altar.file("a.doom")).unwrap(), original);
    }

    #[test]
    fn loading_nothing_is_missing_and_creates_nothing() {
        let altar = Altar::new("load-missing");
        assert_eq!(Note::load(&altar.file("a.rite")), Loaded::Missing);
        assert_eq!(fs::read_dir(&altar.0).unwrap().count(), 0);
    }

    #[test]
    fn loading_a_newer_file_leaves_it_alone() {
        let altar = Altar::new("load-newer");
        let path = altar.file("a.rite");
        let sealed = Sealed {
            version: 99,
            data: note("from the future"),
        };
        let bytes = rmp_serde::to_vec_named(&sealed).unwrap();
        fs::write(&path, &bytes).unwrap();
        assert_eq!(Note::load(&path), Loaded::TooNew);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert!(!altar.file("a.rite.corrupt").exists());
    }

    #[test]
    fn a_corrupt_file_is_moved_aside_and_the_backup_takes_its_place() {
        let altar = Altar::new("load-restore");
        let path = altar.file("a.rite");
        note("good").save(&path).unwrap();
        note("newer").save(&path).unwrap(); // "good" is now the backup
        fs::write(&path, b"this is not msgpack").unwrap();
        assert_eq!(Note::load(&path), Loaded::Restored(note("good")));
        assert_eq!(
            fs::read(altar.file("a.rite.corrupt")).unwrap(),
            b"this is not msgpack"
        );
        // the restored file is a real, loadable file again
        assert_eq!(Note::load(&path), Loaded::Fine(note("good")));
    }

    #[test]
    fn a_corrupt_file_with_no_backup_is_lost_but_kept() {
        let altar = Altar::new("load-lost");
        let path = altar.file("a.rite");
        fs::write(&path, gibberish()).unwrap();
        assert_eq!(Note::load(&path), Loaded::Lost);
        assert_eq!(fs::read(altar.file("a.rite.corrupt")).unwrap(), gibberish());
        assert!(!path.exists());
        // nothing is left to overwrite, and the next load is a clean first run
        assert_eq!(Note::load(&path), Loaded::Missing);
    }

    #[test]
    fn a_corrupt_backup_is_not_trusted() {
        let altar = Altar::new("load-bad-backup");
        let path = altar.file("a.rite");
        fs::write(&path, b"garbage").unwrap();
        fs::write(altar.file("a.rite.bak"), b"also garbage").unwrap();
        assert_eq!(Note::load(&path), Loaded::Lost);
        assert_eq!(fs::read(altar.file("a.rite.corrupt")).unwrap(), b"garbage");
    }

    #[test]
    fn a_corrupt_old_extension_file_is_moved_aside_too() {
        let altar = Altar::new("load-old-corrupt");
        fs::write(altar.file("a.doom"), b"garbage").unwrap();
        assert_eq!(Note::load(&altar.file("a.rite")), Loaded::Lost);
        assert_eq!(fs::read(altar.file("a.doom.corrupt")).unwrap(), b"garbage");
    }

    #[test]
    fn upgrading_writes_the_new_file_and_keeps_the_original() {
        let altar = Altar::new("edition-upgrade");
        let original = legacy(&note("hi"));
        fs::write(altar.file("a.doom"), &original).unwrap();
        let path = altar.file("a.rite");
        let upgraded = Note::open(&path).unwrap();
        assert!(upgraded.upgrade(&path).unwrap());
        assert_eq!(Note::stored_version(&path), Some(2));
        assert_eq!(Note::open(&path), Some(note("hi!?")));
        assert_eq!(fs::read(altar.file("a.doom")).unwrap(), original);
        assert_eq!(fs::read(altar.file("a.rite.bak")).unwrap(), original);
        // already current: nothing to do
        assert!(!upgraded.upgrade(&path).unwrap());
    }

    #[test]
    fn upgrading_a_rite_array_keeps_it_as_the_backup() {
        let altar = Altar::new("edition-upgrade-rite");
        let path = altar.file("a.rite");
        let original = legacy(&note("hi"));
        fs::write(&path, &original).unwrap();
        Note::open(&path).unwrap().upgrade(&path).unwrap();
        assert_eq!(Note::stored_version(&path), Some(2));
        assert_eq!(fs::read(altar.file("a.rite.bak")).unwrap(), original);
    }

    #[test]
    fn a_failed_verification_puts_back_the_legacy_original() {
        let altar = Altar::new("edition-liar-legacy");
        let original = rmp_serde::to_vec(&Liar("hi".into())).unwrap();
        fs::write(altar.file("a.doom"), &original).unwrap();
        let path = altar.file("a.rite");
        let liar = Liar::open(&path).unwrap();
        assert!(liar.upgrade(&path).is_err());
        assert!(!path.exists());
        assert_eq!(fs::read(altar.file("a.doom")).unwrap(), original);
        assert_eq!(Liar::stored_version(&path), Some(0));
    }

    #[test]
    fn a_failed_verification_puts_back_the_old_rite_file() {
        let altar = Altar::new("edition-liar-rite");
        let path = altar.file("a.rite");
        let original = rmp_serde::to_vec(&Liar("hi".into())).unwrap();
        fs::write(&path, &original).unwrap();
        let liar = Liar::open(&path).unwrap();
        assert!(liar.upgrade(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
    }
}
