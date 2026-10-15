use std::{ops::Deref, path::Path};

use directories::ProjectDirs;
use ratatui::{
    crossterm::event::KeyCode,
    style::{Color, Style, Stylize},
    text::{Line, ToLine},
    widgets::{Row, Table, TableState},
};

use crate::{
    dialog::Dialog,
    editor::Editor,
    error::{Error, Result},
    execute,
    host::Host,
    hosting, index,
    join::Join,
    joining,
    legend::Legend,
    library::Library,
    resize,
    ritual::{self, Grimoire, Haunted, Page, Phase, Ritual},
    scribe,
    wad::Wad,
    warning::Warning,
};

/// The main screen: a table of profiles followed by a `+` row for creating a new one.
///
/// Owns the [`Library`] and writes the config on quit.
pub struct Launcher {
    library: Library,
    table_state: TableState,
}

impl Launcher {
    const TITLE: &str = "Launcher";
    const BORDER_COLOR: Color = Color::LightRed;
    const WIDTH: u16 = Self::CONSTRAINTS[0] + Self::CONSTRAINTS[1] + 2;
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

    /// Uses `<passed_dir>/{config,data}` when given, otherwise the platform's standard directories.
    pub fn new(passed_dir: Option<String>) -> Self {
        let (config_dir, data_dir) = if let Some(pass) = passed_dir {
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
        Self {
            library: Library::new(config_dir, data_dir),
            table_state: TableState::new().with_selected(0),
        }
    }

    /// Builds the launcher and runs it until the user quits.
    pub fn summon(grimoire: &mut Grimoire, passed_dir: Option<String>) -> Result<()> {
        Self::new(passed_dir).invoke(grimoire)
    }
}

impl Ritual for Launcher {
    type Return = ();

    /// Shows the "Text too big!" screen first if the terminal starts out too small, then deals with
    /// files saved by other editions: newer ones end the session untouched, older ones are
    /// upgraded if the user agrees, and the originals are deleted only if they agree to that too.
    fn attach(&mut self, grimoire: &mut Grimoire) -> Result<()> {
        let area = grimoire.consult().area();
        if resize::should_resize(area.width, area.height) {
            resize::summon(grimoire)?;
        }
        if self.library.is_too_new() {
            Warning::files_too_new(grimoire)?;
            return Err(Error::Exit);
        }
        if self.library.is_outdated() {
            if !Dialog::should_upgrade(grimoire)? {
                return Err(Error::Exit);
            }
            let Ok(upgraded) = self.library.upgrade() else {
                Warning::upgrade_failed(grimoire)?;
                return Err(Error::Exit);
            };
            if Dialog::should_delete_originals(grimoire)? {
                upgraded.iter().for_each(|path| scribe::retire(path));
            }
        }
        Ok(())
    }

    fn render(&mut self, page: &mut Page) {
        let header = Row::new(Self::HEADER.iter().map(|h| h.to_line().centered())).on_dark_gray();
        let rows = self
            .library
            .profiles
            .iter()
            .map(|p| {
                Row::new([
                    p.name.to_line().centered(),
                    Line::from(p.wads.len().to_string()).centered(),
                ])
            })
            .chain(std::iter::once(Row::new(std::iter::once(
                "+".to_line().centered(),
            ))));

        let (window, window_area, indicator, indicator_area) = ritual::conjure_window(
            Self::TITLE,
            Self::WIDTH,
            Self::HEIGHT,
            Self::BORDER_COLOR,
            page.area(),
        );
        page.render_widget(indicator, indicator_area);

        let table = Table::new(rows, Self::CONSTRAINTS)
            .header(header)
            .block(window)
            .row_highlight_style(Style::new().black().bg(Self::BORDER_COLOR));
        page.render_stateful_widget(table, window_area, &mut self.table_state);
    }

    fn prompt(
        &mut self,
        grimoire: &mut Grimoire,
        key_code: KeyCode,
    ) -> Result<Phase<Self::Return>> {
        let index = self.table_state.selected_mut().as_mut().unwrap();
        match key_code {
            KeyCode::Enter => {
                if *index != self.library.profiles.len() {
                    if let Some(uzdoom) = self.library.profiles[*index].uzdoom(&self.library.config)
                    {
                        execute::launch(
                            uzdoom,
                            self.library.profiles[*index]
                                .base_wad(&self.library.config)
                                .map(Wad::deref),
                            &self.library.profiles[*index],
                            &self
                                .library
                                .data_dir
                                .join(&self.library.profiles[*index].name),
                        )?;
                        Ok(Phase::Prompt)
                    } else {
                        Warning::uzdoom_not_selected(grimoire)?;
                        Ok(Phase::Render)
                    }
                } else {
                    if let Some(profile) = Editor::summon_blank(grimoire, &self.library.profiles)? {
                        self.library.insert(index, profile);
                    }
                    Ok(Phase::Render)
                }
            }
            KeyCode::Char('h') => {
                if *index != self.library.profiles.len() {
                    if self.library.profiles[*index]
                        .uzdoom(&self.library.config)
                        .is_some()
                    {
                        if let Some(settings) = Host::summon(grimoire)? {
                            hosting::summon(grimoire, &self.library, *index, settings)?;
                        }
                    } else {
                        Warning::uzdoom_not_selected(grimoire)?;
                    }
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Prompt)
                }
            }
            KeyCode::Char('n') => {
                if let Some(ip) = Join::summon(grimoire)? {
                    joining::summon(grimoire, &mut self.library, index, &ip)?;
                }
                Ok(Phase::Render)
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if index::select_next(
                    self.table_state.selected_mut(),
                    self.library.profiles.len() + 1,
                ) {
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Prompt)
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if index::select_previous(
                    self.table_state.selected_mut(),
                    self.library.profiles.len() + 1,
                ) {
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Prompt)
                }
            }
            KeyCode::Char('}') => {
                if self.library.profiles.len() != 1 && *index != self.library.profiles.len() {
                    let next_index = index::next(*index, self.library.profiles.len());
                    self.library.profiles.swap(*index, next_index);
                    *index = next_index;
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Prompt)
                }
            }
            KeyCode::Char('{') => {
                if self.library.profiles.len() != 1 && *index != self.library.profiles.len() {
                    let previous_index = index::previous(*index, self.library.profiles.len());
                    self.library.profiles.swap(*index, previous_index);
                    *index = previous_index;
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Prompt)
                }
            }
            KeyCode::Char('a') => {
                if let Some(profile) = Editor::summon_blank(grimoire, &self.library.profiles)? {
                    self.library.insert(index, profile);
                }
                Ok(Phase::Render)
            }
            KeyCode::Char('e') => {
                if *index != self.library.profiles.len() {
                    Editor::summon_existing(grimoire, &mut self.library, *index)?;
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Prompt)
                }
            }
            KeyCode::Char('d') => {
                if *index != self.library.profiles.len() {
                    if Dialog::should_delete(grimoire, &self.library.profiles[*index].name)? {
                        self.library.remove(index);
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
                    self.library.config.uzdoom = Some(path.into());
                }
                Ok(Phase::Prompt)
            }
            KeyCode::Char('w') => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_directory(std::env::current_dir().unwrap())
                    .add_filter("base wad", Wad::EXTENSIONS)
                    .pick_file()
                {
                    self.library.config.base_wad = Some(Wad::from_path(&path)?);
                }
                Ok(Phase::Prompt)
            }
            KeyCode::Esc => {
                if Dialog::should_quit(grimoire)? {
                    self.library
                        .config
                        .write(&self.library.config_dir, &self.library.profiles);
                    Ok(Phase::Return(()))
                } else {
                    Ok(Phase::Render)
                }
            }
            KeyCode::Char('?') => {
                Legend::summon(grimoire, Self::LEGEND)?;
                Ok(Phase::Render)
            }
            _ => Ok(Phase::Prompt),
        }
    }
}
