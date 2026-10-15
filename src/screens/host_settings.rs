use ratatui::{crossterm::event::KeyCode, style::Color};

use crate::{
    error::Result,
    popups::legend::Legend,
    ritual::{self, Grimoire, Page, Phase, Ritual},
};

/// Collects the host settings (player count, mode, map) in a single form.
pub struct HostSettingsForm {
    settings: HostSettings,
    index: Index,
}

impl HostSettingsForm {
    const TITLE: &str = "Host";
    const BORDER_COLOR: Color = Color::LightMagenta;
    const WIDTH: u16 = 32;
    const HEIGHT: u16 = 9;
    const PLAYER_COUNT_LABEL: &str = "Player Count";
    const MODE_LABEL: &str = "Mode";
    const MAP_LABEL: &str = "Map";
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

    pub fn new() -> Self {
        Self {
            settings: HostSettings::new(),
            index: Index::PlayerCount,
        }
    }

    /// `None` if cancelled.
    pub fn summon(grimoire: &mut Grimoire) -> Result<Option<HostSettings>> {
        Self::new().invoke(grimoire)
    }
}

impl Ritual for HostSettingsForm {
    type Return = Option<HostSettings>;

    fn render(&mut self, page: &mut Page) {
        let area = page.area();
        let (window, mut window_area, indicator, indicator_area) = ritual::conjure_window(
            Self::TITLE,
            Self::WIDTH,
            Self::HEIGHT,
            Self::BORDER_COLOR,
            area,
        );
        page.render_widget(window, window_area);
        page.render_widget(indicator, indicator_area);

        window_area.y += 2;
        let (player_count, player_count_position) = ritual::conjure_query(
            Self::PLAYER_COUNT_LABEL,
            &self.settings.player_count,
            area.width,
            window_area.y,
        );
        page.render_widget(player_count, window_area);

        window_area.y += 2;
        let mode = if self.settings.is_deathmatch {
            "Deathmatch"
        } else {
            "Cooperative"
        };
        let (mode, mode_position) =
            ritual::conjure_query(Self::MODE_LABEL, mode, area.width, window_area.y);
        page.render_widget(mode, window_area);

        window_area.y += 2;
        let (map, map_position) = ritual::conjure_query(
            Self::MAP_LABEL,
            &self.settings.map,
            area.width,
            window_area.y,
        );
        page.render_widget(map, window_area);

        let position = match self.index {
            Index::PlayerCount => player_count_position,
            Index::Mode => mode_position,
            Index::Map => map_position,
        };
        page.set_cursor_position(position);
    }

    fn prompt(
        &mut self,
        grimoire: &mut Grimoire,
        key_code: KeyCode,
    ) -> Result<Phase<Option<HostSettings>>> {
        match key_code {
            KeyCode::Enter => Ok(Phase::Return(Some(self.settings.clone()))),
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
            KeyCode::Esc => Ok(Phase::Return(None)),
            KeyCode::Char('?') => {
                Legend::summon(
                    grimoire,
                    match self.index {
                        Index::PlayerCount => Self::PLAYER_COUNT_LEGEND,
                        Index::Mode => Self::MODE_LEGEND,
                        Index::Map => Self::MAP_LEGEND,
                    },
                )?;
                Ok(Phase::Render)
            }
            _ => Ok(Phase::Prompt),
        }
    }
}

/// The options for a hosted game. `player_count` stays a string since it is passed straight to UZDoom.
#[derive(Clone)]
pub struct HostSettings {
    pub player_count: String,
    pub is_deathmatch: bool,
    pub map: String,
}

impl HostSettings {
    fn new() -> Self {
        Self {
            player_count: 2.to_string(),
            is_deathmatch: true,
            map: String::new(),
        }
    }
}

/// Which field of the form has focus.
#[derive(Eq, PartialEq)]
enum Index {
    PlayerCount,
    Mode,
    Map,
}

impl Index {
    fn select_next(&mut self) {
        *self = match self {
            Self::PlayerCount => Self::Mode,
            Self::Mode => Self::Map,
            Self::Map => Self::PlayerCount,
        };
    }

    fn select_previous(&mut self) {
        *self = match self {
            Self::PlayerCount => Self::Map,
            Self::Mode => Self::PlayerCount,
            Self::Map => Self::Mode,
        };
    }
}
