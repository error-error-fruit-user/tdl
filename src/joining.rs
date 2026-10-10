use std::path::Path;

use ratatui::{
    crossterm::event::KeyCode,
    style::{Color, Style},
    text::ToLine,
    widgets::{Row, Table, TableState},
};

use crate::{
    config::Config,
    dialogue,
    error::Result,
    execute, index, legend, network,
    profile::Profile,
    stage,
    utility::{self, Phase, Terminal},
    warning,
};

const BORDER_COLOR: Color = Color::LightMagenta;

pub fn run(
    terminal: &mut Terminal,
    config: &mut Config,
    profiles: &mut Vec<Profile>,
    index: usize,
    ip: &str,
    config_dir: &Path,
    data_dir: &Path,
) -> Result<()> {
    const TITLE: &str = "Joining";
    const WIDTH: u16 = 32;
    const HEIGHT: u16 = 5;

    terminal.draw(|frame| {
        let (window, mut window_area, ..) =
            utility::new_window(TITLE, WIDTH, HEIGHT, BORDER_COLOR, frame.area());

        frame.render_widget(window, window_area);
        window_area.y += 2;
        frame.render_widget("Joining...".to_line().centered(), window_area);
    })?;

    let Some(profile) =
        resolve_profile(terminal, config, profiles, index, ip, config_dir, data_dir)?
    else {
        return Ok(());
    };

    if let Some(uzdoom) = profile.uzdoom.as_ref().or(config.uzdoom.as_ref()) {
        execute::join(uzdoom, &profile, ip, &data_dir.join(&profile.name))
    } else {
        warning::uzdoom_not_selected(terminal)
    }
}

fn resolve_profile(
    terminal: &mut Terminal,
    config: &mut Config,
    profiles: &mut Vec<Profile>,
    index: usize,
    ip: &str,
    config_dir: &Path,
    data_dir: &Path,
) -> Result<Option<Profile>> {
    let manifest = network::Manifest::fetch(ip)?;
    let mut compatible_profiles = Vec::new();

    for profile in profiles.iter() {
        if manifest.is_compatible_with(profile) {
            compatible_profiles.push(profile.clone());
        }
    }

    if compatible_profiles.is_empty() {
        drop(compatible_profiles);
        if dialogue::run(
            terminal,
            "You don't have the profile, would you like to download?",
        )? {
            let downloads_dir = data_dir.join("downloads");
            Ok(Some(network::download_profile(
                terminal,
                config,
                profiles,
                index,
                ip,
                &manifest.entries,
                config_dir,
                data_dir,
                &downloads_dir,
            )?))
        } else {
            Ok(None)
        }
    } else if compatible_profiles.len() == 1 {
        Ok(Some(profiles[0].clone()))
    } else {
        select_profile(terminal, compatible_profiles)
    }
}

fn select_profile(terminal: &mut Terminal, profiles: Vec<Profile>) -> Result<Option<Profile>> {
    const TITLE: &str = "Select Profile";
    const WIDTH: u16 = 32;
    const HEIGHT: u16 = 12;
    const LEGEND: &[[&str; 2]] = &[
        ["Enter", "Select Profile"],
        ["J/K", "Traverse"],
        ["Esc", "Cancel"],
    ];

    let mut table_state = TableState::new();
    table_state.select(Some(0));
    let rows = profiles
        .iter()
        .map(|p| Row::new([p.name.to_line().centered()]));

    stage![
        terminal,
        |frame| {
            let (window, window_area, indicator, indicator_area) =
                utility::new_window(TITLE, WIDTH, HEIGHT, BORDER_COLOR, frame.area());
            let table = Table::new(rows.clone(), [WIDTH])
                .block(window)
                .row_highlight_style(Style::new().black().bg(BORDER_COLOR));
            frame.render_stateful_widget(table, window_area, &mut table_state);
            frame.render_widget(indicator, indicator_area);
        },
        &mut |terminal, key_code| {
            let index = table_state.selected_mut().as_mut().unwrap();
            match key_code {
                KeyCode::Enter => Ok(Phase::Return(Some(profiles[*index].clone()))),
                KeyCode::Char('j') => {
                    *index = index::next(*index, profiles.len());
                    Ok(Phase::Render)
                }
                KeyCode::Char('k') => {
                    *index = index::previous(*index, profiles.len());
                    Ok(Phase::Render)
                }
                KeyCode::Esc => Ok(Phase::Return(None)),
                KeyCode::Char('?') => {
                    legend::run(terminal, LEGEND)?;
                    Ok(Phase::Render)
                }
                _ => Ok(Phase::Prompt),
            }
        }
    ]
}
