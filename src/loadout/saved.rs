//! How paths and wads look on disk, as opposed to in memory.
//!
//! serde saves an `OsString` tagged with the OS that wrote it (`Unix` bytes or `Windows` wide
//! characters), and refuses to read the other kind back, so a file saved on one OS couldn't even
//! be opened on the other. Paths are saved as plain text instead, falling back to the OS's own form
//! only for the rare path that isn't valid UTF-8.

use std::ffi::{OsStr, OsString};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A path as it is saved: text whenever it can be, so any OS can read the file.
///
/// Untagged, so files from before this read too: their tagged `OsString`s aren't text, so they
/// land in [`SavedPath::Native`].
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SavedPath {
    Text(String),
    /// Not valid UTF-8, so saved the way only this OS can read back.
    Native(OsString),
}

impl From<&OsStr> for SavedPath {
    fn from(path: &OsStr) -> Self {
        match path.to_str() {
            Some(text) => Self::Text(text.to_string()),
            None => Self::Native(path.to_os_string()),
        }
    }
}

impl From<SavedPath> for OsString {
    fn from(path: SavedPath) -> Self {
        match path {
            SavedPath::Text(text) => text.into(),
            SavedPath::Native(native) => native,
        }
    }
}

/// For `#[serde(with)]` on an `Option<OsString>` field: saved as an optional [`SavedPath`].
pub mod optional_path {
    use super::*;

    pub fn serialize<S: Serializer>(
        path: &Option<OsString>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        path.as_deref().map(SavedPath::from).serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<OsString>, D::Error> {
        Ok(Option::<SavedPath>::deserialize(deserializer)?.map(OsString::from))
    }
}

/// For `#[serde(with)]` on a `Vec<OsString>` field: saved as a list of [`SavedPath`]s.
pub mod paths {
    use super::*;

    pub fn serialize<S: Serializer>(paths: &[OsString], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(paths.iter().map(|path| SavedPath::from(path.as_os_str())))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<OsString>, D::Error> {
        Ok(Vec::<SavedPath>::deserialize(deserializer)?
            .into_iter()
            .map(OsString::from)
            .collect())
    }
}

/// A wad saved on its own, with its whole path: the global base wad, and any wad in a profile that
/// can't be split into a dir and a text file name.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SavedWad {
    pub path: SavedPath,
    pub hash: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub volatile: bool,
    #[serde(default)]
    pub modified: u64,
    #[serde(default)]
    pub folder: bool,
}
