//! What `Y` copied or `X` cut, for `P` to paste: wads from the editor, whole profiles from the
//! launcher, or wad files from the wads screen. It lasts for the session and is never saved.
//! Copying one kind replaces the other.

use std::path::{Path, PathBuf};

use crate::loadout::{profile::Profile, wad::Wad};

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Clipboard {
    #[default]
    Empty,
    Wads(Vec<Wad>),
    Profiles(Vec<Profile>),
    /// Profiles cut in the launcher, as they were then. They stay where they are until pasted,
    /// which moves them there, once.
    CutProfiles(Vec<Profile>),
    /// Wad files from the wads screen, not yet hashed. Pasted into a dir there, they are copied,
    /// or if `cut`, moved there, once; pasted into a profile, they are hashed and added.
    Files {
        paths: Vec<PathBuf>,
        cut: bool,
    },
}

impl Clipboard {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Empty => true,
            Self::Wads(wads) => wads.is_empty(),
            Self::Profiles(profiles) | Self::CutProfiles(profiles) => profiles.is_empty(),
            Self::Files { paths, .. } => paths.is_empty(),
        }
    }

    /// Every wad on the clipboard: the wads themselves, or each profile's in turn, for pasting
    /// into a wad list. Files are left out, since they have to be hashed first.
    pub fn wads(&self) -> Vec<Wad> {
        match self {
            Self::Empty | Self::Files { .. } => Vec::new(),
            Self::Wads(wads) => wads.clone(),
            Self::Profiles(profiles) | Self::CutProfiles(profiles) => {
                profiles.iter().flat_map(|p| p.wads.clone()).collect()
            }
        }
    }

    /// Points whatever is on the clipboard at `to` where it was at `from`, after the file was
    /// renamed, so pasting it later doesn't paste a wad that is gone.
    pub fn follow(&mut self, from: &Path, to: &Path) {
        match self {
            Self::Empty => {}
            Self::Wads(wads) => {
                for wad in wads {
                    wad.follow(from, to);
                }
            }
            Self::Profiles(profiles) | Self::CutProfiles(profiles) => {
                for profile in profiles {
                    profile.follow(from, to);
                }
            }
            Self::Files { paths, .. } => {
                for path in paths.iter_mut().filter(|path| *path == from) {
                    *path = to.to_path_buf();
                }
            }
        }
    }

    /// What the note under the window says is copied, if anything is.
    pub fn note(&self) -> Option<String> {
        let (count, kind, done) = match self {
            Self::Empty => return None,
            Self::Wads(wads) => (wads.len(), "wad", "copied"),
            Self::Profiles(profiles) => (profiles.len(), "profile", "copied"),
            Self::CutProfiles(profiles) => (profiles.len(), "profile", "cut"),
            Self::Files { paths, cut } => (paths.len(), "wad", if *cut { "cut" } else { "copied" }),
        };
        match count {
            0 => None,
            1 => Some(format!("1 {kind} {done}")),
            n => Some(format!("{n} {kind}s {done}")),
        }
    }
}
