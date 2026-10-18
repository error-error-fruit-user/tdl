//! What `Y` copied or `X` cut, for `P` to paste: wads from the editor, whole profiles from the
//! launcher, or wad files from the wads screen. It lasts for the session and is never saved.
//! Copying one kind replaces the other.

use std::path::{Path, PathBuf};

use crate::vault::{profile::Profile, wad::Wad};

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

#[cfg(test)]
mod tests {
    use super::*;

    fn wad(name: &str) -> Wad {
        Wad::new(format!("/wads/{name}").into(), String::new(), 1)
    }

    #[test]
    fn a_renamed_file_is_followed_wherever_it_is_on_the_clipboard() {
        let (from, to) = (Path::new("/wads/a.wad"), Path::new("/wads/b.wad"));
        let mut wads = Clipboard::Wads(vec![wad("a.wad"), wad("c.wad")]);
        wads.follow(from, to);
        assert_eq!(wads, Clipboard::Wads(vec![wad("b.wad"), wad("c.wad")]));
        let mut files = Clipboard::Files {
            paths: vec![from.to_path_buf()],
            cut: true,
        };
        files.follow(from, to);
        assert_eq!(
            files,
            Clipboard::Files {
                paths: vec![to.to_path_buf()],
                cut: true,
            }
        );
    }

    #[test]
    fn the_note_says_how_many_of_what_are_copied() {
        assert_eq!(Clipboard::Empty.note(), None);
        assert_eq!(Clipboard::Wads(Vec::new()).note(), None);
        assert_eq!(
            Clipboard::Wads(vec![wad("a.wad")]).note().as_deref(),
            Some("1 wad copied")
        );
        let profiles = vec![Profile::default(), Profile::default()];
        assert_eq!(
            Clipboard::Profiles(profiles.clone()).note().as_deref(),
            Some("2 profiles copied")
        );
        assert_eq!(
            Clipboard::CutProfiles(profiles).note().as_deref(),
            Some("2 profiles cut")
        );
    }

    #[test]
    fn copied_or_cut_files_say_so_and_are_not_wads_until_hashed() {
        let paths = vec![PathBuf::from("/wads/a.wad"), "/wads/b.wad".into()];
        let copied = Clipboard::Files {
            paths: paths.clone(),
            cut: false,
        };
        assert_eq!(copied.note().as_deref(), Some("2 wads copied"));
        assert!(copied.wads().is_empty());
        let cut = Clipboard::Files { paths, cut: true };
        assert_eq!(cut.note().as_deref(), Some("2 wads cut"));
        assert!(!cut.is_empty());
    }

    #[test]
    fn copied_profiles_paste_into_a_wad_list_as_all_their_wads_in_order() {
        let profiles = vec![
            Profile::new("a".into(), vec![wad("1.wad"), wad("2.wad")]),
            Profile::new("b".into(), vec![wad("3.wad")]),
        ];
        let names: Vec<_> = Clipboard::Profiles(profiles)
            .wads()
            .iter()
            .map(|w| w.file_name().into_owned())
            .collect();
        assert_eq!(names, ["1.wad", "2.wad", "3.wad"]);
        assert!(Clipboard::Empty.wads().is_empty());
        assert!(Clipboard::Profiles(Vec::new()).is_empty());
    }
}
