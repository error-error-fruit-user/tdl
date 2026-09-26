use std::{io::Stdout, path::Path};

use ratatui::{Terminal, crossterm::event, prelude::CrosstermBackend, style::Color};

use crate::{
    error::Result,
    execute, legend,
    profile::Profile,
    stage,
    window::{self, Phase},
};

const WIDTH: u16 = 32;
const HEIGHT: u16 = 9;
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
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    uzdoom: &Path,
    profile: &Profile,
) -> Result<()> {
    let mut input = Input::PlayerCount;
    let mut player_count = String::new();
    let mut is_deathmatch = false;
    let mut map = String::new();
    stage![
        terminal,
        |frame| {
            let (window, mut window_area, indicator, indicator_area) =
                window::new(WIDTH, HEIGHT, "Host", Color::LightMagenta, frame.area());
            frame.render_widget(window, window_area);
            frame.render_widget(&indicator, indicator_area);

            window_area.y += 2;
            let player_count = player_count.to_string();
            let line = window::new_input("Player Count: ", &player_count);
            frame.render_widget(line, window_area);

            window_area.y += 2;
            let mode = if is_deathmatch {
                "Deathmatch"
            } else {
                "Cooperative"
            };
            let line = window::new_input("Mode: ", mode);
            frame.render_widget(line, window_area);

            window_area.y += 2;
            let line = window::new_input("Map: ", &map);
            frame.render_widget(line, window_area);
        },
        &mut |terminal, key_code| {
            match key_code {
                event::KeyCode::Enter => {
                    execute::host(
                        uzdoom,
                        profile,
                        &player_count.parse().unwrap_or(2).to_string(),
                        is_deathmatch,
                        &map,
                    )?;
                    Ok(Phase::Prompt)
                }
                event::KeyCode::Esc => Ok(Phase::Return(())),
                event::KeyCode::Char('?') => {
                    legend::run(
                        terminal,
                        match input {
                            Input::PlayerCount => PLAYER_COUNT_LEGEND,
                            Input::MatchType => MODE_LEGEND,
                            Input::Map => MAP_LEGEND,
                        },
                    )?;
                    Ok(Phase::Render)
                }
                event::KeyCode::Tab => {
                    input.select_next();
                    Ok(Phase::Render)
                }
                event::KeyCode::BackTab => {
                    input.select_previous();
                    Ok(Phase::Render)
                }
                event::KeyCode::Char(c @ '2'..='8')
                    if input == Input::PlayerCount && player_count.is_empty() =>
                {
                    player_count.push(c);
                    Ok(Phase::Render)
                }
                event::KeyCode::Backspace if input == Input::PlayerCount => {
                    player_count.pop();
                    Ok(Phase::Render)
                }
                event::KeyCode::Char('j' | 'k') if input == Input::MatchType => {
                    is_deathmatch = !is_deathmatch;
                    Ok(Phase::Render)
                }
                event::KeyCode::Char(c @ 'a'..='z' | c @ 'A'..='Z' | c @ '0'..='9')
                    if input == Input::Map =>
                {
                    map.push(c);
                    Ok(Phase::Render)
                }
                event::KeyCode::Backspace if input == Input::Map => {
                    map.pop();
                    Ok(Phase::Render)
                }
                _ => Ok(Phase::Prompt),
            }
        }
    ];
}

#[derive(Eq, PartialEq)]
enum Input {
    PlayerCount,
    MatchType,
    Map,
}

impl Input {
    fn select_next(&mut self) {
        *self = match self {
            Self::PlayerCount => Self::MatchType,
            Self::MatchType => Self::Map,
            Self::Map => Self::PlayerCount,
        };
    }

    fn select_previous(&mut self) {
        *self = match self {
            Self::PlayerCount => Self::Map,
            Self::MatchType => Self::PlayerCount,
            Self::Map => Self::MatchType,
        };
    }
}
