use std::path::PathBuf;

use directories::ProjectDirs;
use ratatui::{
    buffer::Buffer,
    crossterm::event::KeyCode,
    layout::Rect,
    style::{Color, Style},
    text::{Line, ToLine},
    widgets::{Cell, Row, StatefulWidget, Table, TableState, Widget},
};

use crate::{
    config::Config,
    dialogue, editor,
    error::Result,
    execute, host, index, join, legend, present,
    profile::Profile,
    resize,
    utility::{self, Phase, Terminal},
    warning,
};

const TITLE: &'static str = "Launcher";
const BORDER_COLOR: Color = Color::LightRed;
const WIDTH: u16 = CONSTRAINTS[0] + CONSTRAINTS[1] + 2;
const HEIGHT: u16 = 12;
const HEADERS: [&'static str; 2] = ["Profile", "Mods"];
const CONSTRAINTS: [u16; 2] = [24, 6];
const LEGEND: &[[&'static str; 2]] = &[
    ["Esc", "Quit"],
    ["Enter", "Launch"],
    ["H", "Host"],
    ["N", "Join"],
    ["U", "Set UZDoom"],
    ["J/K", "Traverse"],
    ["A", "Add"],
    ["E", "Edit"],
    ["F", "Import Folder"],
    ["D", "Delete"],
    ["[/]", "Swap"],
];

pub fn run(terminal: &mut Terminal) -> Result<()> {
    let area = terminal.get_frame().area();
    if resize::should_resize(area.width, area.height) {
        resize::run(terminal)?;
    }
    let mut state = TableState::new();
    let mut launcher = Launcher::new(&mut state);
    present![terminal, launcher, &mut state];
}

struct Launcher {
    config_dir: PathBuf,
    // data_dir: PathBuf,
    config: Config,
    profiles: Vec<Profile>,
}

impl Launcher {
    fn new(state: &mut TableState) -> Self {
        let project_dirs = ProjectDirs::from("error.error.fruit", "eef", "tdl")
            .expect("Could not find home directory!");
        let config_dir = project_dirs.config_dir();
        // let data_dir = project_dirs.data_dir();
        let config = Config::read(config_dir);
        let mut profiles = Vec::new();
        profiles.reserve(config.profiles.len());
        for profile in &config.profiles {
            profiles.push(Profile::read(config_dir, profile));
        }
        std::fs::create_dir_all(config_dir.join("profiles")).unwrap();
        // std::fs::create_dir_all(data_dir).unwrap();

        state.select(Some(0));
        Self {
            config_dir: config_dir.into(),
            // data_dir: data_dir.into(),
            config,
            profiles,
        }
    }

    fn prompt(&mut self, terminal: &mut Terminal, state: &mut TableState) -> Result<Phase<()>> {
        utility::prompt(terminal, &mut |terminal, key_code| {
            let index = state.selected_mut().as_mut().unwrap();
            match key_code {
                KeyCode::Esc => {
                    if dialogue::should_quit(terminal)? {
                        self.config.write(&self.config_dir);
                        Ok(Phase::Return(()))
                    } else {
                        Ok(Phase::Render)
                    }
                }
                KeyCode::Enter => {
                    if *index != self.profiles.len() {
                        if execute::launch(&self.config.uzdoom, &self.profiles[*index])? {
                            Ok(Phase::Prompt)
                        } else {
                            warning::uzdoom_not_set(terminal)?;
                            Ok(Phase::Render)
                        }
                    } else {
                        let (name, profile) =
                            editor::run(terminal, String::new(), Profile::default())?;
                        profile.write(&self.config_dir, &name);
                        self.config.profiles.push(name);
                        self.config.write(&self.config_dir);
                        self.profiles.push(profile);
                        *index += 1;
                        Ok(Phase::Render)
                    }
                }
                KeyCode::Char('?') => {
                    legend::run(terminal, LEGEND)?;
                    Ok(Phase::Render)
                }
                KeyCode::Char('a') => {
                    let (name, profile) = editor::run_new(terminal)?;
                    profile.write(&self.config_dir, &name);
                    self.config.profiles.push(name);
                    self.config.write(&self.config_dir);
                    self.profiles.insert(*index, profile);
                    if *index == self.profiles.len() - 1 {
                        *index += 1;
                    }
                    Ok(Phase::Render)
                }
                KeyCode::Char('e') => {
                    if *index != self.profiles.len() {
                        let (name, profile) = editor::run(
                            terminal,
                            self.config.profiles.remove(*index),
                            self.profiles.remove(*index),
                        )?;
                        profile.write(&self.config_dir, &name);
                        self.config.profiles.push(name);
                        self.config.write(&self.config_dir);
                        self.profiles.insert(*index, profile);
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('d') => {
                    if *index != self.profiles.len() {
                        if dialogue::should_delete(terminal)? {
                            let name = self.config.profiles.remove(*index);
                            self.config.write(&self.config_dir);
                            self.profiles[*index].remove(&self.config_dir, name);
                            self.profiles.remove(*index);
                            if *index == self.profiles.len() {
                                if *index != 0 {
                                    *index -= 1;
                                }
                            }
                        }
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('j') => {
                    if index::select_next(state.selected_mut(), self.profiles.len() + 1) {
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('k') => {
                    if index::select_previous(state.selected_mut(), self.profiles.len() + 1) {
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char(']') => {
                    if self.profiles.len() != 1 && *index != self.profiles.len() {
                        let next_index = index::next(*index, self.profiles.len());
                        self.profiles.swap(*index, next_index);
                        *index = next_index;
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('[') => {
                    if self.profiles.len() != 1 && *index != self.profiles.len() {
                        let previous_index = index::previous(*index, self.profiles.len());
                        self.profiles.swap(*index, previous_index);
                        *index = previous_index;
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('h') => {
                    if *index != self.profiles.len() {
                        host::run(terminal, &self.config.uzdoom, &self.profiles[*index])?;
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('n') => {
                    if *index != self.profiles.len() {
                        join::run(terminal, &self.config.uzdoom, &self.profiles[*index])?;
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('u') => {
                    if let Some(uzdoom) = rfd::FileDialog::new()
                        .set_directory(std::env::current_dir().unwrap())
                        .pick_file()
                    {
                        self.config.uzdoom = uzdoom.to_str().unwrap().to_string();
                    }
                    Ok(Phase::Prompt)
                }
                _ => Ok(Phase::Prompt),
            }
        })
    }
}

impl StatefulWidget for &Launcher {
    type State = TableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let header = Row::new(
            HEADERS
                .iter()
                .map(|header| Cell::from(Line::from(header.to_string()).centered())),
        );
        let mut rows = self
            .config
            .profiles
            .iter()
            .zip(&self.profiles)
            .map(|(name, profile)| {
                Row::new(
                    [
                        Cell::from(name.to_line().centered()),
                        Cell::from(Line::from(profile.mods.len().to_string()).centered()),
                    ]
                    .into_iter(),
                )
            })
            .collect::<Vec<_>>();
        rows.push(Row::new(["+".to_line().centered()].into_iter()));

        let (window, window_area, indicator, indicator_area) =
            utility::new_window(WIDTH, HEIGHT, TITLE, BORDER_COLOR, area);

        let table = Table::new(rows, CONSTRAINTS)
            .header(header)
            .block(window)
            .highlight_symbol("> ")
            .row_highlight_style(Style::new().black().on_white());
        StatefulWidget::render(table, window_area, buf, state);
        indicator.render(indicator_area, buf);
    }
}
