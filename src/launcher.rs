use std::{
    io::{BufRead, BufReader, BufWriter, Stdout, Write},
    path::PathBuf,
    process::Stdio,
};

use ratatui::{
    Terminal,
    buffer::Buffer,
    crossterm::event::{self},
    layout::{Alignment, Rect},
    prelude::CrosstermBackend,
    style::{Color, Style},
    text::{Line, ToLine},
    widgets::{Cell, Row, StatefulWidget, Table, TableState, Widget},
};

use crate::{
    dialogue, editor,
    error::Result,
    index, legend, present,
    profile::Profile,
    resize,
    window::{self, Step, new_window},
};

pub fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    let area = terminal.get_frame().area();
    if resize::should_resize(area.width, area.height) {
        resize::run(terminal)?;
    }
    let mut state = TableState::new();
    let mut launcher = Launcher::new(&mut state);
    present![terminal, launcher, &mut state];
}

struct Launcher {
    profiles: Vec<Profile>,
    executable_file: Option<PathBuf>,
}

impl Launcher {
    const ENGINE_FILE: &'static str = "engine.txt";
    const PROFILES_FILE: &'static str = "profiles.json";
    const TITLE: &'static str = "Launcher";
    const HEADERS: [&'static str; 2] = ["Profile", "Mods"];
    const CONSTRAINTS: [u16; 2] = [24, 6];
    const WIDTH: u16 = Self::CONSTRAINTS[0] + Self::CONSTRAINTS[1] + 2;
    const HEIGHT: u16 = 12;

    fn new(state: &mut TableState) -> Self {
        let profiles = read_profiles();
        state.select(Some(0));
        Self {
            profiles,
            executable_file: read_engine_path(),
        }
    }

    fn prompt(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        state: &mut TableState,
    ) -> Result<Step<()>> {
        window::process(terminal, &mut |terminal, key_code| match key_code {
            event::KeyCode::Esc => {
                if dialogue::run(terminal, "Are you sure you want to quit?")? {
                    Ok(Step::Return(()))
                } else {
                    Ok(Step::Render)
                }
            }
            event::KeyCode::Enter => {
                let index = state.selected().unwrap();
                if index != self.profiles.len() {
                    self.launch(index);
                    Ok(Step::Prompt)
                } else {
                    let profile = editor::run(terminal, Profile::default())?;
                    self.profiles.push(profile);
                    state.select(Some(index + 1));
                    write_profiles(&self.profiles);
                    Ok(Step::Render)
                }
            }
            event::KeyCode::Char('?') => {
                legend::run(
                    terminal,
                    &[
                        ["Esc", "Quit"],
                        ["Enter", "Launch"],
                        ["W", "Set Executable"],
                        ["J/K", "Traverse"],
                        ["A", "Add"],
                        ["E", "Edit"],
                        ["F", "Import Folder"],
                        ["D", "Delete"],
                        ["[/]", "Swap"],
                    ],
                )?;
                Ok(Step::Render)
            }
            event::KeyCode::Char('a') => {
                let profile = editor::run(terminal, Profile::default())?;
                let index = state.selected().unwrap();
                self.profiles.insert(index, profile);
                if index == self.profiles.len() - 1 {
                    state.select(Some(index + 1));
                }
                write_profiles(&self.profiles);
                Ok(Step::Render)
            }
            event::KeyCode::Char('e') => {
                let index = state.selected().unwrap();
                if index != self.profiles.len() {
                    let profile = editor::run(terminal, self.profiles.remove(index))?;
                    self.profiles.insert(index, profile);
                    write_profiles(&self.profiles);
                    Ok(Step::Render)
                } else {
                    Ok(Step::Prompt)
                }
            }
            event::KeyCode::Char('d') => {
                let index = state.selected().unwrap();
                if index != self.profiles.len() {
                    let should_delete =
                        dialogue::run(terminal, "Are you sure you want to delete?")?;
                    if should_delete {
                        self.profiles.remove(index);
                        if index == self.profiles.len() {
                            if index != 0 {
                                state.select(Some(index - 1));
                            }
                        }
                        write_profiles(&self.profiles);
                    }
                    Ok(Step::Render)
                } else {
                    Ok(Step::Prompt)
                }
            }
            event::KeyCode::Char('j') => {
                if index::select_next(state.selected_mut(), self.profiles.len() + 1) {
                    Ok(Step::Render)
                } else {
                    Ok(Step::Prompt)
                }
            }
            event::KeyCode::Char('k') => {
                if index::select_previous(state.selected_mut(), self.profiles.len() + 1) {
                    Ok(Step::Render)
                } else {
                    Ok(Step::Prompt)
                }
            }
            event::KeyCode::Char('w') => {
                self.set_engine_path();
                Ok(Step::Prompt)
            }
            event::KeyCode::Char(']') => {
                if let Some(index) = state.selected_mut()
                    && self.profiles.len() != 1
                    && *index != self.profiles.len()
                {
                    let next_index = index::next(*index, self.profiles.len());
                    self.profiles.swap(*index, next_index);
                    *index = next_index;
                    Ok(Step::Render)
                } else {
                    Ok(Step::Prompt)
                }
            }
            event::KeyCode::Char('[') => {
                if let Some(index) = state.selected_mut()
                    && self.profiles.len() != 1
                    && *index != self.profiles.len()
                {
                    let previous_index = index::previous(*index, self.profiles.len());
                    self.profiles.swap(*index, previous_index);
                    *index = previous_index;
                    Ok(Step::Render)
                } else {
                    Ok(Step::Prompt)
                }
            }
            _ => Ok(Step::Prompt),
        })
    }

    pub fn set_engine_path(&mut self) {
        let path = rfd::FileDialog::new()
            .set_directory(std::env::current_dir().unwrap())
            .pick_file()
            .unwrap();
        write_engine_path(path.to_str().unwrap());
        self.executable_file = Some(path);
    }

    pub fn launch(&self, index: usize) {
        let mut doom = std::process::Command::new(self.executable_file.as_ref().unwrap())
            .args(self.profiles[index].mods.iter())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        doom.wait().unwrap();
        if let Some(stdout) = doom.stdout.take() {
            let reader = BufReader::new(stdout);
            let log = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open("session.log")
                .unwrap();
            let mut writer = BufWriter::new(log);
            for line in reader.lines() {
                writeln!(&mut writer, "{}", line.unwrap()).unwrap();
            }
            writer.flush().unwrap();
        }
    }
}

impl StatefulWidget for &Launcher {
    type State = TableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let header =
            Row::new(Launcher::HEADERS.iter().map(|header| {
                Cell::from(Line::from(header.to_string()).alignment(Alignment::Center))
            }));
        let mut rows = self
            .profiles
            .iter()
            .map(|profile| {
                Row::new(
                    [
                        Cell::from(Line::from(profile.name.clone()).alignment(Alignment::Center)),
                        Cell::from(
                            Line::from(profile.mods.len().to_string()).alignment(Alignment::Center),
                        ),
                    ]
                    .into_iter(),
                )
            })
            .collect::<Vec<_>>();
        rows.push(Row::new(
            ["+".to_line().alignment(Alignment::Center)].into_iter(),
        ));

        let (window, window_area, question_area) = new_window(
            Launcher::WIDTH,
            Launcher::HEIGHT,
            Launcher::TITLE,
            Color::LightRed,
            area,
        );

        let table = Table::new(rows, Launcher::CONSTRAINTS)
            .header(header)
            .block(window)
            .highlight_symbol("> ")
            .row_highlight_style(Style::new().fg(Color::Black).bg(Color::White));
        StatefulWidget::render(table, window_area, buf, state);
        "[?]"
            .to_line()
            .alignment(Alignment::Center)
            .render(question_area, buf);
    }
}

fn read_profiles() -> Vec<Profile> {
    let Ok(profiles) = std::fs::read_to_string(Launcher::PROFILES_FILE) else {
        return Vec::new();
    };
    serde_json::from_str(&profiles).unwrap_or_default()
}

pub fn write_profiles(profiles: &Vec<Profile>) {
    std::fs::write(
        Launcher::PROFILES_FILE,
        serde_json::to_string_pretty(profiles).unwrap(),
    )
    .unwrap();
}

fn read_engine_path() -> Option<PathBuf> {
    std::fs::read_to_string(Launcher::ENGINE_FILE)
        .ok()
        .map(PathBuf::from)
}

fn write_engine_path(engine_path: &str) {
    std::fs::write(Launcher::ENGINE_FILE, engine_path).unwrap();
}
