use std::path::Path;

use ratatui::{
    crossterm::event::KeyCode,
    style::{Color, Style},
    text::ToLine,
    widgets::{Cell, Row, Table, TableState},
};

use crate::{
    dialogue,
    error::Result,
    execute, index, legend, network,
    profile::Profile,
    stage,
    utility::{self, Phase, Terminal},
};

const BORDER_COLOR: Color = Color::LightMagenta;

pub fn run(
    terminal: &mut Terminal,
    uzdoom: &str,
    profiles: &Vec<Profile>,
    ip: &str,
    data_subdir: &Path,
) -> Result<()> {
    const TITLE: &'static str = "Joining";
    const WIDTH: u16 = 32;
    const HEIGHT: u16 = 5;

    terminal.draw(|frame| {
        let (window, mut window_area, ..) =
            utility::new_window(WIDTH, HEIGHT, TITLE, BORDER_COLOR, frame.area());

        frame.render_widget(window, window_area);
        window_area.y += 2;
        frame.render_widget("Joining...".to_line().centered(), window_area);
    })?;

    let Some(profile) = resolve_profile(terminal, profiles, ip, data_subdir)? else {
        return Ok(());
    };

    execute::join(uzdoom, &profile, ip, data_subdir)
}

fn resolve_profile(
    terminal: &mut Terminal,
    profiles: &Vec<Profile>,
    ip: &str,
    data_subdir: &Path,
) -> Result<Option<Profile>> {
    let manifest = network::Manifest::fetch(ip)?;
    let mut compatible_profiles = Vec::new();

    for profile in profiles {
        if manifest.is_compatible_with(profile) {
            compatible_profiles.push(profile.clone());
        }
    }

    if compatible_profiles.is_empty() {
        if dialogue::run(
            terminal,
            "You don't have the profile, would you like to download?",
        )? {
            let cache = data_subdir.join("downloads");
            Ok(Some(network::download_profile(
                terminal,
                ip,
                &manifest.entries,
                &cache,
            )?))
        } else {
            Ok(None)
        }
    } else if compatible_profiles.len() == 1 {
        Ok(Some(profiles[0].clone()))
    } else {
        choose_profile(terminal, compatible_profiles)
    }
}

fn choose_profile(terminal: &mut Terminal, profiles: Vec<Profile>) -> Result<Option<Profile>> {
    const WIDTH: u16 = 32;
    const HEIGHT: u16 = 12;
    const TITLE: &'static str = "Choose Profile";
    const LEGEND: &[[&'static str; 2]] = &[
        ["Enter", "Select Profile"],
        ["Esc", "Cancel"],
        ["J/K", "Traverse"],
    ];

    let mut table_state = TableState::new();
    table_state.select(Some(0));
    let rows = profiles
        .iter()
        .map(|p| Row::new([Cell::from(p.name.to_line().centered())].into_iter()))
        .collect::<Vec<_>>();

    stage![
        terminal,
        |frame| {
            let (window, window_area, indicator, indicator_area) =
                utility::new_window(WIDTH, HEIGHT, TITLE, BORDER_COLOR, frame.area());
            let table = Table::new(rows.clone(), [WIDTH])
                .block(window)
                .highlight_symbol("> ")
                .row_highlight_style(Style::new().black().on_white());
            frame.render_stateful_widget(table, window_area, &mut table_state);
            frame.render_widget(indicator, indicator_area);
        },
        &mut |terminal, key_code| {
            let index = table_state.selected_mut().as_mut().unwrap();
            match key_code {
                KeyCode::Esc => Ok(Phase::Return(None)),
                KeyCode::Enter => Ok(Phase::Return(Some(profiles[*index].clone()))),
                KeyCode::Char('?') => {
                    legend::run(terminal, LEGEND)?;
                    Ok(Phase::Render)
                }
                KeyCode::Char('j') => {
                    *index = index::next(*index, profiles.len());
                    Ok(Phase::Render)
                }
                KeyCode::Char('k') => {
                    *index = index::previous(*index, profiles.len());
                    Ok(Phase::Render)
                }
                _ => Ok(Phase::Prompt),
            }
        }
    ]
}
