use std::path::Path;

use ratatui::{crossterm::event::KeyCode, style::Color};

use crate::{
    error::Result,
    hosting, legend,
    profile::Profile,
    stage,
    utility::{self, Phase, Terminal},
};

const TITLE: &'static str = "Host";
const BORDER_COLOR: Color = Color::LightMagenta;
const WIDTH: u16 = 32;
const HEIGHT: u16 = 9;
const PLAYER_COUNT_LABEL: &'static str = "Player Count";
const MODE_LABEL: &'static str = "Mode";
const MAP_LABEL: &'static str = "Map";
const PLAYER_COUNT_LEGEND: &[[&'static str; 2]] = &[
    ["Enter", "Launch"],
    ["Esc", "Cancel"],
    ["Tab/Backtab", "Traverse"],
    ["2-8", "Input"],
];
const MODE_LEGEND: &[[&'static str; 2]] = &[
    ["Enter", "Launch"],
    ["Esc", "Cancel"],
    ["Tab/Backtab", "Traverse"],
    ["J/K", "Switch Mode"],
];
const MAP_LEGEND: &[[&'static str; 2]] = &[
    ["Enter", "Launch"],
    ["Esc", "Cancel"],
    ["Tab/Backtab", "Traverse"],
    ["A-Z/_", "Input"],
];

pub fn run(
    terminal: &mut Terminal,
    uzdoom: &str,
    profile: &Profile,
    data_subdir: &Path,
) -> Result<()> {
    let mut index = Index::PlayerCount;
    let mut player_count = String::new();
    let mut is_deathmatch = true;
    let mut map = String::new();
    stage![
        terminal,
        |frame| {
            let area = frame.area();
            let (window, mut window_area, indicator, indicator_area) =
                utility::new_window(WIDTH, HEIGHT, TITLE, BORDER_COLOR, area);
            frame.render_widget(window, window_area);
            frame.render_widget(&indicator, indicator_area);

            window_area.y += 2;
            let (player_count, player_count_position) =
                utility::new_query(PLAYER_COUNT_LABEL, &player_count, area.width, window_area.y);
            frame.render_widget(player_count, window_area);

            window_area.y += 2;
            let mode = if is_deathmatch {
                "Deathmatch"
            } else {
                "Cooperative"
            };
            let (mode, mode_position) =
                utility::new_query(MODE_LABEL, mode, area.width, window_area.y);
            frame.render_widget(mode, window_area);

            window_area.y += 2;
            let (map, map_position) =
                utility::new_query(MAP_LABEL, &map, area.width, window_area.y);
            frame.render_widget(map, window_area);

            let position = match index {
                Index::PlayerCount => player_count_position,
                Index::Mode => mode_position,
                Index::Map => map_position,
            };
            frame.set_cursor_position(position);
        },
        &mut |terminal, key_code| {
            match key_code {
                KeyCode::Enter => {
                    if player_count.is_empty() {
                        player_count = 2.to_string();
                    }
                    hosting::run(
                        terminal,
                        uzdoom,
                        profile,
                        &player_count.parse().unwrap_or(2).to_string(),
                        is_deathmatch,
                        &map,
                        data_subdir,
                    )?;
                    Ok(Phase::Render)
                }
                KeyCode::Esc => Ok(Phase::Return(())),
                KeyCode::Char('?') => {
                    legend::run(
                        terminal,
                        match index {
                            Index::PlayerCount => PLAYER_COUNT_LEGEND,
                            Index::Mode => MODE_LEGEND,
                            Index::Map => MAP_LEGEND,
                        },
                    )?;
                    Ok(Phase::Render)
                }
                KeyCode::Tab => {
                    index.select_next();
                    Ok(Phase::Render)
                }
                KeyCode::BackTab => {
                    index.select_previous();
                    Ok(Phase::Render)
                }
                KeyCode::Char(c @ '2'..='8')
                    if index == Index::PlayerCount && player_count.is_empty() =>
                {
                    player_count.push(c);
                    Ok(Phase::Render)
                }
                KeyCode::Backspace if index == Index::PlayerCount => {
                    player_count.pop();
                    Ok(Phase::Render)
                }
                KeyCode::Char('j' | 'k') if index == Index::Mode => {
                    is_deathmatch = !is_deathmatch;
                    Ok(Phase::Render)
                }
                KeyCode::Char(c @ 'a'..='z' | c @ 'A'..='Z' | c @ '0'..='9')
                    if index == Index::Map =>
                {
                    map.push(c);
                    Ok(Phase::Render)
                }
                KeyCode::Backspace if index == Index::Map => {
                    map.pop();
                    Ok(Phase::Render)
                }
                _ => Ok(Phase::Prompt),
            }
        }
    ];
}

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
