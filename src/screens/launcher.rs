use std::{ops::Deref, path::Path};

use directories::ProjectDirs;
use ratatui::{
    crossterm::event::KeyCode,
    style::{Color, Style, Stylize},
    text::{Line, ToLine},
    widgets::{Row, Table, TableState},
};

use crate::{
    error::{Error, Result},
    popups::dialog::Dialog,
    popups::legend::Legend,
    popups::resize,
    popups::status,
    popups::warning::Warning,
    ritual::{self, Grimoire, Haunted, Page, Phase, Ritual},
    screens::editor::Editor,
    screens::host_settings::HostSettingsForm,
    screens::join_address::JoinAddressForm,
    screens::wrap_around,
    slayer::host_game,
    slayer::join_game,
    uzdoom,
    vault::Vault,
    vault::scribe,
    vault::wad::Wad,
};

/// The main screen: a table of profiles followed by a `+` row for creating a new one.
///
/// Owns the [`Vault`] and writes the config on quit.
pub struct Launcher {
    vault: Vault,
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

    /// Uses `<vault_dir>/{config,data}` when given, otherwise the platform's standard directories.
    pub fn new(vault_dir: Option<String>) -> Self {
        let (config_dir, data_dir) = if let Some(pass) = vault_dir {
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
            vault: Vault::new(config_dir, data_dir),
            table_state: TableState::new().with_selected(0),
        }
    }

    /// Builds the launcher and runs it until the user quits.
    pub fn summon(grimoire: &mut Grimoire, vault_dir: Option<String>) -> Result<()> {
        Self::new(vault_dir).invoke(grimoire)
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
        if self.vault.is_too_new() {
            Warning::files_too_new(grimoire)?;
            return Err(Error::Exit);
        }
        // 📜 anything that was fished out of the wreckage on the way in
        for notice in std::mem::take(&mut self.vault.notices) {
            Warning::summon(grimoire, &notice)?;
        }
        if self.vault.is_outdated() {
            if !Dialog::should_upgrade(grimoire)? {
                return Err(Error::Exit);
            }
            // ⏳ checking every page takes a moment, so don't leave the old dialog hanging there
            status::summon(grimoire, "Upgrading", "Upgrading...", Self::BORDER_COLOR)?;
            let Ok(upgraded) = self.vault.upgrade() else {
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
            .vault
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
                if *index != self.vault.profiles.len() {
                    if let Some(uzdoom) = self.vault.profiles[*index].uzdoom(&self.vault.config) {
                        uzdoom::launch(
                            uzdoom,
                            self.vault.profiles[*index]
                                .base_wad(&self.vault.config)
                                .map(Wad::deref),
                            &self.vault.profiles[*index],
                            &self.vault.data_dir.join(&self.vault.profiles[*index].name),
                        )?;
                        Ok(Phase::Prompt)
                    } else {
                        Warning::uzdoom_not_selected(grimoire)?;
                        Ok(Phase::Render)
                    }
                } else {
                    if let Some(profile) = Editor::summon_blank(grimoire, &self.vault.profiles)?
                        && self.vault.insert(index, profile).is_err()
                    {
                        Warning::save_failed(grimoire)?;
                    }
                    Ok(Phase::Render)
                }
            }
            KeyCode::Char('h') => {
                if *index != self.vault.profiles.len() {
                    if self.vault.profiles[*index]
                        .uzdoom(&self.vault.config)
                        .is_some()
                    {
                        if let Some(settings) = HostSettingsForm::summon(grimoire)? {
                            host_game::summon(grimoire, &self.vault, *index, settings)?;
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
                if let Some(ip) = JoinAddressForm::summon(grimoire)? {
                    join_game::summon(grimoire, &mut self.vault, index, &ip)?;
                }
                Ok(Phase::Render)
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if wrap_around::select_next(
                    self.table_state.selected_mut(),
                    self.vault.profiles.len() + 1,
                ) {
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Prompt)
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if wrap_around::select_previous(
                    self.table_state.selected_mut(),
                    self.vault.profiles.len() + 1,
                ) {
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Prompt)
                }
            }
            KeyCode::Char('}') => {
                if self.vault.profiles.len() != 1 && *index != self.vault.profiles.len() {
                    let next_index = wrap_around::next(*index, self.vault.profiles.len());
                    self.vault.profiles.swap(*index, next_index);
                    *index = next_index;
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Prompt)
                }
            }
            KeyCode::Char('{') => {
                if self.vault.profiles.len() != 1 && *index != self.vault.profiles.len() {
                    let previous_index = wrap_around::previous(*index, self.vault.profiles.len());
                    self.vault.profiles.swap(*index, previous_index);
                    *index = previous_index;
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Prompt)
                }
            }
            KeyCode::Char('a') => {
                if let Some(profile) = Editor::summon_blank(grimoire, &self.vault.profiles)?
                    && self.vault.insert(index, profile).is_err()
                {
                    Warning::save_failed(grimoire)?;
                }
                Ok(Phase::Render)
            }
            KeyCode::Char('e') => {
                if *index != self.vault.profiles.len() {
                    Editor::summon_existing(grimoire, &mut self.vault, *index)?;
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Prompt)
                }
            }
            KeyCode::Char('d') => {
                if *index != self.vault.profiles.len() {
                    if Dialog::should_delete(grimoire, &self.vault.profiles[*index].name)?
                        && self.vault.remove(index).is_err()
                    {
                        Warning::save_failed(grimoire)?;
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
                    self.vault.config.uzdoom = Some(path.into());
                }
                Ok(Phase::Prompt)
            }
            KeyCode::Char('w') => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_directory(std::env::current_dir().unwrap())
                    .add_filter("base wad", Wad::EXTENSIONS)
                    .pick_file()
                {
                    self.vault.config.base_wad = Some(Wad::from_path(&path)?);
                }
                Ok(Phase::Prompt)
            }
            KeyCode::Esc => {
                if Dialog::should_quit(grimoire)? {
                    let saved = self
                        .vault
                        .config
                        .write(&self.vault.config_dir, &self.vault.profiles)
                        .is_ok();
                    // 💾 if the last save failed, leaving is the user's call, not ours
                    if saved || Dialog::should_quit_unsaved(grimoire)? {
                        Ok(Phase::Return(()))
                    } else {
                        Ok(Phase::Render)
                    }
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
