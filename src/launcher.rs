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
    wad::Wad,
    warning,
};

const TITLE: &str = "Launcher";
const BORDER_COLOR: Color = Color::LightRed;
const WIDTH: u16 = CONSTRAINTS[0] + CONSTRAINTS[1] + 2;
const HEIGHT: u16 = 12;
const HEADER: [&str; 2] = ["Profile", "Wads"];
const CONSTRAINTS: [u16; 2] = [24, 6];
const LEGEND: &[[&str; 2]] = &[
    ["Enter", "Launch"],
    ["H", "Host"],
    ["N", "Join"],
    ["J↓/K↑", "Traverse"],
    ["}/{", "Swap"],
    ["A", "Add"],
    ["E", "Edit"],
    ["D", "Delete"],
    ["U", "Select UZDoom"],
    ["W", "Select Base Wad"],
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
        let profiles = config
            .profiles
            .iter()
            .map(|p| Profile::read(&config_dir, p))
            .collect();
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
                                self.profiles[*index]
                                    .base_wad
                                    .as_deref()
                                    .or(self.config.base_wad.as_deref()),
                                &self.profiles[*index],
                                &self.data_dir.join(&self.config.profiles[*index]),
                            )?;
                            Ok(Phase::Prompt)
                        } else {
                            warning::uzdoom_not_selected(terminal)?;
                            Ok(Phase::Render)
                        }
                    } else {
                        if let Some(profile) = editor::run_new(terminal, &self.profiles)? {
                            profile.write(&self.config_dir, &self.data_dir);
                            self.config.profiles.push(profile.name.clone());
                            self.config.write(&self.config_dir);
                            self.profiles.push(profile);
                            *index += 1;
                        }
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
                                self.profiles[*index]
                                    .base_wad
                                    .as_ref()
                                    .or(self.config.base_wad.as_ref()),
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
                        &mut self.config,
                        &mut self.profiles,
                        *index,
                        &self.config_dir,
                        &self.data_dir,
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
                    if let Some(profile) = editor::run_new(terminal, &self.profiles)? {
                        profile.write(&self.config_dir, &self.data_dir);
                        self.config.profiles.insert(*index, profile.name.clone());
                        self.config.write(&self.config_dir);
                        self.profiles.insert(*index, profile);
                        if *index == self.profiles.len() - 1 {
                            *index += 1;
                        }
                    }
                    Ok(Phase::Render)
                }
                KeyCode::Char('e') => {
                    if *index != self.profiles.len() {
                        self.config.profiles.remove(*index);
                        if let Some(profile) =
                            editor::run(terminal, self.profiles.remove(*index), &self.profiles)?
                        {
                            profile.write(&self.config_dir, &self.data_dir);
                            self.config.profiles.insert(*index, profile.name.clone());
                            self.config.write(&self.config_dir);
                            self.profiles.insert(*index, profile);
                        }
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('d') => {
                    if *index != self.profiles.len() {
                        if dialogue::should_delete(terminal, &self.config.profiles[*index])? {
                            self.config.profiles.remove(*index);
                            self.config.write(&self.config_dir);
                            self.profiles[*index].remove(&self.config_dir, &self.data_dir);
                            self.profiles.remove(*index);
                            if *index == self.profiles.len() && *index != 0 {
                                *index -= 1;
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
                KeyCode::Char('w') => {
                    let base_wad_path = rfd::FileDialog::new()
                        .set_directory(std::env::current_dir().unwrap())
                        .add_filter("base wad", Wad::EXTENSIONS)
                        .pick_file();
                    self.config.base_wad = if let Some(base_wad_path) = base_wad_path {
                        Some(Wad::from_path(&base_wad_path)?)
                    } else {
                        None
                    };
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
        let header = Row::new(HEADER.iter().map(|h| h.to_line().centered())).on_dark_gray();
        let rows = self
            .config
            .profiles
            .iter()
            .zip(&self.profiles)
            .map(|(n, p)| {
                Row::new([
                    n.to_line().centered(),
                    Line::from(p.wads.len().to_string()).centered(),
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
