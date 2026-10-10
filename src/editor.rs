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
    profile::{Mod, Profile},
    utility::{self, Phase, Terminal},
};

const BORDER_COLOR: Color = Color::LightBlue;
const WIDTH: u16 = CONSTRAINTS[0] + CONSTRAINTS[1];
const HEIGHT: u16 = 12;
const CONSTRAINTS: [u16; 2] = [5, 27];
const HEADER: [&str; 2] = ["Order", "Mod"];
const NAMING_LEGEND: &[[&str; 2]] = &[
    ["Enter/Esc", "Save and Quit"],
    ["Tab", "Switch Mode"],
    ["A-Z/0-9/_", "Input"],
];
const EDITING_LEGEND: &[[&str; 2]] = &[
    ["Enter/Esc", "Save and Quit"],
    ["Tab", "Switch Mode"],
    ["J↓/K↑", "Traverse"],
    ["}/{", "Swap"],
    ["A", "Add"],
    ["D", "Delete"],
    ["U", "Select UZDoom"],
];

pub fn run(terminal: &mut Terminal, profile: Profile) -> Result<Profile> {
    let mut state = TableState::new();
    let mut editor = Editor::new(profile, &mut state);
    present_with!(terminal, editor, &mut state, |frame: &mut Frame| {
        let area = frame.area();
        if !editor.editing_mods {
            frame.set_cursor_position(editor.get_cursor_position(area));
        }
        frame.render_stateful_widget(&editor, area, &mut state);
    });
}

pub fn run_new(terminal: &mut Terminal) -> Result<Profile> {
    run(terminal, Profile::default())
}

#[derive(Default)]
struct Editor {
    profile: Profile,
    editing_mods: bool,
}

impl Editor {
    fn new(profile: Profile, state: &mut TableState) -> Self {
        state.select(Some(0));
        Self {
            editing_mods: !profile.name.is_empty(),
            profile,
        }
    }

    fn prompt(
        &mut self,
        terminal: &mut Terminal,
        state: &mut TableState,
    ) -> Result<Phase<Profile>> {
        utility::prompt(terminal, &mut |terminal, key_code| {
            let index = state.selected_mut().as_mut().unwrap();
            match key_code {
                KeyCode::Esc | KeyCode::Enter => Ok(Phase::Return(self.profile.clone())),
                KeyCode::Tab => {
                    self.editing_mods = !self.editing_mods;
                    Ok(Phase::Render)
                }
                KeyCode::Char(c @ 'a'..='z' | c @ 'A'..='Z' | c @ '0'..='9' | c @ '_')
                    if !self.editing_mods =>
                {
                    self.profile.name.push(c);
                    Ok(Phase::Render)
                }
                KeyCode::Backspace if !self.editing_mods => {
                    self.profile.name.pop();
                    Ok(Phase::Render)
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    if index::select_next(state.selected_mut(), self.profile.mods.len() + 1) {
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if index::select_previous(state.selected_mut(), self.profile.mods.len() + 1) {
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('}') => {
                    if self.profile.mods.len() != 1 && *index != self.profile.mods.len() {
                        let next_index = index::next(*index, self.profile.mods.len());
                        self.profile.mods.swap(*index, next_index);
                        *index = next_index;
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('{') => {
                    if self.profile.mods.len() != 1 && *index != self.profile.mods.len() {
                        let previous_index = index::previous(*index, self.profile.mods.len());
                        self.profile.mods.swap(*index, previous_index);
                        *index = previous_index;
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('a') => {
                    let mod_paths = rfd::FileDialog::new()
                        .set_directory(std::env::current_dir().unwrap())
                        .add_filter("wads", &["wad", "pk3"])
                        .pick_files();
                    let Some(mod_paths) = mod_paths else {
                        return Ok(Phase::Prompt);
                    };
                    if mod_paths.is_empty() {
                        return Ok(Phase::Prompt);
                    }
                    let length = mod_paths.len();
                    let should_add = *index == self.profile.mods.len();
                    for (index, path) in (*index..*index + length).zip(mod_paths) {
                        self.profile
                            .mods
                            .insert(index, Mod::from_path(path.into())?);
                    }
                    if should_add {
                        *index += length
                    }
                    Ok(Phase::Render)
                }
                KeyCode::Char('d') => {
                    if *index != self.profile.mods.len() {
                        if dialogue::should_delete(
                            terminal,
                            &Path::new(&self.profile.mods[*index].path)
                                .file_name()
                                .unwrap()
                                .to_string_lossy(),
                        )? {
                            self.profile.mods.remove(*index);
                            if *index == self.profile.mods.len() && *index != 0 {
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
                KeyCode::Char('?') => {
                    let legend = if self.editing_mods {
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

impl StatefulWidget for &Editor {
    type State = TableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut TableState) {
        let paths: Vec<_> = self
            .profile
            .mods
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
        let table = if self.editing_mods {
            table.row_highlight_style(Style::new().black().bg(BORDER_COLOR))
        } else {
            table
        };
        StatefulWidget::render(table, window_area, buf, state);
        Widget::render(indicator, indicator_area, buf);
    }
}
