//! What `Y` copied, for `P` to paste: wads from the editor, or whole profiles from the launcher.
//! It lasts for the session and is never saved. Copying one kind replaces the other.

use crate::vault::{profile::Profile, wad::Wad};

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Clipboard {
    #[default]
    Empty,
    Wads(Vec<Wad>),
    Profiles(Vec<Profile>),
}

impl Clipboard {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Empty => true,
            Self::Wads(wads) => wads.is_empty(),
            Self::Profiles(profiles) => profiles.is_empty(),
        }
    }

    /// Every wad on the clipboard: the wads themselves, or each profile's in turn, for pasting
    /// into a wad list.
    pub fn wads(&self) -> Vec<Wad> {
        match self {
            Self::Empty => Vec::new(),
            Self::Wads(wads) => wads.clone(),
            Self::Profiles(profiles) => profiles.iter().flat_map(|p| p.wads.clone()).collect(),
        }
    }

    /// What the note under the window says is copied, if anything is.
    pub fn note(&self) -> Option<String> {
        let (count, kind) = match self {
            Self::Empty => return None,
            Self::Wads(wads) => (wads.len(), "wad"),
            Self::Profiles(profiles) => (profiles.len(), "profile"),
        };
        match count {
            0 => None,
            1 => Some(format!("1 {kind} copied")),
            n => Some(format!("{n} {kind}s copied")),
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
    fn the_note_says_how_many_of_what_are_copied() {
        assert_eq!(Clipboard::Empty.note(), None);
        assert_eq!(Clipboard::Wads(Vec::new()).note(), None);
        assert_eq!(
            Clipboard::Wads(vec![wad("a.wad")]).note().as_deref(),
            Some("1 wad copied")
        );
        let profiles = vec![Profile::default(), Profile::default()];
        assert_eq!(
            Clipboard::Profiles(profiles).note().as_deref(),
            Some("2 profiles copied")
        );
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
