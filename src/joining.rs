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
    wad::Wad,
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

    let Some((profile, joining_base_wad, hosting_base_wad_hash)) =
        resolve_profile(terminal, config, profiles, index, ip, config_dir, data_dir)?
    else {
        return Ok(());
    };
    let Some(uzdoom) = profile.uzdoom.as_ref().or(config.uzdoom.as_ref()) else {
        return warning::uzdoom_not_selected(terminal);
    };
    if hosting_base_wad_hash.is_empty()
        || joining_base_wad
            .as_ref()
            .is_none_or(|w| w.hash == hosting_base_wad_hash)
        || dialogue::run(
            terminal,
            "Selected base wad does not match, do you want to contiue?",
        )?
    {
        execute::join(uzdoom, joining_base_wad.as_deref(), &profile, ip)
    } else {
        Ok(())
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
) -> Result<Option<(Profile, Option<Wad>, String)>> {
    let manifest = network::Manifest::fetch(ip)?;
    let compatible_profiles: Vec<_> = profiles
        .iter()
        .filter(|p| manifest.is_compatible_with(p))
        .collect();

    if compatible_profiles.is_empty() {
        if dialogue::run(
            terminal,
            "You don't have the profile, would you like to download?",
        )? && let Some(profile_name) = name_profile(terminal, profiles)?
        {
            let downloads_dir = data_dir.join(".downloads");
            Ok(Some((
                network::download_profile(
                    terminal,
                    profile_name,
                    config,
                    profiles,
                    index,
                    ip,
                    &manifest.entries,
                    config_dir,
                    data_dir,
                    &downloads_dir,
                )?,
                config.base_wad.clone(),
                manifest.base_wad_hash,
            )))
        } else {
            Ok(None)
        }
    } else if compatible_profiles.len() == 1 {
        Ok(Some((
            compatible_profiles[0].clone(),
            compatible_profiles[0]
                .base_wad
                .clone()
                .or(config.base_wad.clone()),
            manifest.base_wad_hash.clone(),
        )))
    } else if let Some(profile) = select_profile(terminal, &compatible_profiles)? {
        Ok(Some((
            profile.clone(),
            profile.base_wad.clone().or(config.base_wad.clone()),
            manifest.base_wad_hash.clone(),
        )))
    } else {
        Ok(None)
    }
}

fn select_profile(terminal: &mut Terminal, profiles: &[&Profile]) -> Result<Option<Profile>> {
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

fn name_profile(terminal: &mut Terminal, profiles: &[Profile]) -> Result<Option<String>> {
    const TITLE: &str = "Name Profile";
    const WIDTH: u16 = 32;
    const HEIGHT: u16 = 5;
    const LEGEND: &[[&str; 2]] = &[
        ["Enter", "Save Profile"],
        ["A-Z/0-9/_", "Input"],
        ["Esc", "Cancel"],
    ];

    let mut name = String::new();
    stage![
        terminal,
        |frame| {
            let area = frame.area();
            let (window, mut window_area, indicator, indicator_area) =
                utility::new_window(TITLE, WIDTH, HEIGHT, BORDER_COLOR, area);
            frame.render_widget(window, window_area);
            frame.render_widget(indicator, indicator_area);
            window_area.y += 2;

            let (line, cursor_position) =
                utility::new_query("Name", &name, area.width, window_area.y);

            frame.render_widget(line, window_area);
            frame.set_cursor_position(cursor_position);
        },
        &mut |terminal, key_code| {
            match key_code {
                KeyCode::Enter => {
                    if name.is_empty() {
                        warning::empty_profile_name(terminal)?;
                        Ok(Phase::Render)
                    } else if profiles.iter().find(|p| p.name == name).is_some() {
                        warning::profile_already_exists(terminal)?;
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Return(Some(name.clone())))
                    }
                }
                KeyCode::Char(c @ 'a'..='z' | c @ 'A'..='Z' | c @ '0'..='9' | c @ '_') => {
                    name.push(c);
                    Ok(Phase::Render)
                }
                KeyCode::Backspace => {
                    name.pop();
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
    ];
}
