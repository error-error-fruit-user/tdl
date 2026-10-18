use ratatui::{crossterm::event::KeyCode, style::Color};

use crate::{
    error::Result,
    net,
    popups::{dialog::Dialog, legend::Legend, warning::Warning},
    ritual::{Grimoire, Page, Phase, Ritual},
    theme,
    vault::{Vault, host_settings::HostSettings},
    widgets::{
        form::{self, Button, Field},
        text,
    },
};

/// Collects the host settings (player count, mode, map, port, join code, whether the ini is
/// shared) in a single form, with
/// buttons to save them as the profile's defaults or to clear those.
pub struct HostSettingsForm {
    settings: HostSettings,
    index: Index,
    /// Whether the profile has defaults saved, so the clear button knows if it can do anything.
    has_defaults: bool,
    /// Where the cursor is in the focused field, when it is one that is typed into.
    caret: usize,
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
    fn border_color() -> Color {
        theme::worn().colors.multiplayer
    }
    const PLAYER_COUNT_LABEL: &str = "players";
    const MODE_LABEL: &str = "mode";
    const MAP_LABEL: &str = "map";
    const PORT_LABEL: &str = "port";
    const CODE_LABEL: &str = "code";
    const INI_LABEL: &str = "ini";
    /// The longest a map name may be typed, far longer than any real one.
    const MAX_MAP: usize = 32;
    const PLAYER_COUNT_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Launch"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["2-8, ←/→", "Input"],
        ["Esc", "Cancel"],
    ];
    const MODE_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Launch"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["J/K, ←/→", "Switch Mode"],
        ["Esc", "Cancel"],
    ];
    const MAP_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Launch"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["A-Z/0-9/_", "Input"],
        ["←/→/Home/End", "Move the Cursor"],
        ["Esc", "Cancel"],
    ];
    const PORT_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Launch"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["0-9", "Input"],
        ["←/→/Home/End", "Move the Cursor"],
        ["Esc", "Cancel"],
    ];
    const CODE_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Launch"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["a-z/0-9", "Input"],
        ["←/→/Home/End", "Move the Cursor"],
        ["Esc", "Cancel"],
    ];
    const INI_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Launch"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["J/K, ←/→", "Share It or Not"],
        ["Esc", "Cancel"],
    ];
    const BUTTON_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Press"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["Esc", "Cancel"],
    ];

    /// Starts from the profile's saved `defaults` if it has them; otherwise from a fresh form whose
    /// port is `port`, the one used last time, if any.
    pub fn new(defaults: Option<HostSettings>, port: Option<u16>) -> Self {
        Self {
            has_defaults: defaults.is_some(),
            settings: defaults.unwrap_or_else(|| HostSettings::new(port)),
            index: Index::PlayerCount,
            caret: 0,
        }
    }

    /// Asks how to host the profile at `index`, starting from `start` if given (as `tdl host` does
    /// with its flags), else from the profile's defaults. Saving or clearing its defaults asks
    /// first, writes the profile straight away, and comes back to the form as it was. `None` if
    /// cancelled.
    pub fn summon(
        grimoire: &mut Grimoire,
        vault: &mut Vault,
        index: usize,
        start: Option<HostSettings>,
    ) -> Result<Option<HostSettings>> {
        let defaults = vault.profiles[index].host_defaults.clone();
        let mut form = Self::new(defaults, vault.global.host_port);
        if let Some(start) = start {
            form.settings = start;
        }
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
        let ini = if self.settings.share_ini {
            "shared"
        } else {
            "not shared"
        };
        let default_port = net::DEFAULT_PORT.to_string();
        let fields = [
            Field {
                label: Self::PLAYER_COUNT_LABEL,
                value: &self.settings.player_count,
                placeholder: "",
                room: 1,
                caret: None,
            },
            Field {
                label: Self::MODE_LABEL,
                value: mode,
                placeholder: "",
                room: 11,
                caret: None,
            },
            Field {
                label: Self::MAP_LABEL,
                value: &self.settings.map,
                placeholder: "any",
                room: 8,
                caret: (self.index == Index::Map).then_some(self.caret),
            },
            Field {
                label: Self::PORT_LABEL,
                value: &self.settings.port,
                placeholder: &default_port,
                room: 5,
                caret: (self.index == Index::Port).then_some(self.caret),
            },
            Field {
                label: Self::CODE_LABEL,
                value: &self.settings.code,
                placeholder: "random",
                room: net::MAX_CODE_LENGTH as u16,
                caret: (self.index == Index::Code).then_some(self.caret),
            },
            Field {
                label: Self::INI_LABEL,
                value: ini,
                placeholder: "",
                room: 10,
                caret: None,
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
            Index::Ini => 5,
            Index::Save => 6,
            Index::Clear => 7,
        };
        form::draw_with_buttons(
            page,
            Self::TITLE,
            Self::border_color(),
            &fields,
            &buttons,
            focus,
        );
    }

    fn prompt(&mut self, grimoire: &mut Grimoire, key_code: KeyCode) -> Result<Phase<Choice>> {
        match key_code {
            KeyCode::Char('?') | KeyCode::F(1) => {
                Legend::summon(
                    grimoire,
                    match self.index {
                        Index::PlayerCount => Self::PLAYER_COUNT_LEGEND,
                        Index::Mode => Self::MODE_LEGEND,
                        Index::Map => Self::MAP_LEGEND,
                        Index::Port => Self::PORT_LEGEND,
                        Index::Code => Self::CODE_LEGEND,
                        Index::Ini => Self::INI_LEGEND,
                        Index::Save | Index::Clear => Self::BUTTON_LEGEND,
                    },
                )?;
                Ok(Phase::Render)
            }
            key_code => Ok(self.edit(key_code)),
        }
    }
}

impl HostSettingsForm {
    /// The fewest and most players UZDoom's `-host` is given here.
    const PLAYERS: std::ops::RangeInclusive<u8> = 2..=8;

    /// Moves to the next field or button, or with `back` the one before, with the cursor at the
    /// end of its text.
    fn move_focus(&mut self, back: bool) -> Phase<Choice> {
        if back {
            self.index.select_previous();
        } else {
            self.index.select_next();
        }
        self.caret = self.focused_text().map_or(0, |text| text::end(text));
        Phase::Render
    }

    /// The text of the field in focus, if it is one that is typed into.
    fn focused_text(&mut self) -> Option<&mut String> {
        match self.index {
            Index::Map => Some(&mut self.settings.map),
            Index::Port => Some(&mut self.settings.port),
            Index::Code => Some(&mut self.settings.code),
            _ => None,
        }
    }

    /// Steps the player count up or down by one, staying within [`Self::PLAYERS`].
    fn step_players(&mut self, up: bool) -> Phase<Choice> {
        let count: u8 = self
            .settings
            .player_count
            .parse()
            .unwrap_or(*Self::PLAYERS.start());
        let stepped = if up {
            count.saturating_add(1)
        } else {
            count.saturating_sub(1)
        };
        let stepped = stepped.clamp(*Self::PLAYERS.start(), *Self::PLAYERS.end());
        Phase::render_if_then(stepped != count, || {
            self.settings.player_count = stepped.to_string();
        })
    }

    /// Every key but the legend's, none of which needs the terminal.
    fn edit(&mut self, key_code: KeyCode) -> Phase<Choice> {
        match (key_code, &self.index) {
            (KeyCode::Enter, Index::Save) => Phase::Return(Choice::SaveDefaults),
            (KeyCode::Enter, Index::Clear) => Phase::Return(Choice::ClearDefaults),
            (KeyCode::Enter, _) => Phase::Return(Choice::Launch(self.settings.clone())),
            (KeyCode::Esc, _) => Phase::Return(Choice::Cancel),
            (KeyCode::Tab | KeyCode::Down, _) => self.move_focus(false),
            (KeyCode::BackTab | KeyCode::Up, _) => self.move_focus(true),
            (KeyCode::Char(c @ '2'..='8'), Index::PlayerCount) => {
                self.settings.player_count = c.to_string();
                Phase::Render
            }
            (KeyCode::Right, Index::PlayerCount) => self.step_players(true),
            (KeyCode::Left, Index::PlayerCount) => self.step_players(false),
            (KeyCode::Char('j' | 'k') | KeyCode::Left | KeyCode::Right, Index::Mode) => {
                self.settings.is_deathmatch = !self.settings.is_deathmatch;
                Phase::Render
            }
            (KeyCode::Char('j' | 'k') | KeyCode::Left | KeyCode::Right, Index::Ini) => {
                self.settings.share_ini = !self.settings.share_ini;
                Phase::Render
            }
            // ↔️ between the two buttons
            (KeyCode::Right, Index::Save) => self.move_focus(false),
            (KeyCode::Left, Index::Clear) => self.move_focus(true),
            (key_code, Index::Map | Index::Port | Index::Code) => {
                let (max, accept): (usize, fn(char) -> Option<char>) = match self.index {
                    Index::Map => (Self::MAX_MAP, |c| {
                        (c.is_ascii_alphanumeric() || c == '_').then_some(c)
                    }),
                    Index::Port => (5, |c| c.is_ascii_digit().then_some(c)),
                    _ => (net::MAX_CODE_LENGTH, |c| {
                        c.is_ascii_alphanumeric().then_some(c.to_ascii_lowercase())
                    }),
                };
                let mut caret = self.caret;
                let text = self.focused_text().expect("a typed field has focus");
                let changed = text::edit(text, &mut caret, key_code, max, accept);
                self.caret = caret;
                match changed {
                    Some(changed) => Phase::render_if(changed),
                    None => Phase::Prompt,
                }
            }
            _ => Phase::Prompt,
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
    Ini,
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
            Self::Code => Self::Ini,
            Self::Ini => Self::Save,
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
            Self::Ini => Self::Code,
            Self::Save => Self::Ini,
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
    fn the_arrows_step_the_players_switch_the_mode_and_move_about_the_text() {
        let mut form = HostSettingsForm::new(None, None);
        assert!(form.edit(KeyCode::Right) == Phase::Render);
        assert_eq!(form.settings.player_count, "3");
        for _ in 0..10 {
            let _ = form.edit(KeyCode::Right);
        }
        assert_eq!(form.settings.player_count, "8", "no more than 8");
        assert!(form.edit(KeyCode::Right) == Phase::Prompt);

        assert!(form.edit(KeyCode::Down) == Phase::Render);
        assert_eq!(form.index, Index::Mode);
        let deathmatch = form.settings.is_deathmatch;
        let _ = form.edit(KeyCode::Left);
        assert_ne!(form.settings.is_deathmatch, deathmatch);

        let _ = form.edit(KeyCode::Down);
        for c in "MAP7".chars() {
            let _ = form.edit(KeyCode::Char(c));
        }
        let _ = form.edit(KeyCode::Left);
        let _ = form.edit(KeyCode::Char('0'));
        assert_eq!(form.settings.map, "MAP07");
        let _ = form.edit(KeyCode::Up);
        assert_eq!(form.index, Index::Mode);
    }

    #[test]
    fn the_ini_is_kept_to_yourself_until_switched_to_shared() {
        let mut form = HostSettingsForm::new(None, None);
        assert!(!form.settings.share_ini);
        form.index = Index::Ini;
        assert!(form.edit(KeyCode::Right) == Phase::Render);
        assert!(form.settings.share_ini);
        let _ = form.edit(KeyCode::Char('k'));
        assert!(!form.settings.share_ini);
    }

    #[test]
    fn left_and_right_move_between_the_buttons() {
        let mut form = HostSettingsForm::new(None, None);
        form.index = Index::Save;
        let _ = form.edit(KeyCode::Right);
        assert_eq!(form.index, Index::Clear);
        let _ = form.edit(KeyCode::Left);
        assert_eq!(form.index, Index::Save);
    }

    #[test]
    fn the_buttons_come_after_the_fields_and_wrap_around() {
        let mut index = Index::Code;
        index.select_next();
        assert_eq!(index, Index::Ini);
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
