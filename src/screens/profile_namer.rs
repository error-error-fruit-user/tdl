use ratatui::{crossterm::event::KeyCode, style::Color};

use crate::{
    error::Result,
    popups::{legend::Legend, warning::Warning},
    ritual::{Grimoire, Page, Phase, Ritual},
    theme,
    vault::profile::Profile,
    widgets::form::{self, Field},
};

/// Prompts for a name for a new profile (downloaded or imported), rejecting empty and taken names.
pub struct ProfileNamer<'a> {
    profile_name: String,
    used_profiles: &'a [Profile],
}

impl<'a> ProfileNamer<'a> {
    const TITLE: &str = "name the profile";
    const BORDER_COLOR: Color = theme::MULTIPLAYER;
    const LEGEND: &[[&str; 2]] = &[
        ["Enter", "Save Profile"],
        ["A-Z/0-9/_", "Input"],
        ["Esc", "Cancel"],
    ];

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
        Self {
            profile_name: suggested
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect(),
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
        }];
        form::draw(page, Self::TITLE, Self::BORDER_COLOR, &fields, 0);
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
            KeyCode::Char(c @ 'a'..='z' | c @ 'A'..='Z' | c @ '0'..='9' | c @ '_') => {
                self.profile_name.push(c);
                Ok(Phase::Render)
            }
            KeyCode::Backspace => {
                self.profile_name.pop();
                Ok(Phase::Render)
            }
            KeyCode::Esc => Ok(Phase::Return(None)),
            KeyCode::Char('?') => {
                Legend::summon(grimoire, Self::LEGEND)?;
                Ok(Phase::Render)
            }
            _ => Ok(Phase::Prompt),
        }
    }
}
