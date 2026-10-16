use ratatui::{crossterm::event::KeyCode, style::Color};

use crate::{
    error::Result,
    net,
    popups::{dialog::Dialog, legend::Legend, warning::Warning},
    ritual::{Grimoire, Page, Phase, Ritual},
    theme,
    vault::{Vault, host_settings::HostSettings},
    widgets::form::{self, Button, Field},
};

/// Collects the host settings (player count, mode, map, port, join code) in a single form, with
/// buttons to save them as the profile's defaults or to clear those.
pub struct HostSettingsForm {
    settings: HostSettings,
    index: Index,
    /// Whether the profile has defaults saved, so the clear button knows if it can do anything.
    has_defaults: bool,
}

/// What the form was left with.
#[derive(Debug, PartialEq)]
pub enum Choice {
    Launch(HostSettings),
    SaveDefaults,
    ClearDefaults,
    Cancel,
}

impl HostSettingsForm {
    const TITLE: &str = "host";
    const BORDER_COLOR: Color = theme::MULTIPLAYER;
    const PLAYER_COUNT_LABEL: &str = "players";
    const MODE_LABEL: &str = "mode";
    const MAP_LABEL: &str = "map";
    const PORT_LABEL: &str = "port";
    const CODE_LABEL: &str = "code";
    const PLAYER_COUNT_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Launch"],
        ["Tab/Backtab", "Traverse"],
        ["2-8", "Input"],
        ["Esc", "Cancel"],
    ];
    const MODE_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Launch"],
        ["Tab/Backtab", "Traverse"],
        ["J/K", "Switch Mode"],
        ["Esc", "Cancel"],
    ];
    const MAP_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Launch"],
        ["Tab/Backtab", "Traverse"],
        ["A-Z/0-9/_", "Input"],
        ["Esc", "Cancel"],
    ];
    const PORT_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Launch"],
        ["Tab/Backtab", "Traverse"],
        ["0-9", "Input"],
        ["Esc", "Cancel"],
    ];
    const CODE_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Launch"],
        ["Tab/Backtab", "Traverse"],
        ["a-z/0-9", "Input"],
        ["Esc", "Cancel"],
    ];
    const BUTTON_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Press"],
        ["Tab/Backtab", "Traverse"],
        ["Esc", "Cancel"],
    ];

    /// Starts from the profile's saved `defaults` if it has them; otherwise from a fresh form whose
    /// port is `port`, the one used last time, if any.
    pub fn new(defaults: Option<HostSettings>, port: Option<u16>) -> Self {
        Self {
            has_defaults: defaults.is_some(),
            settings: defaults.unwrap_or_else(|| HostSettings::new(port)),
            index: Index::PlayerCount,
        }
    }

    /// Asks how to host the profile at `index`. Saving or clearing its defaults asks first, writes
    /// the profile straight away, and comes back to the form as it was. `None` if cancelled.
    pub fn summon(
        grimoire: &mut Grimoire,
        vault: &mut Vault,
        index: usize,
    ) -> Result<Option<HostSettings>> {
        let defaults = vault.profiles[index].host_defaults.clone();
        let mut form = Self::new(defaults, vault.config.host_port);
        loop {
            match form.invoke(grimoire)? {
                Choice::Launch(settings) => return Ok(Some(settings)),
                Choice::Cancel => return Ok(None),
                Choice::SaveDefaults => {
                    if Dialog::summon(grimoire, "Save these as this profile's host defaults?")?
                        && save_defaults(grimoire, vault, index, Some(form.settings.clone()))?
                    {
                        form.has_defaults = true;
                    }
                }
                Choice::ClearDefaults if !form.has_defaults => {
                    Warning::summon(grimoire, "This profile has no host defaults to clear")?;
                }
                Choice::ClearDefaults => {
                    if Dialog::summon(grimoire, "Clear this profile's host defaults?")?
                        && save_defaults(grimoire, vault, index, None)?
                    {
                        form.has_defaults = false;
                    }
                }
            }
        }
    }
}

/// Writes `defaults` into the profile at `index`. Says so and returns `false` if it couldn't be
/// saved, in which case the profile is left as it was.
fn save_defaults(
    grimoire: &mut Grimoire,
    vault: &mut Vault,
    index: usize,
    defaults: Option<HostSettings>,
) -> Result<bool> {
    let mut profile = vault.profiles[index].clone();
    profile.host_defaults = defaults;
    if vault.replace(index, profile).is_err() {
        Warning::save_failed(grimoire)?;
        return Ok(false);
    }
    Ok(true)
}

impl Ritual for HostSettingsForm {
    type Return = Choice;

    fn render(&mut self, page: &mut Page) {
        let mode = if self.settings.is_deathmatch {
            "deathmatch"
        } else {
            "cooperative"
        };
        let default_port = net::DEFAULT_PORT.to_string();
        let fields = [
            Field {
                label: Self::PLAYER_COUNT_LABEL,
                value: &self.settings.player_count,
                placeholder: "",
                room: 1,
            },
            Field {
                label: Self::MODE_LABEL,
                value: mode,
                placeholder: "",
                room: 11,
            },
            Field {
                label: Self::MAP_LABEL,
                value: &self.settings.map,
                placeholder: "any",
                room: 8,
            },
            Field {
                label: Self::PORT_LABEL,
                value: &self.settings.port,
                placeholder: &default_port,
                room: 5,
            },
            Field {
                label: Self::CODE_LABEL,
                value: &self.settings.code,
                placeholder: "random",
                room: net::MAX_CODE_LENGTH as u16,
            },
        ];
        let buttons = [
            Button {
                label: "save as default",
                enabled: true,
            },
            Button {
                label: "clear default",
                enabled: self.has_defaults,
            },
        ];
        let focus = match self.index {
            Index::PlayerCount => 0,
            Index::Mode => 1,
            Index::Map => 2,
            Index::Port => 3,
            Index::Code => 4,
            Index::Save => 5,
            Index::Clear => 6,
        };
        form::draw_with_buttons(
            page,
            Self::TITLE,
            Self::BORDER_COLOR,
            &fields,
            &buttons,
            focus,
        );
    }

    fn prompt(&mut self, grimoire: &mut Grimoire, key_code: KeyCode) -> Result<Phase<Choice>> {
        match key_code {
            KeyCode::Enter => Ok(Phase::Return(match self.index {
                Index::Save => Choice::SaveDefaults,
                Index::Clear => Choice::ClearDefaults,
                _ => Choice::Launch(self.settings.clone()),
            })),
            KeyCode::Tab => {
                self.index.select_next();
                Ok(Phase::Render)
            }
            KeyCode::BackTab => {
                self.index.select_previous();
                Ok(Phase::Render)
            }
            KeyCode::Char(c @ '2'..='8') if self.index == Index::PlayerCount => {
                self.settings.player_count.pop();
                self.settings.player_count.push(c);
                Ok(Phase::Render)
            }
            KeyCode::Char('j' | 'k') if self.index == Index::Mode => {
                self.settings.is_deathmatch = !self.settings.is_deathmatch;
                Ok(Phase::Render)
            }
            KeyCode::Char(c @ 'a'..='z' | c @ 'A'..='Z' | c @ '0'..='9' | c @ '_')
                if self.index == Index::Map =>
            {
                self.settings.map.push(c);
                Ok(Phase::Render)
            }
            KeyCode::Backspace if self.index == Index::Map => {
                self.settings.map.pop();
                Ok(Phase::Render)
            }
            KeyCode::Char(c @ '0'..='9')
                if self.index == Index::Port && self.settings.port.len() < 5 =>
            {
                self.settings.port.push(c);
                Ok(Phase::Render)
            }
            KeyCode::Backspace if self.index == Index::Port => {
                self.settings.port.pop();
                Ok(Phase::Render)
            }
            KeyCode::Char(c @ ('a'..='z' | 'A'..='Z' | '0'..='9'))
                if self.index == Index::Code && self.settings.code.len() < net::MAX_CODE_LENGTH =>
            {
                self.settings.code.push(c.to_ascii_lowercase());
                Ok(Phase::Render)
            }
            KeyCode::Backspace if self.index == Index::Code => {
                self.settings.code.pop();
                Ok(Phase::Render)
            }
            KeyCode::Esc => Ok(Phase::Return(Choice::Cancel)),
            KeyCode::Char('?') => {
                Legend::summon(
                    grimoire,
                    match self.index {
                        Index::PlayerCount => Self::PLAYER_COUNT_LEGEND,
                        Index::Mode => Self::MODE_LEGEND,
                        Index::Map => Self::MAP_LEGEND,
                        Index::Port => Self::PORT_LEGEND,
                        Index::Code => Self::CODE_LEGEND,
                        Index::Save | Index::Clear => Self::BUTTON_LEGEND,
                    },
                )?;
                Ok(Phase::Render)
            }
            _ => Ok(Phase::Prompt),
        }
    }
}

/// Which field or button of the form has focus.
#[derive(Debug, Eq, PartialEq)]
enum Index {
    PlayerCount,
    Mode,
    Map,
    Port,
    Code,
    Save,
    Clear,
}

impl Index {
    fn select_next(&mut self) {
        *self = match self {
            Self::PlayerCount => Self::Mode,
            Self::Mode => Self::Map,
            Self::Map => Self::Port,
            Self::Port => Self::Code,
            Self::Code => Self::Save,
            Self::Save => Self::Clear,
            Self::Clear => Self::PlayerCount,
        };
    }

    fn select_previous(&mut self) {
        *self = match self {
            Self::PlayerCount => Self::Clear,
            Self::Mode => Self::PlayerCount,
            Self::Map => Self::Mode,
            Self::Port => Self::Map,
            Self::Code => Self::Port,
            Self::Save => Self::Code,
            Self::Clear => Self::Save,
        };
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;

    use super::*;
    use crate::testing;

    #[test]
    fn a_filled_in_form_with_the_map_in_focus() {
        let mut form = HostSettingsForm::new(None, None);
        form.settings.player_count = "4".into();
        form.settings.map = "MAP07".into();
        form.index = Index::Map;
        assert_snapshot!(testing::draw(60, 20, |page| form.render(page)));
    }

    #[test]
    fn the_save_button_in_focus_with_defaults_saved() {
        let mut form = HostSettingsForm::new(Some(HostSettings::new(None)), None);
        form.index = Index::Save;
        assert_snapshot!(testing::draw(60, 20, |page| form.render(page)));
    }

    #[test]
    fn a_profile_with_defaults_starts_from_them() {
        let defaults = HostSettings {
            map: "MAP07".into(),
            is_deathmatch: false,
            ..HostSettings::new(Some(9000))
        };
        // the remembered port is only for profiles without defaults of their own
        let form = HostSettingsForm::new(Some(defaults.clone()), Some(7000));
        assert_eq!(form.settings, defaults);
        assert!(form.has_defaults);
        let form = HostSettingsForm::new(None, Some(7000));
        assert_eq!(form.settings.port, "7000");
        assert!(!form.has_defaults);
    }

    #[test]
    fn the_buttons_come_after_the_fields_and_wrap_around() {
        let mut index = Index::Code;
        index.select_next();
        assert_eq!(index, Index::Save);
        index.select_next();
        assert_eq!(index, Index::Clear);
        index.select_next();
        assert_eq!(index, Index::PlayerCount);
        index.select_previous();
        assert_eq!(index, Index::Clear);
    }
}
