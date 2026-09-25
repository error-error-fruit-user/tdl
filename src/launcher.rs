use std::{
    io::{BufRead, BufReader, BufWriter, Stdout, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
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
    host, index, join, legend, present,
    profile::Profile,
    resize,
    window::{self, Phase},
};

const TITLE: &'static str = "Launcher";
const WIDTH: u16 = CONSTRAINTS[0] + CONSTRAINTS[1] + 2;
const HEIGHT: u16 = 12;
const HEADERS: [&'static str; 2] = ["Profile", "Mods"];
const CONSTRAINTS: [u16; 2] = [24, 6];
const ENGINE_FILE: &'static str = "engine.txt";
const PROFILES_FILE: &'static str = "profiles.json";
const LEGEND: &[[&'static str; 2]] = &[
    ["Esc", "Quit"],
    ["Enter", "Launch"],
    ["H", "Host"],
    ["N", "Join"],
    ["W", "Set Executable"],
    ["J/K", "Traverse"],
    ["A", "Add"],
    ["E", "Edit"],
    ["F", "Import Folder"],
    ["D", "Delete"],
    ["[/]", "Swap"],
];

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
    ) -> Result<Phase<()>> {
        window::process(terminal, &mut |terminal, key_code| {
            let index = state.selected_mut().as_mut().unwrap();
            match key_code {
                event::KeyCode::Esc => {
                    if dialogue::run(terminal, "Are you sure you want to quit?")? {
                        Ok(Phase::Return(()))
                    } else {
                        Ok(Phase::Render)
                    }
                }
                event::KeyCode::Enter => {
                    if *index != self.profiles.len() {
                        let Some(executable_file) = self.executable_file.as_ref() else {
                            return Ok(Phase::Prompt);
                        };
                        launch(executable_file, &self.profiles[*index])?;
                        Ok(Phase::Prompt)
                    } else {
                        let profile = editor::run(terminal, Profile::default())?;
                        self.profiles.push(profile);
                        *index += 1;
                        write_profiles(&self.profiles);
                        Ok(Phase::Render)
                    }
                }
                event::KeyCode::Char('?') => {
                    legend::run(terminal, LEGEND)?;
                    Ok(Phase::Render)
                }
                event::KeyCode::Char('a') => {
                    let profile = editor::run(terminal, Profile::default())?;
                    self.profiles.insert(*index, profile);
                    if *index == self.profiles.len() - 1 {
                        *index += 1;
                    }
                    write_profiles(&self.profiles);
                    Ok(Phase::Render)
                }
                event::KeyCode::Char('e') => {
                    if *index != self.profiles.len() {
                        let profile = editor::run(terminal, self.profiles.remove(*index))?;
                        self.profiles.insert(*index, profile);
                        write_profiles(&self.profiles);
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                event::KeyCode::Char('d') => {
                    if *index != self.profiles.len() {
                        if dialogue::run(terminal, "Are you sure you want to delete?")? {
                            self.profiles.remove(*index);
                            if *index == self.profiles.len() {
                                if *index != 0 {
                                    *index -= 1;
                                }
                            }
                            write_profiles(&self.profiles);
                        }
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                event::KeyCode::Char('j') => {
                    if index::select_next(state.selected_mut(), self.profiles.len() + 1) {
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                event::KeyCode::Char('k') => {
                    if index::select_previous(state.selected_mut(), self.profiles.len() + 1) {
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                event::KeyCode::Char('w') => {
                    self.set_engine_path();
                    Ok(Phase::Prompt)
                }
                event::KeyCode::Char(']') => {
                    if self.profiles.len() != 1 && *index != self.profiles.len() {
                        let next_index = index::next(*index, self.profiles.len());
                        self.profiles.swap(*index, next_index);
                        *index = next_index;
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                event::KeyCode::Char('[') => {
                    if self.profiles.len() != 1 && *index != self.profiles.len() {
                        let previous_index = index::previous(*index, self.profiles.len());
                        self.profiles.swap(*index, previous_index);
                        *index = previous_index;
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                event::KeyCode::Char('h') => {
                    if *index != self.profiles.len()
                        && let Some(executable_file) = self.executable_file.as_ref()
                        && let Some(player_count) = host::run(terminal)?
                    {
                        host(executable_file, &self.profiles[*index], player_count)?;
                    }
                    Ok(Phase::Render)
                }
                event::KeyCode::Char('n') => {
                    if *index != self.profiles.len()
                        && let Some(executable_file) = self.executable_file.as_ref()
                        && let Some(ip) = join::run(terminal)?
                    {
                        join(executable_file, &self.profiles[*index], &ip)?;
                    }
                    Ok(Phase::Render)
                }
                _ => Ok(Phase::Prompt),
            }
        })
    }

    pub fn set_engine_path(&mut self) {
        let path = rfd::FileDialog::new()
            .set_directory(std::env::current_dir().unwrap())
            .pick_file();
        let Some(path) = path else {
            return;
        };
        write_engine_path(path.to_str().unwrap());
        self.executable_file = Some(path);
    }
}

impl StatefulWidget for &Launcher {
    type State = TableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let header =
            Row::new(HEADERS.iter().map(|header| {
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

        let (window, window_area, indicator, indicator_area) =
            window::new(WIDTH, HEIGHT, TITLE, Color::LightRed, area);

        let table = Table::new(rows, CONSTRAINTS)
            .header(header)
            .block(window)
            .highlight_symbol("> ")
            .row_highlight_style(Style::new().fg(Color::Black).bg(Color::White));
        StatefulWidget::render(table, window_area, buf, state);
        indicator.render(indicator_area, buf);
    }
}

fn read_profiles() -> Vec<Profile> {
    let Ok(profiles) = std::fs::read_to_string(PROFILES_FILE) else {
        return Vec::new();
    };
    serde_json::from_str(&profiles).unwrap_or_default()
}

pub fn write_profiles(profiles: &Vec<Profile>) {
    std::fs::write(
        PROFILES_FILE,
        serde_json::to_string_pretty(profiles).unwrap(),
    )
    .unwrap();
}

fn read_engine_path() -> Option<PathBuf> {
    std::fs::read_to_string(ENGINE_FILE).ok().map(PathBuf::from)
}

fn write_engine_path(engine_path: &str) {
    std::fs::write(ENGINE_FILE, engine_path).unwrap();
}

fn execute(executable_file: &Path, profile: &Profile, args: &[&str]) -> Result<()> {
    let mut doom = Command::new(executable_file)
        .args(args)
        .args(profile.mods.iter())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(stdout) = doom.stdout.take() {
        let reader = BufReader::new(stdout);
        let log = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open("session.log")?;
        let mut writer = BufWriter::new(log);
        for line in reader.lines() {
            writeln!(&mut writer, "{}", line.unwrap())?;
        }
        writer.flush()?;
    }
    Ok(())
}

fn launch(executable_file: &Path, profile: &Profile) -> Result<()> {
    execute(executable_file, profile, &[])
}

fn host(executable_file: &Path, profile: &Profile, player_count: u8) -> Result<()> {
    execute(
        executable_file,
        profile,
        &["-host", &player_count.to_string()],
    )
}

pub fn join(executable_file: &Path, profile: &Profile, ip: &str) -> Result<()> {
    execute(executable_file, profile, &["-join", ip])
}
