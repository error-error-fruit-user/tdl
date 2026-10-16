use std::{
    collections::HashMap,
    ffi::OsString,
    ops::Deref,
    path::{Path, PathBuf},
};

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
    popups::hashing,
    popups::legend::Legend,
    popups::resize,
    popups::running,
    popups::status,
    popups::three_way_dialog::ThreeWayDialog,
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
    vault::edition::{Edition, Loaded},
    vault::global::Global,
    vault::scribe,
    vault::wad::Wad,
    widgets::{
        details::Details,
        mark::{self, Mark},
    },
};

/// The main screen: a table of profiles followed by a `+` row for creating a new one.
///
/// Owns the [`Vault`] and writes the config on quit.
pub struct Launcher {
    vault: Vault,
    table_state: TableState,
    /// Whether to play the splash screen first.
    splash: bool,
    /// Each profile's mark, by position. A profile is only checked once its row is on screen, and
    /// the marks are forgotten whenever something could have changed them.
    marks: HashMap<usize, Option<Mark>>,
    /// What was passed on the command line, made absolute, for the startup questions about
    /// keeping it.
    passed_vault: Option<PathBuf>,
    ask_about_vault: bool,
    forget_vault: bool,
    passed_wads: Vec<PathBuf>,
    global: Global,
    global_path: PathBuf,
    /// False if `global.rite` is from a newer tdl, so it is never overwritten.
    global_writable: bool,
}

/// What the command line asked the launcher for.
#[derive(Default)]
pub struct Start {
    pub vault: Option<String>,
    /// Whether to ask about opening the passed vault every time; off for `--vault-once`.
    pub ask_about_vault: bool,
    /// Opens the standard vault and offers to forget the remembered one.
    pub forget_vault: bool,
    pub wads: Vec<String>,
    pub splash: bool,
}

/// `path` as an absolute path, so a folder kept from `tdl --wads .` still means the same folder
/// when tdl is started from somewhere else.
fn absolute(path: &str) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| PathBuf::from(path))
}

/// The wad folders to use when none are passed: the vault's own, then the global ones, each once.
fn remembered_folders(vault: &[OsString], global: &[OsString]) -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = Vec::new();
    for folder in vault.iter().chain(global) {
        let folder = PathBuf::from(folder);
        if !folders.contains(&folder) {
            folders.push(folder);
        }
    }
    folders
}

impl Launcher {
    const TITLE: &str = "launcher";
    const BORDER_COLOR: Color = theme::SCREEN;
    const HEADER: [&str; 2] = ["profile", "wads"];
    /// The last row, which makes a new profile.
    const NEW_ROW: &str = "+ new profile";
    /// The wads column: a count.
    const WADS_WIDTH: u16 = 4;
    const COLUMN_SPACING: u16 = 1;
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
        ["S", "Toggle Splash Screen"],
        ["Esc", "Save and Quit"],
    ];

    /// Opens the vault: `<vault>/{config,data}` when one is passed, the standard folders when
    /// asked to forget the kept one, otherwise the one kept in the global settings if it still
    /// exists, otherwise the standard folders (`standard_config_dir`
    /// and `standard_data_dir`, which also hold `global.rite`). The wad folders are the passed
    /// ones, or else the ones kept for this vault and then the global ones.
    pub fn new(start: Start, standard_config_dir: &Path, standard_data_dir: &Path) -> Self {
        let global_path = Global::path(standard_config_dir);
        let mut notices = Vec::new();
        let mut global_writable = true;
        let global = match Global::load(&global_path) {
            Loaded::Fine(global) => global,
            Loaded::Missing => Global::default(),
            Loaded::Restored(global) => {
                notices.push("Restored your global settings from their backup".to_string());
                global
            }
            Loaded::Lost => {
                notices.push("Your global settings were unreadable, saved as .corrupt".to_string());
                Global::default()
            }
            Loaded::TooNew => {
                global_writable = false;
                Global::default()
            }
        };
        let passed_vault = start.vault.as_deref().map(absolute);
        let kept_vault = global.vault.as_ref().map(PathBuf::from).filter(|kept| {
            if start.forget_vault {
                return false;
            }
            let there = kept.is_dir();
            if !there && passed_vault.is_none() {
                notices.push(format!(
                    "Your usual vault {} is missing, so the standard one is open",
                    kept.display()
                ));
            }
            there
        });
        let (config_dir, data_dir) = match passed_vault.clone().or(kept_vault) {
            Some(root) => (root.join("config"), root.join("data")),
            None => (standard_config_dir.into(), standard_data_dir.into()),
        };
        let mut vault = Vault::new(config_dir, data_dir);
        vault.notices.extend(notices);
        let passed_wads: Vec<_> = start.wads.iter().map(|folder| absolute(folder)).collect();
        vault.wad_folders = if passed_wads.is_empty() {
            remembered_folders(&vault.config.wad_folders, &global.wad_folders)
        } else {
            passed_wads.clone()
        };
        Self {
            vault,
            table_state: TableState::new().with_selected(0),
            splash: start.splash,
            marks: HashMap::new(),
            passed_vault,
            ask_about_vault: start.ask_about_vault,
            forget_vault: start.forget_vault,
            passed_wads,
            global,
            global_path,
            global_writable,
        }
    }

    /// Builds the launcher and runs it until the user quits, after the splash screen if asked.
    pub fn summon(grimoire: &mut Grimoire, start: Start) -> Result<()> {
        let project_dirs = ProjectDirs::from("error.error.fruit", "eef", "tdl")
            .expect("Could not find home directory!");
        Self::new(start, project_dirs.config_dir(), project_dirs.data_dir()).invoke(grimoire)
    }

    /// The startup questions about what was passed on the command line: whether to open the
    /// passed vault every time, and whether to keep the passed wad folders for this session,
    /// globally, or for this vault. Folders that don't exist are dropped first, with a warning.
    fn offer_to_keep(&mut self, grimoire: &mut Grimoire) -> Result<()> {
        let missing: Vec<_> = self
            .passed_wads
            .iter()
            .filter(|folder| !folder.is_dir())
            .map(|folder| folder.display().to_string())
            .collect();
        if !missing.is_empty() {
            Warning::summon(
                grimoire,
                &format!("Not a folder, so left out: {}", missing.join(", ")),
            )?;
            self.passed_wads.retain(|folder| folder.is_dir());
            // 📂 none of them left: the kept folders are better than none
            self.vault.wad_folders = if self.passed_wads.is_empty() {
                remembered_folders(&self.vault.config.wad_folders, &self.global.wad_folders)
            } else {
                self.passed_wads.clone()
            };
        }

        if self.forget_vault {
            match self.global.vault.clone() {
                Some(kept) if self.global_writable => {
                    let question =
                        format!("Stop opening {} by default?", Path::new(&kept).display());
                    if Dialog::summon(grimoire, &question)? {
                        self.global.vault = None;
                        if self.global.keep(&self.global_path).is_err() {
                            Warning::save_failed(grimoire)?;
                        }
                    }
                }
                Some(_) => Warning::summon(grimoire, "The global settings are from a newer tdl")?,
                None => Warning::summon(grimoire, "No vault was being opened by default")?,
            }
        }

        if let Some(passed) = &self.passed_vault
            && self.ask_about_vault
            && self.global_writable
            && self.global.vault.as_deref() != Some(passed.as_os_str())
            && Dialog::summon(
                grimoire,
                &format!("Open {} every time tdl starts?", passed.display()),
            )?
        {
            self.global.vault = Some(passed.clone().into_os_string());
            if self.global.keep(&self.global_path).is_err() {
                Warning::save_failed(grimoire)?;
            }
        }

        let passed: Vec<OsString> = self
            .passed_wads
            .iter()
            .map(|folder| folder.clone().into_os_string())
            .collect();
        let remembered =
            remembered_folders(&self.vault.config.wad_folders, &self.global.wad_folders);
        if passed.is_empty() || self.passed_wads == remembered {
            return Ok(());
        }
        let list = self
            .passed_wads
            .iter()
            .map(|folder| folder.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let question = format!("Keep the wad folders {list}?");
        let options = ["Session", "Global", "Vault"];
        match ThreeWayDialog::summon(grimoire, &question, options)? {
            Some(1) if self.global_writable => {
                self.global.wad_folders = passed;
                if self.global.keep(&self.global_path).is_err() {
                    Warning::save_failed(grimoire)?;
                }
            }
            Some(1) => Warning::summon(grimoire, "The global settings are from a newer tdl")?,
            Some(2) => {
                self.vault.config.wad_folders = passed;
                if self
                    .vault
                    .config
                    .write(&self.vault.config_dir, &self.vault.profiles)
                    .is_err()
                {
                    Warning::save_failed(grimoire)?;
                }
            }
            // 🕯️ just for tonight: nothing is written
            _ => {}
        }
        Ok(())
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
        if self.splash && !self.vault.config.skip_splash {
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
        self.offer_to_keep(grimoire)?;
        picker::set_wad_folders(&self.vault.wad_folders);
        Ok(())
    }

    /// The profile list, each row with its badge, and under it a panel for whatever the cursor is
    /// on: what that profile would launch with, or the global settings on the new-profile row.
    fn render(&mut self, page: &mut Page) {
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
            Line::default(),
            Line::from(Self::HEADER[0]),
            Line::from(Self::HEADER[1]).right_aligned(),
        ])
        .style(theme::HEADER);
        let rows = self
            .vault
            .profiles
            .iter()
            .map(|p| {
                Row::new([
                    Line::default(),
                    Line::from(p.name.as_str()),
                    Line::from(p.wads.len().to_string()).right_aligned(),
                ])
            })
            .chain([Row::new([
                Line::default(),
                Line::styled(Self::NEW_ROW, Style::new().fg(theme::MUTED)),
                Line::default(),
            ])]);

        let (details_width, details_height) = details.size();
        let panel = layout::window(details_width, details_height);
        // the badge, the name and the count, then the header, the profiles and the new-profile row
        let list_width = mark::WIDTH + name_width + Self::WADS_WIDTH + 2 * Self::COLUMN_SPACING;
        let (width, height) = layout::window(list_width, self.vault.profiles.len() as u16 + 2);
        let list = layout::grow(width, height, panel.1, page.area());
        let [list_area, details_area] = layout::stack(&[list, panel], page.area())[..] else {
            return;
        };

        let veil = ritual::conjure_veil(Self::TITLE, Self::BORDER_COLOR);
        let inner = veil.inner(list_area);
        // 🔎 scroll first, so the rows about to be shown are known, then look only at those
        let visible = inner.height.saturating_sub(1) as usize;
        let offset = layout::scroll(self.table_state.offset(), selected.unwrap_or(0), visible);
        *self.table_state.offset_mut() = offset;
        let shown = offset..(offset + visible).min(self.vault.profiles.len());
        for i in shown.clone() {
            self.marks
                .entry(i)
                .or_insert_with(|| Mark::of_profile(&self.vault.profiles[i], &self.vault.config));
        }

        let table = Table::new(
            rows,
            [
                Constraint::Length(mark::WIDTH),
                Constraint::Fill(1),
                Constraint::Length(Self::WADS_WIDTH),
            ],
        )
        .column_spacing(Self::COLUMN_SPACING)
        .header(header)
        .block(veil)
        .row_highlight_style(if is_broken {
            theme::BROKEN_CURSOR
        } else {
            theme::CURSOR
        });
        page.render_stateful_widget(table, list_area, &mut self.table_state);
        // 🩸 the badges go on last, so even the cursor's highlight can't hide them
        for i in shown {
            if let Some(Some(mark)) = self.marks.get(&i) {
                let y = inner.y + 1 + (i - offset) as u16;
                mark.paint(inner.x, y, inner, page.buffer_mut());
            }
        }

        let panel = ritual::conjure_banner(details.title, theme::PANEL, details.title_color);
        let fitting = details.lines_fitting(panel.inner(details_area).width);
        page.render_widget(Paragraph::new(fitting).block(panel), details_area);

        let (sigil, sigil_area) = ritual::conjure_sigil(details_area);
        page.render_widget(sigil, sigil_area);
    }

    fn prompt(
        &mut self,
        grimoire: &mut Grimoire,
        key_code: KeyCode,
    ) -> Result<Phase<Self::Return>> {
        // 🧹 moving the cursor changes nothing on disk; anything else might, so the marks are redone
        if !matches!(
            key_code,
            KeyCode::Char('j' | 'k' | '?') | KeyCode::Up | KeyCode::Down
        ) {
            self.marks.clear();
        }
        let index = self.table_state.selected_mut().as_mut().unwrap();
        match key_code {
            KeyCode::Enter => {
                if *index != self.vault.profiles.len() {
                    let profile = &self.vault.profiles[*index];
                    if let Some(uzdoom) = profile.uzdoom(&self.vault.config) {
                        // 🚧 a profile with a broken part is not fixed for you, but it isn't launched either
                        if !Warning::faults(grimoire, &profile.faults(&self.vault.config))? {
                            let details = [format!("profile {}", profile.name)];
                            let result = uzdoom::launch(
                                uzdoom,
                                profile.base_wad(&self.vault.config).map(Wad::deref),
                                profile,
                                &self.vault.data_dir.join(&profile.name),
                            )
                            .and_then(|game| {
                                running::summon(grimoire, "launching", &details, game)
                            });
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
                        && let Some(settings) =
                            HostSettingsForm::summon(grimoire, &mut self.vault, *index)?
                    {
                        host_game::summon(grimoire, &mut self.vault, *index, settings)?;
                    }
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Prompt)
                }
            }
            KeyCode::Char('n') => {
                if let Some(address) = JoinAddressForm::summon(grimoire)? {
                    join_game::summon(grimoire, &mut self.vault, index, &address)?;
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
                    // 🖼️ redrawn either way, since the hashing window may have covered the launcher
                    if let Some(wad) = hashing::wad(grimoire, &path)? {
                        self.vault.config.base_wad = Some(wad);
                        self.vault.forget_lookup();
                    }
                    return Ok(Phase::Render);
                }
                Ok(Phase::Prompt)
            }
            KeyCode::Char('s') => {
                self.vault.config.skip_splash = !self.vault.config.skip_splash;
                // 💾 saved right away: the splash is the first thing the next start shows
                if self
                    .vault
                    .config
                    .write(&self.vault.config_dir, &self.vault.profiles)
                    .is_err()
                {
                    Warning::save_failed(grimoire)?;
                }
                Ok(Phase::Render)
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

#[cfg(test)]
mod tests {
    use std::fs;

    use insta::assert_snapshot;

    use super::*;
    use crate::{testing, vault::scribe::tests::Altar};

    fn launcher(altar: &Altar, row: usize) -> Launcher {
        let vault = testing::example_vault(altar);
        let start = Start {
            vault: Some(vault.to_string_lossy().into_owned()),
            ..Start::default()
        };
        let mut launcher = Launcher::new(start, &altar.file("standard"), &altar.file("standard"));
        launcher.table_state.select(Some(row));
        launcher
    }

    #[test]
    fn on_a_healthy_profile() {
        let altar = Altar::new("snap-launcher-healthy");
        let mut launcher = launcher(&altar, 0);
        assert_snapshot!(testing::draw(90, 28, |page| launcher.render(page)));
    }

    #[test]
    fn on_a_broken_profile() {
        let altar = Altar::new("snap-launcher-broken");
        let mut launcher = launcher(&altar, 1);
        assert_snapshot!(testing::draw(90, 28, |page| launcher.render(page)));
    }

    #[test]
    fn on_the_new_profile_row() {
        let altar = Altar::new("snap-launcher-new");
        let mut launcher = launcher(&altar, 3);
        assert_snapshot!(testing::draw(90, 28, |page| launcher.render(page)));
    }

    #[test]
    fn on_the_smallest_terminal() {
        let altar = Altar::new("snap-launcher-small");
        let mut launcher = launcher(&altar, 1);
        assert_snapshot!(testing::draw(40, 20, |page| launcher.render(page)));
    }

    // 🚪 what the command line opens, and what is kept from it

    #[test]
    fn kept_folders_are_the_vaults_then_the_global_ones_each_once() {
        let folders = remembered_folders(
            &["/v1".into(), "/both".into()],
            &["/both".into(), "/g1".into()],
        );
        let folders: Vec<_> = folders.iter().map(|f| f.to_str().unwrap()).collect();
        assert_eq!(folders, ["/v1", "/both", "/g1"]);
    }

    #[test]
    fn passed_folders_are_made_absolute_and_replace_the_kept_ones() {
        let altar = Altar::new("launcher-passed-wads");
        let standard = altar.file("standard");
        Global {
            wad_folders: vec!["/kept".into()],
            ..Global::default()
        }
        .keep(&Global::path(&standard))
        .unwrap();

        let kept = Launcher::new(Start::default(), &standard, &standard);
        assert_eq!(kept.vault.wad_folders, [PathBuf::from("/kept")]);

        let start = Start {
            wads: vec!["relative/wads".into()],
            ..Start::default()
        };
        let passed = Launcher::new(start, &standard, &standard);
        let folder = &passed.vault.wad_folders[0];
        assert!(folder.is_absolute());
        assert!(folder.ends_with("relative/wads"));
        assert_eq!(passed.vault.wad_folders.len(), 1);
    }

    #[test]
    fn the_kept_vault_opens_unless_another_is_passed() {
        let altar = Altar::new("launcher-kept-vault");
        let standard = altar.file("standard");
        let kept = altar.file("kept");
        fs::create_dir_all(&kept).unwrap();
        Global {
            vault: Some(kept.clone().into_os_string()),
            ..Global::default()
        }
        .keep(&Global::path(&standard))
        .unwrap();

        let opened = Launcher::new(Start::default(), &standard, &standard);
        assert_eq!(opened.vault.config_dir, kept.join("config"));

        let other = altar.file("other");
        let start = Start {
            vault: Some(other.to_string_lossy().into_owned()),
            ..Start::default()
        };
        let passed = Launcher::new(start, &standard, &standard);
        assert_eq!(passed.vault.config_dir, other.join("config"));
        assert_eq!(passed.passed_vault, Some(other));
    }

    #[test]
    fn a_kept_vault_that_is_gone_falls_back_to_the_standard_one_and_says_so() {
        let altar = Altar::new("launcher-gone-vault");
        let standard = altar.file("standard");
        Global {
            vault: Some(altar.file("gone").into_os_string()),
            ..Global::default()
        }
        .keep(&Global::path(&standard))
        .unwrap();
        let opened = Launcher::new(Start::default(), &standard, &standard);
        assert_eq!(opened.vault.config_dir, standard);
        assert!(
            opened
                .vault
                .notices
                .iter()
                .any(|n| n.contains("is missing"))
        );
    }

    #[test]
    fn forgetting_opens_the_standard_vault_even_with_one_kept() {
        let altar = Altar::new("launcher-forget-vault");
        let standard = altar.file("standard");
        let kept = altar.file("kept");
        fs::create_dir_all(&kept).unwrap();
        Global {
            vault: Some(kept.into_os_string()),
            ..Global::default()
        }
        .keep(&Global::path(&standard))
        .unwrap();
        let start = Start {
            forget_vault: true,
            ..Start::default()
        };
        let opened = Launcher::new(start, &standard, &standard);
        assert_eq!(opened.vault.config_dir, standard);
        // nothing is forgotten yet: that waits for the question
        assert!(opened.global.vault.is_some());
        assert!(opened.vault.notices.is_empty());
    }
}
