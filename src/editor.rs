use std::io::Stdout;

use ratatui::{
    Frame, Terminal,
    buffer::Buffer,
    crossterm::event::{self},
    layout::{Alignment, Position, Rect},
    prelude::CrosstermBackend,
    style::{Color, Style},
    text::ToLine,
    widgets::{Cell, Row, StatefulWidget, Table, TableState, Widget},
};

use crate::{
    dialogue,
    error::Result,
    index, legend, present_with,
    profile::Profile,
    window::{self, Phase},
};

const WIDTH: u16 = 32;
const HEIGHT: u16 = 12;
const NAMING_LEGEND: &[[&'static str; 2]] = &[
    ["Enter/Esc", "Quit to Launcher"],
    ["Tab", "Switch Mode"],
    ["A-Z/0-9/_", "Input"],
];
const EDITING_LEGEND: &[[&'static str; 2]] = &[
    ["Enter/Esc", "Quit to Launcher"],
    ["Tab", "Switch Mode"],
    ["A", "Add"],
    ["D", "Delete"],
    ["F", "Import"],
    ["[/]", "Swap"],
];

pub fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>, profile: Profile) -> Result<Profile> {
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
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        state: &mut TableState,
    ) -> Result<Phase<Profile>> {
        window::process(terminal, &mut |terminal, key_code| {
            let index = state.selected_mut().as_mut().unwrap();
            match key_code {
                event::KeyCode::Esc | event::KeyCode::Enter => {
                    Ok(Phase::Return(self.profile.clone()))
                }
                event::KeyCode::Char('?') => {
                    let legend = if self.editing_mods {
                        EDITING_LEGEND
                    } else {
                        NAMING_LEGEND
                    };
                    legend::run(terminal, legend)?;
                    Ok(Phase::Render)
                }
                event::KeyCode::Tab => {
                    self.editing_mods = !self.editing_mods;
                    Ok(Phase::Render)
                }
                event::KeyCode::Char(c @ 'a'..='z' | c @ 'A'..='Z' | c @ '0'..='9' | c @ '_')
                    if !self.editing_mods =>
                {
                    self.profile.name.push(c);
                    Ok(Phase::Render)
                }
                event::KeyCode::Backspace if !self.editing_mods => {
                    self.profile.name.pop();
                    Ok(Phase::Render)
                }
                event::KeyCode::Char('j') => {
                    if index::select_next(state.selected_mut(), self.profile.mods.len() + 1) {
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                event::KeyCode::Char('k') => {
                    if index::select_previous(state.selected_mut(), self.profile.mods.len() + 1) {
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                event::KeyCode::Char('d') => {
                    if *index != self.profile.mods.len() {
                        let should_delete =
                            dialogue::run(terminal, "Are you sure you want to delete?")?;
                        if should_delete {
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
                event::KeyCode::Char(']') => {
                    if self.profile.mods.len() != 1 && *index != self.profile.mods.len() {
                        let next_index = index::next(*index, self.profile.mods.len());
                        self.profile.mods.swap(*index, next_index);
                        *index = next_index;
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                event::KeyCode::Char('[') => {
                    if self.profile.mods.len() != 1 && *index != self.profile.mods.len() {
                        let previous_index = index::previous(*index, self.profile.mods.len());
                        self.profile.mods.swap(*index, previous_index);
                        *index = previous_index;
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                event::KeyCode::Char('a') => {
                    let new_mods = rfd::FileDialog::new()
                        .set_directory(std::env::current_dir().unwrap())
                        .add_filter("wads", &["wad", "pk3"])
                        .pick_files();
                    let Some(new_mods) = new_mods else {
                        return Ok(Phase::Prompt);
                    };
                    if new_mods.is_empty() {
                        return Ok(Phase::Prompt);
                    }
                    let new_mods = new_mods.iter().map(|f| f.to_string_lossy().to_string());
                    let length = new_mods.len();
                    let should_add = *index == self.profile.mods.len();
                    for (index, r#mod) in (*index..*index + new_mods.len()).zip(new_mods) {
                        self.profile.mods.insert(index, r#mod);
                    }
                    if should_add {
                        *index += length
                    }
                    Ok(Phase::Render)
                }
                event::KeyCode::Char('f') => {
                    let folder = rfd::FileDialog::new()
                        .set_directory(std::env::current_dir().unwrap())
                        .pick_folder();
                    let Some(folder) = folder else {
                        return Ok(Phase::Prompt);
                    };
                    let new_mods = std::fs::read_dir(folder)
                        .into_iter()
                        .flatten()
                        .map(|entry| entry.unwrap().path())
                        .filter(|path| {
                            path.is_file()
                                && matches!(
                                    path.extension()
                                        .map(|extension| extension.to_str().unwrap()),
                                    Some("wad" | "pk3")
                                )
                        })
                        .map(|path| path.to_string_lossy().to_string())
                        .collect::<Vec<_>>();
                    if new_mods.is_empty() {
                        return Ok(Phase::Prompt);
                    }
                    let length = new_mods.len();
                    let should_add = *index == self.profile.mods.len();
                    for (index, r#mod) in (*index..*index + new_mods.len()).zip(new_mods) {
                        self.profile.mods.insert(index, r#mod);
                    }
                    if should_add {
                        *index += length;
                    }
                    Ok(Phase::Render)
                }
                _ => Ok(Phase::Prompt),
            }
        })
    }

    fn get_cursor_position(&self, area: Rect) -> Position {
        Position::new(
            area.width / 2 + self.profile.name.len() as u16 / 2,
            area.height / 2 - HEIGHT as u16 / 2,
        )
    }
}

impl StatefulWidget for &Editor {
    type State = TableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut TableState) {
        let mut rows = self
            .profile
            .mods
            .iter()
            .map(|file| {
                Row::new([Cell::from(file.to_line().alignment(Alignment::Right))].into_iter())
            })
            .collect::<Vec<_>>();
        rows.push(Row::new(
            ["+".to_line().alignment(Alignment::Center)].into_iter(),
        ));
        let (window, window_area, indicator, indicator_area) =
            window::new(WIDTH, HEIGHT, &self.profile.name, Color::LightBlue, area);
        let table = Table::new(rows, [WIDTH]).block(window);
        let table = if self.editing_mods {
            table
                .highlight_symbol("> ")
                .row_highlight_style(Style::new().fg(Color::Black).bg(Color::White))
        } else {
            table
        };
        StatefulWidget::render(table, window_area, buf, state);
        Widget::render(indicator, indicator_area, buf);
    }
}
