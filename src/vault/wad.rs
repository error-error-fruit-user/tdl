use std::{
    ffi::{OsStr, OsString},
    ops::Deref,
    path::Path,
};

use serde::{Deserialize, Serialize};

use crate::{error::Result, vault::hash};

/// A wad or pk3 file, identified across machines by the BLAKE3 hash of its contents.
///
/// `Deref`s to its path so it can be passed straight to a command line.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct Wad {
    pub path: OsString,
    pub hash: String,
}

impl Wad {
    /// File extensions the pickers accept.
    pub const EXTENSIONS: &[&str] = &["wad", "WAD", "pk3"];

    pub fn new(path: OsString, hash: String) -> Self {
        Self { path, hash }
    }

    /// Hashes the file at `path`, which can be slow for large wads.
    pub fn from_path(path: &Path) -> Result<Self> {
        Ok(Self::new(path.as_os_str().into(), hash::hash(path)?))
    }
}

impl Deref for Wad {
    type Target = OsStr;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}
