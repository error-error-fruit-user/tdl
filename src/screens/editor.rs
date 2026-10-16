use std::path::Path;

use ratatui::{
    crossterm::event::KeyCode,
    layout::{Alignment, Position, Rect},
    style::{Color, Style, Stylize},
    text::{Line, ToLine},
    widgets::{Row, Table, TableState},
};

use crate::{
    error::Result,
    popups::dialog::Dialog,
    popups::legend::Legend,
    popups::warning::Warning,
    ritual::{self, Grimoire, Page, Phase, Ritual},
    screens::wrap_around,
    vault::Vault,
    vault::profile::Profile,
    vault::wad::Wad,
};

/// Creates or edits one profile; `Enter`/`Esc` both finish, and an empty untouched profile is discarded.
///
/// `profiles` is every existing profile, used to reject duplicate names; when editing, the
/// profile's own entry is excluded via `skipped_index`.
pub struct Editor<'a> {
    profile: Profile,
    profiles: &'a [Profile],
    skipped_index: Option<usize>,
    editing_wads: bool,
    table_state: TableState,
}

impl<'a> Editor<'a> {
    const BORDER_COLOR: Color = Color::LightBlue;
    const WIDTH: u16 = Self::CONSTRAINTS[0] + Self::CONSTRAINTS[1];
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

    /// Creates a profile. `None` if cancelled or left empty.
    pub fn summon_blank(
        grimoire: &mut Grimoire,
        profiles: &'a [Profile],
    ) -> Result<Option<Profile>> {
        Self::new(profiles).invoke(grimoire)
    }

    /// Edits `vault.profiles[index]` and saves the result. Cancelling keeps the original.
    pub fn summon_existing(grimoire: &mut Grimoire, vault: &mut Vault, index: usize) -> Result<()> {
        if let Some(profile) = Editor::edit(&vault.profiles, index).invoke(grimoire)?
            && vault.replace(index, profile).is_err()
        {
            Warning::save_failed(grimoire)?;
        }
        Ok(())
    }

    /// Starts on the name field.
    fn new(profiles: &'a [Profile]) -> Self {
        Self {
            profile: Profile::default(),
            profiles,
            skipped_index: None,
            editing_wads: false,
            table_state: TableState::new().with_selected(0),
        }
    }

    /// Starts on the wad list, with the existing profile loaded.
    fn edit(profiles: &'a [Profile], index: usize) -> Self {
        Self {
            profile: profiles[index].clone(),
            skipped_index: Some(index),
            editing_wads: true,
            ..Self::new(profiles)
        }
    }

    /// The cursor belongs in the window's title, which shows the name being typed.
    fn get_cursor_position(&self, area: Rect) -> Position {
        Position::new(
            area.width / 2 + self.profile.name.len() as u16 / 2,
            (area.height - Self::HEIGHT) / 2,
        )
    }
}

impl Ritual for Editor<'_> {
    type Return = Option<Profile>;

    fn render(&mut self, page: &mut Page) {
        let area = page.area();
        let (window, window_area, indicator, indicator_area) = ritual::conjure_window(
            &self.profile.name,
            Self::WIDTH,
            Self::HEIGHT,
            Self::BORDER_COLOR,
            area,
        );
        page.render_widget(indicator, indicator_area);

        let paths: Vec<_> = self
            .profile
            .wads
            .iter()
            .map(|w| Path::new(&w.path).file_name().unwrap().to_string_lossy())
            .collect();
        let header = Row::new(
            Self::HEADER
                .iter()
                .map(|header| header.to_line().centered()),
        )
        .on_dark_gray();

        let rows = paths
            .iter()
            .enumerate()
            .map(|(i, p)| {
                Row::new([
                    Line::from(format!("{}.", i + 1)).alignment(Alignment::Right),
                    p.to_line().centered(),
                ])
            })
            .chain([Row::new(["".to_line(), "+".to_line().centered()])]);
        let table = Table::new(rows, Self::CONSTRAINTS)
            .block(window)
            .header(header);
        let table = if self.editing_wads {
            table.row_highlight_style(Style::new().black().bg(Self::BORDER_COLOR))
        } else {
            table
        };
        page.render_stateful_widget(table, window_area, &mut self.table_state);
        if !self.editing_wads {
            page.set_cursor_position(self.get_cursor_position(area));
        }
    }

    fn prompt(&mut self, grimoire: &mut Grimoire, key_code: KeyCode) -> Result<Phase<Self::Tea>> {
        let index = self.table_state.selected_mut().as_mut().unwrap();
        match key_code {
            KeyCode::Esc | KeyCode::Enter => {
                if self.profile.name.is_empty() {
                    if self.profile.wads.is_empty()
                        && self.profile.uzdoom.is_none()
                        && self.profile.base_wad.is_none()
                    {
                        Ok(Phase::Return(None))
                    } else {
                        Warning::empty_profile_name(grimoire)?;
                        Ok(Phase::Render)
                    }
                } else if self
                    .profiles
                    .iter()
                    .enumerate()
                    .any(|(i, p)| Some(i) != self.skipped_index && p.name == self.profile.name)
                {
                    Warning::profile_already_exists(grimoire)?;
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
                if wrap_around::select_next(
                    self.table_state.selected_mut(),
                    self.profile.wads.len() + 1,
                ) {
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Prompt)
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if wrap_around::select_previous(
                    self.table_state.selected_mut(),
                    self.profile.wads.len() + 1,
                ) {
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Prompt)
                }
            }
            KeyCode::Char('}') => {
                if self.profile.wads.len() != 1 && *index != self.profile.wads.len() {
                    let next_index = wrap_around::next(*index, self.profile.wads.len());
                    self.profile.wads.swap(*index, next_index);
                    *index = next_index;
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Prompt)
                }
            }
            KeyCode::Char('{') => {
                if self.profile.wads.len() != 1 && *index != self.profile.wads.len() {
                    let previous_index = wrap_around::previous(*index, self.profile.wads.len());
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
                            && path
                                .extension()
                                .and_then(|e| e.to_str())
                                .is_some_and(|e| Wad::EXTENSIONS.contains(&e))
                        {
                            wad_paths.push(path);
                        }
                    }
                    wad_paths.sort();
                    let length = wad_paths.len();
                    let should_add = *index == self.profile.wads.len();
                    for (index, path) in (*index..*index + length).zip(wad_paths) {
                        self.profile.wads.insert(index, Wad::from_path(&path)?);
                    }
                    if should_add {
                        *index += length;
                    }
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Prompt)
                }
            }
            KeyCode::Char('d') => {
                if *index != self.profile.wads.len() {
                    if Dialog::should_delete(
                        grimoire,
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
                if let Some(path) = rfd::FileDialog::new()
                    .set_directory(std::env::current_dir().unwrap())
                    .pick_file()
                {
                    self.profile.uzdoom = Some(path.into());
                }
                Ok(Phase::Prompt)
            }
            KeyCode::Char('w') => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_directory(std::env::current_dir().unwrap())
                    .add_filter("base wad", Wad::EXTENSIONS)
                    .pick_file()
                {
                    self.profile.base_wad = Some(Wad::from_path(&path)?);
                }
                Ok(Phase::Prompt)
            }
            KeyCode::Char('i') => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_directory(std::env::current_dir().unwrap())
                    .add_filter("config", &["ini"])
                    .pick_file()
                {
                    self.profile.ini = Some(path.into());
                }
                Ok(Phase::Prompt)
            }
            KeyCode::Char('?') => {
                let legend = if self.editing_wads {
                    Self::EDITING_LEGEND
                } else {
                    Self::NAMING_LEGEND
                };
                Legend::summon(grimoire, legend)?;
                Ok(Phase::Render)
            }
            _ => Ok(Phase::Prompt),
        }
    }
}
