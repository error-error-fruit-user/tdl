use ratatui::{
    Frame,
    buffer::Buffer,
    crossterm::event::KeyCode,
    layout::{Alignment, Position, Rect},
    style::{Color, Style},
    text::ToLine,
    widgets::{Cell, Row, StatefulWidget, Table, TableState, Widget},
};

use crate::{
    dialogue,
    error::Result,
    index, legend, present_with,
    profile::Profile,
    utility::{self, Phase, Terminal},
};

const BORDER_COLOR: Color = Color::LightBlue;
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

pub fn run(terminal: &mut Terminal, name: String, profile: Profile) -> Result<(String, Profile)> {
    let mut state = TableState::new();
    let mut editor = Editor::new(name, profile, &mut state);
    present_with!(terminal, editor, &mut state, |frame: &mut Frame| {
        let area = frame.area();
        if !editor.editing_mods {
            frame.set_cursor_position(editor.get_cursor_position(area));
        }
        frame.render_stateful_widget(&editor, area, &mut state);
    });
}

pub fn run_new(terminal: &mut Terminal) -> Result<(String, Profile)> {
    run(terminal, String::new(), Profile::default())
}

#[derive(Default)]
struct Editor {
    name: String,
    profile: Profile,
    editing_mods: bool,
}

impl Editor {
    fn new(name: String, profile: Profile, state: &mut TableState) -> Self {
        state.select(Some(0));
        Self {
            editing_mods: !name.is_empty(),
            name,
            profile,
        }
    }

    fn prompt(
        &mut self,
        terminal: &mut Terminal,
        state: &mut TableState,
    ) -> Result<Phase<(String, Profile)>> {
        utility::prompt(terminal, &mut |terminal, key_code| {
            let index = state.selected_mut().as_mut().unwrap();
            match key_code {
                KeyCode::Esc | KeyCode::Enter => {
                    Ok(Phase::Return((self.name.clone(), self.profile.clone())))
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
                KeyCode::Tab => {
                    self.editing_mods = !self.editing_mods;
                    Ok(Phase::Render)
                }
                KeyCode::Char(c @ 'a'..='z' | c @ 'A'..='Z' | c @ '0'..='9' | c @ '_')
                    if !self.editing_mods =>
                {
                    self.name.push(c);
                    Ok(Phase::Render)
                }
                KeyCode::Backspace if !self.editing_mods => {
                    self.name.pop();
                    Ok(Phase::Render)
                }
                KeyCode::Char('j') => {
                    if index::select_next(state.selected_mut(), self.profile.mods.len() + 1) {
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('k') => {
                    if index::select_previous(state.selected_mut(), self.profile.mods.len() + 1) {
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('d') => {
                    if *index != self.profile.mods.len() {
                        if dialogue::should_delete(terminal)? {
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
                KeyCode::Char(']') => {
                    if self.profile.mods.len() != 1 && *index != self.profile.mods.len() {
                        let next_index = index::next(*index, self.profile.mods.len());
                        self.profile.mods.swap(*index, next_index);
                        *index = next_index;
                        Ok(Phase::Render)
                    } else {
                        Ok(Phase::Prompt)
                    }
                }
                KeyCode::Char('[') => {
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
                KeyCode::Char('f') => {
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
            area.width / 2 + self.name.len() as u16 / 2,
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
        rows.push(Row::new(["+".to_line().centered()].into_iter()));
        let (window, window_area, indicator, indicator_area) =
            utility::new_window(WIDTH, HEIGHT, &self.name, BORDER_COLOR, area);
        let table = Table::new(rows, [WIDTH]).block(window);
        let table = if self.editing_mods {
            table
                .highlight_symbol("> ")
                .row_highlight_style(Style::new().black().on_white())
        } else {
            table
        };
        StatefulWidget::render(table, window_area, buf, state);
        Widget::render(indicator, indicator_area, buf);
    }
}
