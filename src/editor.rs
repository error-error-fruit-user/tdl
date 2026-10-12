use std::path::{Path, PathBuf};

use ratatui::{
    Frame,
    buffer::Buffer,
    crossterm::event::KeyCode,
    layout::{Alignment, Position, Rect},
    style::{Color, Style, Stylize},
    text::{Line, ToLine},
    widgets::{Row, StatefulWidget, Table, TableState, Widget},
};

use crate::{
    dialogue,
    error::Result,
    index, legend, present_with,
    profile::Profile,
    utility::{self, Phase, Terminal},
    wad::Wad,
    warning,
};

const BORDER_COLOR: Color = Color::LightBlue;
const WIDTH: u16 = CONSTRAINTS[0] + CONSTRAINTS[1];
const HEIGHT: u16 = 12;
const CONSTRAINTS: [u16; 2] = [5, 27];
const HEADER: [&str; 2] = ["Order", "Wad"];
const NAMING_LEGEND: &[[&str; 2]] = &[
    ["Enter/Esc", "Save and Quit"],
    ["Tab", "Edit Wads"],
    ["A-Z/0-9/_", "Input"],
];
const EDITING_LEGEND: &[[&str; 2]] = &[
    ["Enter/Esc", "Save and Quit"],
    ["Tab", "Edit Name"],
    ["J↓/K↑", "Traverse"],
    ["}/{", "Swap"],
    ["A", "Add"],
    ["F", "Import from Folder"],
    ["D", "Delete"],
    ["U", "Select UZDoom"],
    ["W", "Select Base Wad"],
    ["I", "Select Ini"],
];

pub fn run(
    terminal: &mut Terminal,
    profile: Profile,
    profiles: &Vec<Profile>,
) -> Result<Option<Profile>> {
    let mut state = TableState::new();
    let mut editor = Editor::new(profile, profiles, &mut state);
    present_with!(terminal, editor, &mut state, |frame: &mut Frame| {
        let area = frame.area();
        if !editor.editing_wads {
            frame.set_cursor_position(editor.get_cursor_position(area));
        }
        frame.render_stateful_widget(&editor, area, &mut state);
    });
}

pub fn run_new(terminal: &mut Terminal, profiles: &Vec<Profile>) -> Result<Option<Profile>> {
    run(terminal, Profile::default(), profiles)
}

struct Editor<'a> {
    profile: Profile,
    profiles: &'a Vec<Profile>,
    editing_wads: bool,
}

impl<'a> Editor<'a> {
    fn new(profile: Profile, profiles: &'a Vec<Profile>, state: &mut TableState) -> Self {
        state.select(Some(0));
        Self {
            editing_wads: !profile.name.is_empty(),
            profile,
            profiles,
        }
    }

    fn prompt(
        &mut self,
        terminal: &mut Terminal,
        state: &mut TableState,
    ) -> Result<Phase<Option<Profile>>> {
        utility::prompt(terminal, &mut |terminal, key_code| {
            let index = state.selected_mut().as_mut().unwrap();
            match key_code {
                KeyCode::Esc | KeyCode::Enter => {
                    if self.profile.name.is_empty() {
                        if self.profile.wads.is_empty()
                            && self.profile.uzdoom.is_none()
                            && self.profile.base_wad.is_none()
                        {
                            Ok(Phase::Return(None))
                        } else {
                            warning::empty_profile_name(terminal)?;
                            Ok(Phase::Render)
                        }
                    } else if self
                        .profiles
                        .iter()
                        .find(|p| p.name == self.profile.name)
                        .is_some()
                    {
                        warning::profile_already_exists(terminal)?;
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Return(Some(self.profile.clone())))
                    }
                }
                KeyCode::Tab => {
                    self.editing_wads = !self.editing_wads;
                    Ok(Phase::Render)
                }
                KeyCode::Char(c @ 'a'..='z' | c @ 'A'..='Z' | c @ '0'..='9' | c @ '_')
                    if !self.editing_wads =>
                {
                    self.profile.name.push(c);
                    Ok(Phase::Render)
                }
                KeyCode::Backspace if !self.editing_wads => {
                    self.profile.name.pop();
                    Ok(Phase::Render)
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    if index::select_next(state.selected_mut(), self.profile.wads.len() + 1) {
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if index::select_previous(state.selected_mut(), self.profile.wads.len() + 1) {
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('}') => {
                    if self.profile.wads.len() != 1 && *index != self.profile.wads.len() {
                        let next_index = index::next(*index, self.profile.wads.len());
                        self.profile.wads.swap(*index, next_index);
                        *index = next_index;
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('{') => {
                    if self.profile.wads.len() != 1 && *index != self.profile.wads.len() {
                        let previous_index = index::previous(*index, self.profile.wads.len());
                        self.profile.wads.swap(*index, previous_index);
                        *index = previous_index;
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('a') => {
                    let mod_paths = rfd::FileDialog::new()
                        .set_directory(std::env::current_dir().unwrap())
                        .add_filter("wads", Wad::EXTENSIONS)
                        .pick_files();
                    let Some(mod_paths) = mod_paths else {
                        return Ok(Phase::Prompt);
                    };
                    if mod_paths.is_empty() {
                        return Ok(Phase::Prompt);
                    }
                    let length = mod_paths.len();
                    let should_add = *index == self.profile.wads.len();
                    for (index, path) in (*index..*index + length).zip(mod_paths) {
                        self.profile.wads.insert(index, Wad::from_path(&path)?);
                    }
                    if should_add {
                        *index += length
                    }
                    Ok(Phase::Render)
                }
                KeyCode::Char('f') => {
                    if let Some(folder) = rfd::FileDialog::new()
                        .set_directory(std::env::current_dir()?)
                        .pick_folder()
                    {
                        let mut wad_paths = Vec::new();
                        for entry in std::fs::read_dir(folder)? {
                            let path = entry?.path();
                            if path.is_file()
                                && matches!(
                                    path.extension().and_then(|e| e.to_str()),
                                    Some("wad" | "WAD" | "pk3")
                                )
                            {
                                wad_paths.push(path.to_path_buf());
                            }
                        }
                        let length = wad_paths.len();
                        let should_add = *index == self.profile.wads.len();
                        for (index, path) in (*index..*index + length).zip(wad_paths) {
                            self.profile.wads.insert(index, Wad::from_path(&path)?);
                        }
                        if should_add {
                            *index += length;
                            Ok(Phase::Render)
                        } else {
                            Ok(Phase::Prompt)
                        }
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('d') => {
                    if *index != self.profile.wads.len() {
                        if dialogue::should_delete(
                            terminal,
                            &Path::new(&self.profile.wads[*index].path)
                                .file_name()
                                .unwrap()
                                .to_string_lossy(),
                        )? {
                            self.profile.wads.remove(*index);
                            if *index == self.profile.wads.len() && *index != 0 {
                                *index -= 1;
                            }
                        }
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('u') => {
                    self.profile.uzdoom = rfd::FileDialog::new()
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
                    self.profile.base_wad = if let Some(base_wad_path) = base_wad_path {
                        Some(Wad::from_path(&base_wad_path)?)
                    } else {
                        None
                    };
                    Ok(Phase::Prompt)
                }
                KeyCode::Char('i') => {
                    self.profile.ini = rfd::FileDialog::new()
                        .set_directory(std::env::current_dir().unwrap())
                        .add_filter("config", &["ini"])
                        .pick_file()
                        .map(PathBuf::into);
                    Ok(Phase::Prompt)
                }
                KeyCode::Char('?') => {
                    let legend = if self.editing_wads {
                        EDITING_LEGEND
                    } else {
                        NAMING_LEGEND
                    };
                    legend::run(terminal, legend)?;
                    Ok(Phase::Render)
                }
                _ => Ok(Phase::Prompt),
            }
        })
    }

    fn get_cursor_position(&self, area: Rect) -> Position {
        Position::new(
            area.width / 2 + self.profile.name.len() as u16 / 2,
            (area.height - HEIGHT as u16) / 2,
        )
    }
}

impl StatefulWidget for &Editor<'_> {
    type State = TableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut TableState) {
        let paths: Vec<_> = self
            .profile
            .wads
            .iter()
            .map(|m| Path::new(&m.path).file_name().unwrap().to_string_lossy())
            .collect();
        let header =
            Row::new(HEADER.iter().map(|header| header.to_line().centered())).on_dark_gray();

        let rows = paths
            .iter()
            .enumerate()
            .map(|(i, p)| {
                Row::new([
                    Line::from(format!("{}.", i + 1)).alignment(Alignment::Right),
                    p.to_line().centered(),
                ])
            })
            .chain([Row::new(
                ["".to_line(), "+".to_line().centered()].into_iter(),
            )]);
        let (window, window_area, indicator, indicator_area) =
            utility::new_window(&self.profile.name, WIDTH, HEIGHT, BORDER_COLOR, area);
        let table = Table::new(rows, [CONSTRAINTS[0], CONSTRAINTS[1]])
            .block(window)
            .header(header);
        let table = if self.editing_wads {
            table.row_highlight_style(Style::new().black().bg(BORDER_COLOR))
        } else {
            table
        };
        StatefulWidget::render(table, window_area, buf, state);
        Widget::render(indicator, indicator_area, buf);
    }
}
