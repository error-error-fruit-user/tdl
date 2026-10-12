use std::{
    ffi::{OsStr, OsString},
    ops::Deref,
    path::Path,
};

use serde::{Deserialize, Serialize};

use crate::{error::Result, hash};

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct Wad {
    pub path: OsString,
    pub hash: String,
}

impl Wad {
    pub const EXTENSIONS: &[&str] = &["wad", "WAD", "pk3"];

    pub fn new(path: OsString, hash: String) -> Self {
        Self { path, hash }
    }

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
