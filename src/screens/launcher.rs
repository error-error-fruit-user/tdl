use std::{ops::Deref, path::Path};

use directories::ProjectDirs;
use ratatui::{
    crossterm::event::KeyCode,
    layout::Constraint,
    style::{Color, Style},
    text::Line,
    widgets::{Paragraph, Row, Table, TableState},
};

use crate::{
    error::{Error, Result},
    layout, picker,
    popups::dialog::Dialog,
    popups::legend::Legend,
    popups::resize,
    popups::status,
    popups::warning::Warning,
    ritual::{self, Grimoire, Haunted, Page, Phase, Ritual},
    screens::editor::Editor,
    screens::host_settings::HostSettingsForm,
    screens::join_address::JoinAddressForm,
    screens::splash,
    screens::wrap_around,
    slayer::host_game,
    slayer::join_game,
    theme, uzdoom,
    vault::Vault,
    vault::scribe,
    vault::wad::Wad,
    widgets::details::Details,
};

/// The main screen: a table of profiles followed by a `+` row for creating a new one.
///
/// Owns the [`Vault`] and writes the config on quit.
pub struct Launcher {
    vault: Vault,
    table_state: TableState,
    /// Whether to play the splash screen first.
    splash: bool,
}

impl Launcher {
    const TITLE: &str = "launcher";
    const BORDER_COLOR: Color = theme::SCREEN;
    const HEADER: [&str; 2] = ["profile", "wads"];
    /// The last row, which makes a new profile.
    const NEW_ROW: &str = "+ new profile";
    /// The wads column: a count and the broken mark.
    const WADS_WIDTH: u16 = 5;
    const COLUMN_SPACING: u16 = 2;
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
    pub fn new(vault_dir: Option<String>, splash: bool) -> Self {
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
            splash,
        }
    }

    /// Builds the launcher and runs it until the user quits, after the splash screen if `splash`.
    pub fn summon(grimoire: &mut Grimoire, vault_dir: Option<String>, splash: bool) -> Result<()> {
        Self::new(vault_dir, splash).invoke(grimoire)
    }
}

impl Ritual for Launcher {
    type Return = ();

    /// Shows the "Text too big!" screen first if the terminal starts out too small, then deals with
    /// files saved by other editions: newer ones end the session untouched, older ones are
    /// upgraded if the user agrees, and the originals are deleted only if they agree to that too.
    fn attach(&mut self, grimoire: &mut Grimoire) -> Result<()> {
        let area = grimoire.consult().area();
        if layout::is_too_small(area.width, area.height) {
            resize::summon(grimoire)?;
        }
        if self.splash {
            splash::summon(grimoire)?;
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

    /// The profile list, and under it a panel for whatever the cursor is on: what that profile
    /// would launch with, or the global settings when it's on the new-profile row.
    fn render(&mut self, page: &mut Page) {
        // 🔎 only the profile under the cursor is looked at, so even a huge pack costs a handful of stats
        let selected = self.table_state.selected();
        let profile = selected.and_then(|i| self.vault.profiles.get(i));
        let details = match profile {
            Some(profile) => Details::launch(
                profile,
                &self.vault.config,
                &profile.faults(&self.vault.config),
            ),
            None => Details::global(&self.vault.config),
        };
        let is_broken = profile.is_some() && details.is_broken();

        let name_width = self
            .vault
            .profiles
            .iter()
            .map(|p| layout::width(&p.name))
            .chain([layout::width(Self::HEADER[0]), layout::width(Self::NEW_ROW)])
            .max()
            .unwrap_or(0);
        let header = Row::new([
            Line::from(Self::HEADER[0]),
            Line::from(Self::HEADER[1]).right_aligned(),
        ])
        .style(theme::HEADER);
        let rows = self
            .vault
            .profiles
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let mark = if is_broken && Some(i) == selected {
                    theme::BROKEN_MARK
                } else {
                    ""
                };
                Row::new([
                    Line::from(p.name.as_str()),
                    Line::from(format!("{}{mark}", p.wads.len())).right_aligned(),
                ])
            })
            .chain([Row::new([
                Line::styled(Self::NEW_ROW, Style::new().fg(theme::MUTED)),
                Line::default(),
            ])]);

        let (details_width, details_height) = details.size();
        let panel = layout::window(details_width, details_height);
        // the header, the profiles and the new-profile row
        let list_width = name_width + Self::COLUMN_SPACING + Self::WADS_WIDTH;
        let (width, height) = layout::window(list_width, self.vault.profiles.len() as u16 + 2);
        let list = layout::grow(width, height, panel.1, page.area());
        let [list_area, details_area] = layout::stack(&[list, panel], page.area())[..] else {
            return;
        };

        let table = Table::new(
            rows,
            [Constraint::Fill(1), Constraint::Length(Self::WADS_WIDTH)],
        )
        .column_spacing(Self::COLUMN_SPACING)
        .header(header)
        .block(ritual::conjure_veil(Self::TITLE, Self::BORDER_COLOR))
        .row_highlight_style(if is_broken {
            theme::BROKEN_CURSOR
        } else {
            theme::CURSOR
        });
        page.render_stateful_widget(table, list_area, &mut self.table_state);

        let panel = ritual::conjure_banner(details.title, theme::PANEL, details.title_color);
        page.render_widget(Paragraph::new(details.lines()).block(panel), details_area);

        let (sigil, sigil_area) = ritual::conjure_sigil(details_area);
        page.render_widget(sigil, sigil_area);
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
                    let profile = &self.vault.profiles[*index];
                    if let Some(uzdoom) = profile.uzdoom(&self.vault.config) {
                        // 🚧 a profile with a broken part is not fixed for you, but it isn't launched either
                        if !Warning::faults(grimoire, &profile.faults(&self.vault.config))? {
                            let result = uzdoom::launch(
                                uzdoom,
                                profile.base_wad(&self.vault.config).map(Wad::deref),
                                profile,
                                &self.vault.data_dir.join(&profile.name),
                            );
                            Warning::report_spawn(grimoire, result)?;
                        }
                    } else {
                        Warning::uzdoom_not_selected(grimoire)?;
                    }
                    Ok(Phase::Render)
                } else {
                    if let Some(profile) = Editor::summon_blank(grimoire, &self.vault)?
                        && self.vault.insert(index, profile).is_err()
                    {
                        Warning::save_failed(grimoire)?;
                    }
                    Ok(Phase::Render)
                }
            }
            KeyCode::Char('h') => {
                if *index != self.vault.profiles.len() {
                    let profile = &self.vault.profiles[*index];
                    if profile.uzdoom(&self.vault.config).is_none() {
                        Warning::uzdoom_not_selected(grimoire)?;
                    } else if !Warning::faults(grimoire, &profile.faults(&self.vault.config))?
                        && let Some(settings) = HostSettingsForm::summon(grimoire)?
                    {
                        host_game::summon(grimoire, &self.vault, *index, settings)?;
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
            KeyCode::Char('j') | KeyCode::Down => Ok(Phase::render_if(wrap_around::select_next(
                self.table_state.selected_mut(),
                self.vault.profiles.len() + 1,
            ))),
            KeyCode::Char('k') | KeyCode::Up => Ok(Phase::render_if(wrap_around::select_previous(
                self.table_state.selected_mut(),
                self.vault.profiles.len() + 1,
            ))),
            KeyCode::Char('}') => Ok(Phase::render_if_then(
                self.vault.profiles.len() != 1 && *index != self.vault.profiles.len(),
                || {
                    let next_index = wrap_around::next(*index, self.vault.profiles.len());
                    self.vault.profiles.swap(*index, next_index);
                    *index = next_index;
                },
            )),
            KeyCode::Char('{') => Ok(Phase::render_if_then(
                self.vault.profiles.len() != 1 && *index != self.vault.profiles.len(),
                || {
                    let previous_index = wrap_around::previous(*index, self.vault.profiles.len());
                    self.vault.profiles.swap(*index, previous_index);
                    *index = previous_index;
                },
            )),
            KeyCode::Char('a') => {
                if let Some(profile) = Editor::summon_blank(grimoire, &self.vault)?
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
                if let Some(path) = picker::file(grimoire, None, None)? {
                    self.vault.config.uzdoom = Some(path.into());
                }
                Ok(Phase::Prompt)
            }
            KeyCode::Char('w') => {
                if let Some(path) =
                    picker::file(grimoire, Some(("base wad", Wad::EXTENSIONS)), None)?
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
