use ratatui::{crossterm::event::KeyCode, style::Color};

use crate::{
    error::Result,
    loadout::profile::{self, Profile},
    popups::{legend::Legend, three_way_dialog::ThreeWayDialog, warning::Warning},
    ritual::{Grimoire, Page, Phase, Ritual},
    theme,
    widgets::{
        form::{self, Field},
        text,
    },
};

/// Prompts for a name for a new profile (downloaded or imported), rejecting empty and taken names.
pub struct ProfileNamer<'a> {
    profile_name: String,
    /// Where the cursor is in the name.
    caret: usize,
    used_profiles: &'a [Profile],
}

impl<'a> ProfileNamer<'a> {
    const TITLE: &'static str = "name the profile";
    fn border_color() -> Color {
        theme::worn().colors.multiplayer
    }
    const LEGEND: &'static [[&'static str; 2]] = &[
        ["Enter", "Save Profile"],
        ["A-Z/0-9/_-", "Input (Spaces Too)"],
        ["←/→/Home/End", "Move the Cursor"],
        ["Esc", "Cancel"],
    ];
    /// The longest a name may be typed.
    const MAX_LENGTH: usize = profile::MAX_NAME;

    pub fn summon(grimoire: &mut Grimoire, used_profiles: &'a [Profile]) -> Result<Option<String>> {
        Self::new(used_profiles, "").invoke(grimoire)
    }

    /// Like [`ProfileNamer::summon`], with `suggested` typed in already (as far as it is a
    /// name tdl allows), so it can be taken with Enter or changed. A taken name is still refused.
    pub fn summon_suggesting(
        grimoire: &mut Grimoire,
        used_profiles: &'a [Profile],
        suggested: &str,
    ) -> Result<Option<String>> {
        Self::new(used_profiles, suggested).invoke(grimoire)
    }

    fn new(used_profiles: &'a [Profile], suggested: &str) -> Self {
        let profile_name: String = suggested
            .chars()
            .filter(|c| profile::is_name_character(*c))
            .collect();
        let profile_name = profile_name.trim_matches([' ', '-']).to_string();
        Self {
            caret: text::end(&profile_name),
            profile_name,
            used_profiles,
        }
    }
}

impl Ritual for ProfileNamer<'_> {
    type Return = Option<String>;

    fn typing(&self) -> bool {
        true
    }

    fn render(&mut self, page: &mut Page) {
        let fields = [Field {
            label: "name",
            value: &self.profile_name,
            placeholder: "type a name",
            room: 16,
            caret: Some(self.caret),
        }];
        form::draw(page, Self::TITLE, Self::border_color(), &fields, 0);
    }

    fn prompt(
        &mut self,
        grimoire: &mut Grimoire,
        key_code: KeyCode,
    ) -> Result<Phase<Self::Return>> {
        match key_code {
            KeyCode::Enter => {
                let name = self.profile_name.trim_matches(' ');
                if name.is_empty() {
                    Warning::empty_profile_name(grimoire)?;
                    Ok(Phase::Render)
                } else if let Some(name) = profile::clean_name(name) {
                    if self.used_profiles.iter().any(|p| p.name == name) {
                        Warning::profile_already_exists(grimoire)?;
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Return(Some(name)))
                    }
                } else {
                    Warning::bad_profile_name(grimoire)?;
                    Ok(Phase::Render)
                }
            }
            KeyCode::Esc => Ok(Phase::Return(None)),
            KeyCode::Char('?') | KeyCode::F(1) => {
                Legend::summon(grimoire, Self::LEGEND)?;
                Ok(Phase::Render)
            }
            key_code => Ok(
                match text::edit(
                    &mut self.profile_name,
                    &mut self.caret,
                    key_code,
                    Self::MAX_LENGTH,
                    name_character,
                ) {
                    Some(changed) => Phase::render_if(changed),
                    None => Phase::Prompt,
                },
            ),
        }
    }
}

/// A character a profile name may hold: it is also a file and dir name, so only `A-Z a-z 0-9 _ -`
/// and spaces (see [`profile::clean_name`] for how a whole name is held to it).
pub fn name_character(c: char) -> Option<char> {
    profile::is_name_character(c).then_some(c)
}

/// Asks what to call a copy of the profile called `name`, which is taken: `tagged` (`brutal_2`), a
/// name the user types (starting from `tagged`), or none, to skip it.
pub fn ask_copy_name(
    grimoire: &mut Grimoire,
    profiles: &[Profile],
    name: &str,
    tagged: &str,
) -> Result<Option<String>> {
    let question = format!("'{name}' already exists. Add it as '{tagged}', or name it?");
    Ok(
        match ThreeWayDialog::summon(grimoire, &question, ["Tag", "Name", "Skip"])? {
            Some(0) => Some(tagged.to_string()),
            Some(1) => ProfileNamer::summon_suggesting(grimoire, profiles, tagged)?,
            _ => None,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Types `keys` into a namer that has `taken` names in use, until it returns, and says what
    /// it returned (`None` for a namer still waiting).
    fn typed(taken: &[&str], suggested: &str, keys: &[KeyCode]) -> Option<Option<String>> {
        let used: Vec<Profile> = taken
            .iter()
            .map(|name| Profile::new(name.to_string(), vec![]))
            .collect();
        let mut namer = ProfileNamer::new(&used, suggested);
        let mut grimoire = crate::testing::silent_grimoire();
        keys.iter()
            .find_map(|&key| match namer.prompt(&mut grimoire, key).unwrap() {
                Phase::Return(name) => Some(name),
                _ => None,
            })
    }

    fn chars(text: &str) -> Vec<KeyCode> {
        text.chars().map(KeyCode::Char).collect()
    }

    #[test]
    fn a_name_with_spaces_and_dashes_is_taken_and_the_spaces_around_it_are_not() {
        let mut keys = chars(" my profile-2  ");
        keys.push(KeyCode::Enter);
        assert_eq!(typed(&[], "", &keys), Some(Some("my profile-2".into())));
    }

    #[test]
    fn a_suggested_name_is_taken_with_enter_and_esc_is_none() {
        // the Name button of a Tag/Name/Skip dialog leads here with the tagged name typed in
        assert_eq!(
            typed(&["brutal doom"], "brutal doom_2", &[KeyCode::Enter]),
            Some(Some("brutal doom_2".into()))
        );
        assert_eq!(typed(&[], "brutal_2", &[KeyCode::Esc]), Some(None));
        // 🚪 and nothing the naming window is opened with closes it before a key is pressed
        assert_eq!(typed(&["a"], "a_2", &[]), None);
    }

    #[test]
    fn the_suggestion_loses_a_leading_dash_or_space() {
        assert_eq!(
            typed(&[], "-dash", &[KeyCode::Enter]),
            Some(Some("dash".into()))
        );
        assert_eq!(typed(&[], "- x", &[KeyCode::Enter]), Some(Some("x".into())));
    }

    #[test]
    fn snapshot_the_keyboard_over_the_profile_namer() {
        let mut namer = ProfileNamer::new(&[], "brutal_doom");
        insta::assert_snapshot!(crate::keyboard::over(|page| namer.render(page)));
    }
}
