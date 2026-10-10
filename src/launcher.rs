use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use ratatui::{
    buffer::Buffer,
    crossterm::event::KeyCode,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, ToLine},
    widgets::{Row, StatefulWidget, Table, TableState, Widget},
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

const TITLE: &str = "Launcher";
const BORDER_COLOR: Color = Color::LightRed;
const WIDTH: u16 = CONSTRAINTS[0] + CONSTRAINTS[1] + 2;
const HEIGHT: u16 = 12;
const HEADER: [&str; 2] = ["Profile", "Mods"];
const CONSTRAINTS: [u16; 2] = [24, 6];
const LEGEND: &[[&str; 2]] = &[
    ["Enter", "Launch"],
    ["H", "Host"],
    ["N", "Join"],
    ["J↓/K↑", "Traverse"],
    ["}/{", "Move"],
    ["A", "Add"],
    ["E", "Edit"],
    ["D", "Delete"],
    ["U", "Select UZDoom"],
    ["Esc", "Save and Quit"],
];

pub fn run(terminal: &mut Terminal, pass: Option<String>) -> Result<()> {
    let area = terminal.get_frame().area();
    if resize::should_resize(area.width, area.height) {
        resize::run(terminal)?;
    }
    let mut state = TableState::new();
    let mut launcher = Launcher::new(&mut state, pass);
    present![terminal, launcher, &mut state];
}

struct Launcher {
    config_dir: PathBuf,
    data_dir: PathBuf,
    config: Config,
    profiles: Vec<Profile>,
}

impl Launcher {
    fn new(state: &mut TableState, pass: Option<String>) -> Self {
        let (config_dir, data_dir) = if let Some(pass) = pass {
            let pass = Path::new(&pass);
            (pass.join("config"), pass.join("data"))
        } else {
            let project_dirs = ProjectDirs::from("error.error.fruit", "eef", "tdl")
                .expect("Could not find home directory!");
            (
                project_dirs.config_dir().to_path_buf(),
                project_dirs.data_dir().to_path_buf(),
            )
        };
        let config = Config::read(&config_dir);
        let mut profiles = Vec::new();
        profiles.reserve(config.profiles.len());
        for profile in &config.profiles {
            profiles.push(Profile::read(&config_dir, profile));
        }
        std::fs::create_dir_all(config_dir.join("profiles")).unwrap();

        state.select(Some(0));
        Self {
            config_dir: config_dir.into(),
            data_dir: data_dir.into(),
            config,
            profiles,
        }
    }

    fn prompt(&mut self, terminal: &mut Terminal, state: &mut TableState) -> Result<Phase<()>> {
        utility::prompt(terminal, &mut |terminal, key_code| {
            let index = state.selected_mut().as_mut().unwrap();
            match key_code {
                KeyCode::Enter => {
                    if *index != self.profiles.len() {
                        if let Some(uzdoom) = self.profiles[*index]
                            .uzdoom
                            .as_ref()
                            .or(self.config.uzdoom.as_ref())
                        {
                            execute::launch(
                                uzdoom,
                                &self.profiles[*index],
                                &self.data_dir.join(&self.config.profiles[*index]),
                            )?;
                            Ok(Phase::Prompt)
                        } else {
                            warning::uzdoom_not_selected(terminal)?;
                            Ok(Phase::Render)
                        }
                    } else {
                        let profile = editor::run_new(terminal)?;
                        profile.write(&self.config_dir, &self.data_dir);
                        self.config.profiles.push(profile.name.clone());
                        self.config.write(&self.config_dir);
                        self.profiles.push(profile);
                        *index += 1;
                        Ok(Phase::Render)
                    }
                }
                KeyCode::Char('h') => {
                    if *index != self.profiles.len() {
                        if let Some(uzdoom) = self.profiles[*index]
                            .uzdoom
                            .as_ref()
                            .or(self.config.uzdoom.as_ref())
                        {
                            host::run(
                                terminal,
                                uzdoom,
                                &self.profiles[*index],
                                &self.data_dir.join(&self.config.profiles[*index]),
                            )?;
                        } else {
                            warning::uzdoom_not_selected(terminal)?;
                        }
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('n') => {
                    join::run(
                        terminal,
                        &self.config.uzdoom,
                        &self.profiles,
                        &self.data_dir.join(&self.config.profiles[*index]),
                    )?;
                    Ok(Phase::Render)
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    if index::select_next(state.selected_mut(), self.profiles.len() + 1) {
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if index::select_previous(state.selected_mut(), self.profiles.len() + 1) {
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('}') => {
                    if self.profiles.len() != 1 && *index != self.profiles.len() {
                        let next_index = index::next(*index, self.profiles.len());
                        self.config.profiles.swap(*index, next_index);
                        self.profiles.swap(*index, next_index);
                        *index = next_index;
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('{') => {
                    if self.profiles.len() != 1 && *index != self.profiles.len() {
                        let previous_index = index::previous(*index, self.profiles.len());
                        self.config.profiles.swap(*index, previous_index);
                        self.profiles.swap(*index, previous_index);
                        *index = previous_index;
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('a') => {
                    let profile = editor::run_new(terminal)?;
                    profile.write(&self.config_dir, &self.data_dir);
                    self.config.profiles.insert(*index, profile.name.clone());
                    self.config.write(&self.config_dir);
                    self.profiles.insert(*index, profile);
                    if *index == self.profiles.len() - 1 {
                        *index += 1;
                    }
                    Ok(Phase::Render)
                }
                KeyCode::Char('e') => {
                    if *index != self.profiles.len() {
                        self.config.profiles.remove(*index);
                        let profile = editor::run(terminal, self.profiles.remove(*index))?;
                        profile.write(&self.config_dir, &self.data_dir);
                        self.config.profiles.insert(*index, profile.name.clone());
                        self.config.write(&self.config_dir);
                        self.profiles.insert(*index, profile);
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('d') => {
                    if *index != self.profiles.len() {
                        if dialogue::should_delete(terminal, &self.config.profiles[*index])? {
                            let name = self.config.profiles.remove(*index);
                            self.config.write(&self.config_dir);
                            self.profiles[*index].remove(&self.config_dir, &self.data_dir, name);
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
                KeyCode::Char('u') => {
                    self.config.uzdoom = rfd::FileDialog::new()
                        .set_directory(std::env::current_dir().unwrap())
                        .pick_file()
                        .map(PathBuf::into);
                    Ok(Phase::Prompt)
                }
                KeyCode::Esc => {
                    if dialogue::should_quit(terminal)? {
                        self.config.write(&self.config_dir);
                        Ok(Phase::Return(()))
                    } else {
                        Ok(Phase::Render)
                    }
                }
                KeyCode::Char('?') => {
                    legend::run(terminal, LEGEND)?;
                    Ok(Phase::Render)
                }
                _ => Ok(Phase::Prompt),
            }
        })
    }
}

impl StatefulWidget for &Launcher {
    type State = TableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let header =
            Row::new(HEADER.iter().map(|header| header.to_line().centered())).on_dark_gray();
        let rows = self
            .config
            .profiles
            .iter()
            .zip(&self.profiles)
            .map(|(name, profile)| {
                Row::new([
                    name.to_line().centered(),
                    Line::from(profile.mods.len().to_string()).centered(),
                ])
            })
            .chain(std::iter::once(Row::new(std::iter::once(
                "+".to_line().centered(),
            ))));

        let (window, window_area, indicator, indicator_area) =
            utility::new_window(TITLE, WIDTH, HEIGHT, BORDER_COLOR, area);

        let table = Table::new(rows, CONSTRAINTS)
            .header(header)
            .block(window)
            .row_highlight_style(Style::new().black().bg(BORDER_COLOR));
        StatefulWidget::render(table, window_area, buf, state);
        indicator.render(indicator_area, buf);
    }
}
