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
        Some(
            Self::UPGRADES[version as usize..]
                .iter()
                .fold(data?, |data, upgrade| upgrade(data)),
        )
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
