use ratatui::{crossterm::event::KeyCode, style::Color};

use crate::{
    error::Result,
    loadout::profile::Profile,
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
    const TITLE: &str = "name the profile";
    fn border_color() -> Color {
        theme::worn().colors.multiplayer
    }
    const LEGEND: &[[&str; 2]] = &[
        ["Enter", "Save Profile"],
        ["A-Z/0-9/_", "Input"],
        ["←/→/Home/End", "Move the Cursor"],
        ["Esc", "Cancel"],
    ];
    /// The longest a name may be typed.
    const MAX_LENGTH: usize = 64;

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
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        Self {
            caret: text::end(&profile_name),
            profile_name,
            used_profiles,
        }
    }
}

impl Ritual for ProfileNamer<'_> {
    type Return = Option<String>;

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
                if self.profile_name.is_empty() {
                    Warning::empty_profile_name(grimoire)?;
                    Ok(Phase::Render)
                } else if self
                    .used_profiles
                    .iter()
                    .any(|p| p.name == self.profile_name)
                {
                    Warning::profile_already_exists(grimoire)?;
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Return(Some(self.profile_name.clone())))
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

/// A character a profile name may hold: it is also a file and dir name, so only `A-Z a-z 0-9 _`.
pub fn name_character(c: char) -> Option<char> {
    (c.is_ascii_alphanumeric() || c == '_').then_some(c)
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
