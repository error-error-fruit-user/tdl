use std::{
    collections::HashMap,
    ffi::OsString,
    ops::Deref,
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use ratatui::{
    crossterm::event::KeyCode,
    layout::{Constraint, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Paragraph, Row, Table, TableState},
};

use crate::{
    cli::{Ask, Command, Forget, Seal},
    clipboard::Clipboard,
    error::{Error, Result},
    layout, music, picker,
    popups::dialog::Dialog,
    popups::hashing,
    popups::legend::Legend,
    popups::resize,
    popups::running,
    popups::status,
    popups::three_way_dialog::ThreeWayDialog,
    popups::warning::Warning,
    quips,
    ritual::{self, Grimoire, Haunted, Page, Phase, Ritual},
    screens::editor::{self, Editor},
    screens::host_settings::HostSettingsForm,
    screens::join_address::JoinAddressForm,
    screens::library::Library,
    screens::profile_namer::ask_copy_name,
    screens::search::{self, Found, Search, Typed},
    screens::selection::{self, Selection},
    screens::settings::{Settings, SettingsForm},
    screens::splash,
    screens::wrap_around,
    slayer::host_game,
    slayer::join_game,
    slayer::plain,
    slayer::share,
    slayer::watch,
    theme, uzdoom,
    vault::Vault,
    vault::edition::{Edition, Loaded},
    vault::global::{Global, Music, Shared},
    vault::host_settings::HostSettings,
    vault::profile::Profile,
    vault::scribe,
    vault::wad::Wad,
    widgets::{
        details::Details,
        mark::{self, Mark},
        notes,
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
    /// The quip for the splash screen, from the command line, instead of a random one.
    quip: Option<String>,
    /// `--no-quips`: no quips anywhere this run, whatever the settings say.
    no_quips: bool,
    /// `--keep-music`: the music plays on past the splash this run, whatever the settings say.
    keep_music: bool,
    /// `--no-music`: no music at all this run, whatever the settings say.
    no_music: bool,
    /// `--uzdoom` and `--base-wad`, until they are put into the config for the session (after any
    /// upgrade, since an upgrade compares the config with what it saved).
    session_uzdoom: Option<PathBuf>,
    session_base_wad: Option<PathBuf>,
    /// The command from the command line, until it is done.
    command: Option<Command>,
    /// Each profile's mark, by position. A profile is only checked once its row is on screen, and
    /// the marks are forgotten whenever something could have changed them.
    marks: HashMap<usize, Option<Mark>>,
    /// What was passed on the command line, made absolute, for the startup questions about
    /// keeping it.
    passed_vault: Option<PathBuf>,
    ask_about_vault: bool,
    forget_vault: Forget,
    passed_wads: Vec<PathBuf>,
    ask_about_wads: bool,
    forget_wads: Forget,
    /// The scroll from `--unseal`, until it has been imported.
    passed_scroll: Option<PathBuf>,
    /// Whether the scroll replaces the vault's profiles (`--unseal-delete`), and asks first.
    unseal_delete: Option<Ask>,
    /// Whether the global settings took any from an older `config.rite`, to be saved before the
    /// upgrade writes it without them.
    adopted: bool,
    /// The search being typed, if one is.
    search: Option<Search>,
    /// The last search's matches, for `;` and `,` to step through.
    found: Found,
    /// The profiles picked to be copied together.
    selection: Selection,
}

/// What the command line asked the launcher for.
#[derive(Default)]
pub struct Start {
    pub vault: Option<String>,
    /// Whether to ask about opening the passed vault every time; off for `--vault-once`.
    pub ask_about_vault: bool,
    /// Opens the standard vault, and maybe offers to forget the remembered one.
    pub forget_vault: Forget,
    pub wads: Vec<String>,
    /// Whether to ask about keeping the passed wad dirs; off for `--wads-once`.
    pub ask_about_wads: bool,
    /// Uses no wad dirs but the downloads, and maybe offers to forget the kept ones.
    pub forget_wads: Forget,
    /// A scroll to import as soon as the launcher is up.
    pub unseal: Option<String>,
    /// The scroll replaces every profile in the vault, asking first or not.
    pub unseal_delete: Option<Ask>,
    pub splash: bool,
    /// The splash screen's quip, instead of a random one. Plays the splash even if the settings
    /// skip it, since asking for a quip is asking to see it.
    pub quip: Option<String>,
    /// Shows no quips anywhere for this run.
    pub no_quips: bool,
    /// Keeps the music playing past the splash for this run.
    pub keep_music: bool,
    /// Plays no music at all for this run.
    pub no_music: bool,
    /// The UZDoom to play with for this run only, over every profile's own and the global one.
    pub uzdoom: Option<PathBuf>,
    /// The base wad to play with for this run only, the same way. Hashed once the launcher is up.
    pub base_wad: Option<PathBuf>,
    /// `run`, `host` or `join`: done without the launcher, after which tdl closes.
    pub command: Option<Command>,
}

/// `path` as an absolute path, so a dir kept from `tdl --wads .` still means the same dir
/// when tdl is started from somewhere else.
fn absolute(path: &str) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| PathBuf::from(path))
}

/// The wad dirs to use when none are passed: the vault's own, then the global ones, each once.
fn remembered_dirs(vault: &[OsString], global: &[OsString]) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for dir in vault.iter().chain(global) {
        let dir = PathBuf::from(dir);
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    dirs
}

impl Launcher {
    const TITLE: &str = "launcher";
    fn border_color() -> Color {
        theme::worn().colors.screen
    }
    const HEADER: [&str; 2] = ["profile", "wads"];
    /// The last row, which makes a new profile.
    const NEW_ROW: &str = "+ new profile";
    /// The wads column: a count.
    const WADS_WIDTH: u16 = 4;
    /// The columns between the profile's panel and the global one beside it.
    const PANEL_GAP: u16 = 1;
    const COLUMN_SPACING: u16 = 1;
    const LEGEND: &[[&str; 2]] = &[
        ["Enter", "Launch"],
        ["H", "Host"],
        ["N", "Join"],
        ["J↓/K↑/Home/End", "Traverse"],
        Search::LEGEND,
        ["}/{", "Swap (Selected, If Any)"],
        Selection::LEGEND,
        ["Y/Shift+Y", "Copy Selected/All Profiles"],
        ["X", "Cut (Selected, If Any)"],
        ["P", "Paste"],
        ["A/E/D", "Add/Edit/Delete (Selected, If Any)"],
        ["F", "Add a Profile That Watches a Dir"],
        ["S/I", "Seal (Selected or All)/Import Scroll"],
        ["U/W", "Select UZDoom/Base Wad"],
        ["L", "Wads (Every Dir)"],
        ["O", "Settings"],
        ["Esc", "Save and Quit (unselects first)"],
    ];

    /// Opens the vault: `<vault>/{config,data}` when one is passed, the standard dirs when
    /// asked to forget the kept one, otherwise the one kept in the global settings if it still
    /// exists, otherwise the standard dirs (`standard_config_dir`
    /// and `standard_data_dir`, which also hold `global.rite`). The wad dirs are the passed
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
            if start.forget_vault.is_on() {
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
        let passed_wads: Vec<_> = start.wads.iter().map(|dir| absolute(dir)).collect();
        vault.wad_dirs = if !passed_wads.is_empty() {
            passed_wads.clone()
        } else if start.forget_wads.is_on() {
            Vec::new()
        } else {
            remembered_dirs(&vault.config.wad_dirs, &global.wad_dirs)
        };
        vault.global = global;
        vault.global_path = global_path;
        vault.global_writable = global_writable;
        // 🌐 settings a vault kept before they all went global, saved once the upgrade is agreed to
        let adopted = vault.global.adopt(&mut vault.config);
        Self {
            vault,
            table_state: TableState::new().with_selected(0),
            splash: start.splash,
            quip: start.quip,
            no_quips: start.no_quips,
            keep_music: start.keep_music,
            no_music: start.no_music,
            session_uzdoom: start.uzdoom,
            session_base_wad: start.base_wad,
            command: start.command,
            marks: HashMap::new(),
            passed_vault,
            ask_about_vault: start.ask_about_vault,
            forget_vault: start.forget_vault,
            passed_wads,
            ask_about_wads: start.ask_about_wads,
            forget_wads: start.forget_wads,
            passed_scroll: start.unseal.as_deref().map(absolute),
            unseal_delete: start.unseal_delete,
            adopted,
            search: None,
            found: Found::default(),
            selection: Selection::default(),
        }
    }

    /// Builds the launcher and runs it until the user quits, after the splash screen if asked.
    ///
    /// With a command (`run`, `host`, `join`) it does that instead, with everything set up the same
    /// way first, and then returns, so tdl closes: the launcher itself is never shown.
    pub fn summon(grimoire: &mut Grimoire, start: Start) -> Result<()> {
        let project_dirs = ProjectDirs::from("error.error.fruit", "eef", "tdl")
            .expect("Could not find home directory!");
        let mut launcher = Self::new(start, project_dirs.config_dir(), project_dirs.data_dir());
        match launcher.command.take() {
            Some(command) => {
                launcher.prepare(grimoire)?;
                launcher.perform(grimoire, command)
            }
            None => launcher.invoke(grimoire),
        }
    }

    /// Everything the launcher does as it opens, which a command needs just as much: the resize
    /// notice, the music and the splash, files saved by other editions (newer ones end the session
    /// untouched, older ones are upgraded if the user agrees, and the originals deleted only if
    /// they agree to that too), notices, the questions about what was passed on the command line,
    /// this session's UZDoom and base wad, and a scroll from `--unseal`.
    fn prepare(&mut self, grimoire: &mut Grimoire) -> Result<()> {
        let area = grimoire.consult().area();

        if layout::is_too_small(area.width, area.height) {
            resize::summon(grimoire)?;
        }
        quips::show(!self.no_quips && !self.vault.global.no_quips);
        // 🔇 nothing plays at all, not even on the splash
        if self.no_music || self.vault.global.music == Music::Off {
            music::mute();
        }
        // 🎶 the song from the settings, unless `--music` already chose one; a song that has gone
        // missing is said, and the built-in track plays instead
        if !music::has_song()
            && !self.no_music
            && self.vault.global.music != Music::Off
            && let Some(song) = &self.vault.global.song
            && let Err(problem) = music::use_song(Path::new(song))
        {
            Warning::summon(grimoire, &problem)?;
        }
        // 🎵 music kept on plays from the start, through the splash and past it
        if self.keep_music || self.vault.global.music == Music::Kept {
            music::start(true);
        }
        // 🏳️‍🌈 the pride splash, kept on in the settings (`--with-pride` already wore it for this run)
        if self.vault.global.pride {
            theme::wear_pride();
        }
        if self.splash && (!self.vault.global.skip_splash || self.quip.is_some()) {
            // 💬 the command line's quip, or else the one kept in the settings, or else a random one
            let kept = quips::shown()
                .then(|| self.vault.global.splash_quip.text())
                .flatten();
            let quip = self.quip.clone().or(kept);
            splash::summon(grimoire, quip.as_deref())?;
        }
        if self.vault.is_too_new() {
            Warning::files_too_new(grimoire)?;
            return Err(Error::Interrupted);
        }
        // 📜 anything that was fished out of the wreckage on the way in
        for notice in std::mem::take(&mut self.vault.notices) {
            Warning::summon(grimoire, &notice)?;
        }
        if self.vault.is_outdated() {
            if !Dialog::should_upgrade(grimoire)? {
                return Err(Error::Interrupted);
            }
            // 🌐 the vault's old settings are kept globally first, or the upgrade would lose them
            if self.adopted && self.vault.keep_global().is_err() {
                Warning::upgrade_failed(grimoire)?;
                return Err(Error::Interrupted);
            }
            // ⏳ checking every page takes a moment, so don't leave the old dialog hanging there
            status::summon(grimoire, "Upgrading", "Upgrading...", Self::border_color())?;
            let Ok(upgraded) = self.vault.upgrade() else {
                Warning::upgrade_failed(grimoire)?;
                return Err(Error::Interrupted);
            };
            if Dialog::should_delete_originals(grimoire)? {
                upgraded.iter().for_each(|path| scribe::retire(path));
            }
        }
        self.offer_to_keep(grimoire)?;
        // 🧪 this run's UZDoom and base wad, used over every other and never saved
        if let Some(uzdoom) = self.session_uzdoom.take() {
            self.vault.global.session_uzdoom = Some(uzdoom.into_os_string());
        }
        if let Some(path) = self.session_base_wad.take() {
            match hashing::wad(grimoire, &path)? {
                Some(wad) => {
                    self.vault.global.session_base_wad = Some(wad);
                    self.vault.forget_lookup();
                }
                None => Warning::summon(
                    grimoire,
                    "The base wad from --base-wad wasn't read, so the usual ones are used",
                )?,
            }
        }
        picker::set_wad_dirs(&self.vault.wad_dirs);
        if let Some(scroll) = self.passed_scroll.take() {
            // 📜 the new profile goes last, and the cursor lands on it
            let before = self.vault.profiles.len();
            let mut index = before;
            let current = self.vault.global.shared();
            let replacing = self.unseal_delete;
            if let Some(settings) = share::unseal_file(
                grimoire,
                &mut self.vault,
                &mut index,
                &scroll,
                &current,
                replacing,
            )? {
                self.take_shared(grimoire, settings)?;
            }
            if replacing.is_some() {
                self.table_state.select(Some(0));
            } else if self.vault.profiles.len() > before {
                self.table_state.select(Some(before));
            }
        }
        // 👀 the dirs profiles watch may have had wads put in or taken out since last time
        watch::refresh_all(grimoire, &mut self.vault)?;
        Ok(())
    }

    /// `tdl host --headless` and `tdl join --headless`: set up like [`Launcher::prepare`], but with no
    /// windows at all. What would be a notice is a line on stderr, and anything that would need a
    /// question (a file to upgrade) is an error instead; then the command runs (see
    /// [`plain`](crate::slayer::plain)). `Err` says what stopped it.
    pub fn summon_plain(start: Start) -> std::result::Result<(), String> {
        let mut launcher = Self::open_plain(start)?;
        let command = launcher
            .command
            .take()
            .ok_or("--headless needs launch, host or join")?;
        let vault = &mut launcher.vault;
        vault.wad_dirs.retain(|dir| dir.is_dir());
        if let Some(uzdoom) = launcher.session_uzdoom.take() {
            vault.global.session_uzdoom = Some(uzdoom.into_os_string());
        }
        if let Some(path) = launcher.session_base_wad.take() {
            let wad = Wad::hashed(&path, &mut ())
                .map_err(|error| format!("Could not read --base-wad: {error}"))?;
            vault.global.session_base_wad = Some(wad);
            vault.forget_lookup();
        }
        match command {
            Command::Host { profile, overrides } => {
                let index = find_profile(&vault.profiles, &profile)
                    .ok_or(format!("There is no profile called '{profile}'"))?;
                watch::refresh_plain(vault, index)?;
                plain::host(vault, index, &overrides)
            }
            Command::Join {
                host,
                port,
                code,
                always_download,
                host_ini,
            } => {
                let address = Command::address(&host, port, code.as_deref())
                    .ok_or("join --headless needs the host's join code: --code <code>")?;
                plain::join(vault, &address, always_download, host_ini)
            }
            Command::Launch { profile } => {
                let index = find_profile(&vault.profiles, &profile)
                    .ok_or(format!("There is no profile called '{profile}'"))?;
                watch::refresh_plain(vault, index)?;
                plain::launch(vault, index)
            }
        }
    }

    /// The launcher for a run with no windows, with the vault checked as fine to use: notices are
    /// printed, and files from another edition of tdl are an error.
    fn open_plain(start: Start) -> std::result::Result<Self, String> {
        let project_dirs = ProjectDirs::from("error.error.fruit", "eef", "tdl")
            .ok_or("Could not find the home directory")?;
        let mut launcher = Self::new(start, project_dirs.config_dir(), project_dirs.data_dir());
        let vault = &mut launcher.vault;
        if vault.is_too_new() {
            return Err("Your files are from a newer tdl, please update".to_string());
        }
        for notice in std::mem::take(&mut vault.notices) {
            eprintln!("{notice}");
        }
        if vault.is_outdated() {
            return Err(
                "Your files are from an older tdl: open tdl once to upgrade them, then try again"
                    .to_string(),
            );
        }
        Ok(launcher)
    }

    /// `tdl --seal`: seals the whole vault into a scroll, with the global settings only if
    /// `--with-settings` asks for them, with no windows.
    pub fn seal_plain(start: Start, seal: &Seal) -> std::result::Result<(), String> {
        let launcher = Self::open_plain(start)?;
        let settings = seal.with_settings.then(|| launcher.vault.global.shared());
        plain::seal(&launcher.vault, settings, &seal.scroll)?;
        Ok(())
    }

    /// Does a command from the command line. Whatever happens (the game ends, it couldn't start,
    /// the user backs out of a question), it returns, and tdl closes.
    fn perform(&mut self, grimoire: &mut Grimoire, command: Command) -> Result<()> {
        match command {
            Command::Launch { profile } => match self.find(grimoire, &profile)? {
                Some(index) => self.launch_profile(grimoire, index),
                None => Ok(()),
            },
            Command::Host { profile, overrides } => match self.find(grimoire, &profile)? {
                Some(index) => {
                    let defaults = self.vault.profiles[index]
                        .host_defaults
                        .clone()
                        .unwrap_or_else(|| HostSettings::new(self.vault.global.host_port));
                    self.host_profile(grimoire, index, Some(overrides.apply(defaults)))
                }
                None => Ok(()),
            },
            Command::Join {
                host,
                port,
                code,
                always_download,
                host_ini,
            } => {
                let Some(joining) =
                    JoinAddressForm::summon_with(grimoire, &host, port, code.as_deref(), host_ini)?
                else {
                    return Ok(());
                };
                let mut index = self.vault.profiles.len();
                join_game::summon(
                    grimoire,
                    &mut self.vault,
                    &mut index,
                    &joining.address,
                    always_download,
                    joining.host_ini,
                )
            }
        }
    }

    /// The profile called `name`: that exact name, or else the only one that is it but for case.
    /// Says so and gives `None` if there is no such profile.
    fn find(&self, grimoire: &mut Grimoire, name: &str) -> Result<Option<usize>> {
        let found = find_profile(&self.vault.profiles, name);
        if found.is_none() {
            Warning::summon(grimoire, &format!("There is no profile called '{name}'"))?;
        }
        Ok(found)
    }

    /// Plays the profile at `index`, unless it has no UZDoom or something it needs is broken, which
    /// is said instead.
    fn launch_profile(&mut self, grimoire: &mut Grimoire, index: usize) -> Result<()> {
        watch::refresh_saved(grimoire, &mut self.vault, index)?;
        let profile = &self.vault.profiles[index];
        let Some(uzdoom) = profile.uzdoom(&self.vault.global) else {
            return Warning::uzdoom_not_selected(grimoire);
        };
        // 🚧 a profile with a broken part is not fixed for you, but it isn't launched either
        if Warning::faults(grimoire, &profile.faults(&self.vault.global))? {
            return Ok(());
        }
        let details = [format!("profile {}", profile.name)];
        let result = uzdoom::launch(
            uzdoom,
            profile.base_wad(&self.vault.global).map(Wad::deref),
            profile,
            profile.uzdoom_args(&self.vault.global),
            &self.vault.data_dir.join(&profile.name),
        )
        .and_then(|game| running::summon(grimoire, "launching", &details, game));
        Warning::report_spawn(grimoire, result)
    }

    /// Hosts the profile at `index` with what the host form is left with, the form starting from
    /// `start` if given (`tdl host` with its flags), else from the profile's defaults; unless the
    /// profile has no UZDoom or something it needs is broken.
    fn host_profile(
        &mut self,
        grimoire: &mut Grimoire,
        index: usize,
        start: Option<HostSettings>,
    ) -> Result<()> {
        watch::refresh_saved(grimoire, &mut self.vault, index)?;
        let profile = &self.vault.profiles[index];
        if profile.uzdoom(&self.vault.global).is_none() {
            return Warning::uzdoom_not_selected(grimoire);
        }
        if Warning::faults(grimoire, &profile.faults(&self.vault.global))? {
            return Ok(());
        }
        let Some(settings) = HostSettingsForm::summon(grimoire, &mut self.vault, index, start)?
        else {
            return Ok(());
        };
        host_game::summon(grimoire, &mut self.vault, index, settings)
    }

    /// The startup questions about what was passed on the command line: whether to open the
    /// passed vault every time, and whether to keep the passed wad dirs for this session,
    /// globally, or for this vault. Dirs that don't exist are dropped first, with a warning.
    fn offer_to_keep(&mut self, grimoire: &mut Grimoire) -> Result<()> {
        let missing: Vec<_> = self
            .passed_wads
            .iter()
            .filter(|dir| !dir.is_dir())
            .map(|dir| dir.display().to_string())
            .collect();
        if !missing.is_empty() {
            Warning::summon(
                grimoire,
                &format!("Not a dir, so left out: {}", missing.join(", ")),
            )?;
            self.passed_wads.retain(|dir| dir.is_dir());
            // 📂 none of them left: the kept dirs are better than none
            self.vault.wad_dirs = if self.passed_wads.is_empty() {
                remembered_dirs(&self.vault.config.wad_dirs, &self.vault.global.wad_dirs)
            } else {
                self.passed_wads.clone()
            };
        }

        if self.forget_vault == Forget::Asking {
            match self.vault.global.vault.clone() {
                Some(kept) if self.vault.global_writable => {
                    let question =
                        format!("Stop opening {} by default?", Path::new(&kept).display());
                    if Dialog::summon(grimoire, &question)? {
                        self.vault.global.vault = None;
                        if self.vault.global.keep(&self.vault.global_path).is_err() {
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
            && self.vault.global_writable
            && self.vault.global.vault.as_deref() != Some(passed.as_os_str())
            && Dialog::summon(
                grimoire,
                &format!("Open {} every time tdl starts?", passed.display()),
            )?
        {
            self.vault.global.vault = Some(passed.clone().into_os_string());
            if self.vault.global.keep(&self.vault.global_path).is_err() {
                Warning::save_failed(grimoire)?;
            }
        }

        if self.forget_wads == Forget::Asking {
            self.offer_to_forget_wads(grimoire)?;
        }

        if !self.ask_about_wads {
            return Ok(());
        }
        let passed: Vec<OsString> = self
            .passed_wads
            .iter()
            .map(|dir| dir.clone().into_os_string())
            .collect();
        let remembered = remembered_dirs(&self.vault.config.wad_dirs, &self.vault.global.wad_dirs);
        if passed.is_empty() || self.passed_wads == remembered {
            return Ok(());
        }
        let list = self
            .passed_wads
            .iter()
            .map(|dir| dir.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let question = format!("Keep the wad dirs {list}?");
        let options = ["Session", "Global", "Vault"];
        match ThreeWayDialog::summon(grimoire, &question, options)? {
            Some(1) if self.vault.global_writable => {
                self.vault.global.wad_dirs = passed;
                if self.vault.global.keep(&self.vault.global_path).is_err() {
                    Warning::save_failed(grimoire)?;
                }
            }
            Some(1) => Warning::summon(grimoire, "The global settings are from a newer tdl")?,
            Some(2) => {
                self.vault.config.wad_dirs = passed;
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

    /// `--forget-wads`: asks whether to forget the wad dirs kept for every vault, for this one, or
    /// both, and forgets them for good if so.
    fn offer_to_forget_wads(&mut self, grimoire: &mut Grimoire) -> Result<()> {
        let list = |dirs: &[OsString]| {
            dirs.iter()
                .map(|dir| Path::new(dir).display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let (global, vault) = match (
            self.vault.global.wad_dirs.is_empty(),
            self.vault.config.wad_dirs.is_empty(),
        ) {
            (true, true) => return Warning::summon(grimoire, "No wad dirs were kept"),
            (false, true) => {
                let question = format!(
                    "Forget the wad dirs kept for every vault, {}?",
                    list(&self.vault.global.wad_dirs)
                );
                (Dialog::summon(grimoire, &question)?, false)
            }
            (true, false) => {
                let question = format!(
                    "Forget the wad dirs kept for this vault, {}?",
                    list(&self.vault.config.wad_dirs)
                );
                (false, Dialog::summon(grimoire, &question)?)
            }
            (false, false) => {
                let question = format!(
                    "Forget the wad dirs kept for every vault ({}), for this vault ({}), or both?",
                    list(&self.vault.global.wad_dirs),
                    list(&self.vault.config.wad_dirs)
                );
                match ThreeWayDialog::summon(grimoire, &question, ["Global", "Vault", "Both"])? {
                    Some(0) => (true, false),
                    Some(1) => (false, true),
                    Some(2) => (true, true),
                    _ => (false, false),
                }
            }
        };
        if global && !self.vault.global_writable {
            Warning::summon(grimoire, "The global settings are from a newer tdl")?;
        } else if global {
            self.vault.global.wad_dirs.clear();
            if self.vault.global.keep(&self.vault.global_path).is_err() {
                Warning::save_failed(grimoire)?;
            }
        }
        if vault {
            self.vault.config.wad_dirs.clear();
            if self
                .vault
                .config
                .write(&self.vault.config_dir, &self.vault.profiles)
                .is_err()
            {
                Warning::save_failed(grimoire)?;
            }
        }
        Ok(())
    }

    /// Opens the settings, and saves whatever was changed straight away in `global.rite`, since
    /// the splash is the first thing the next start shows. Settings from a newer tdl can't be
    /// changed at all.
    fn change_settings(&mut self, grimoire: &mut Grimoire) -> Result<()> {
        let before = Settings::of(&self.vault.global);
        let Some(settings) = SettingsForm::summon(grimoire, before.clone())? else {
            return Ok(());
        };
        if settings == before {
            return Ok(());
        }
        if !self.vault.global_writable {
            return Warning::summon(
                grimoire,
                "The global settings are from a newer tdl, so they can't be changed",
            );
        }
        let kept_song = self.vault.global.song.clone();
        let kept_music = self.vault.global.music;
        if let Err(problem) = settings.apply(&mut self.vault.global) {
            return Warning::summon(grimoire, &problem);
        }
        // 🎶 the song and the music change now
        if self.vault.global.song != kept_song {
            match &self.vault.global.song {
                Some(song) if music::use_song(Path::new(song)).is_ok() => {}
                _ => music::forget_song(),
            }
            music::restart();
        }
        self.follow_music(kept_music, self.vault.global.music);
        // 💬 the quips come and go now, unless `--no-quips` said otherwise for this run
        quips::show(!self.no_quips && !self.vault.global.no_quips);
        if self.vault.global.keep(&self.vault.global_path).is_err() {
            Warning::save_failed(grimoire)?;
        }
        Ok(())
    }

    /// Changes the music now, from `before` to `after`. The splash is over by now, so **splash
    /// only** means quiet, and **keeps playing** starts it again if it had stopped. `--no-music`
    /// keeps it quiet whatever the settings say.
    fn follow_music(&self, before: Music, after: Music) {
        if before == after || self.no_music {
            return;
        }
        match after {
            Music::Off => music::mute(),
            Music::Splash => {
                music::unmute();
                music::stop();
            }
            Music::Kept => {
                music::unmute();
                music::start(true);
            }
        }
    }

    /// Keeps the global settings that came with a scroll, and puts them to work straight away.
    fn take_shared(&mut self, grimoire: &mut Grimoire, settings: Shared) -> Result<()> {
        if !self.vault.global_writable {
            return Warning::summon(
                grimoire,
                "The global settings are from a newer tdl, so the scroll's can't be kept",
            );
        }
        let before = self.vault.global.music;
        self.vault.global.take_shared(settings);
        self.follow_music(before, self.vault.global.music);
        quips::show(!self.no_quips && !self.vault.global.no_quips);
        if self.vault.global.keep(&self.vault.global_path).is_err() {
            Warning::save_failed(grimoire)?;
        }
        Ok(())
    }

    /// Moves every selected profile one place down, or up, keeping them selected; nothing moves if
    /// one is already at the end. The cursor goes along if it is on one of them.
    fn shift_selection(&mut self, down: bool) -> Phase<()> {
        let (cursor, count) = (self.cursor(), self.vault.profiles.len());
        let rows = self.selection.rows(cursor, count);
        let Some(moved) = selection::shift(&mut self.vault.profiles, &rows, down) else {
            return Phase::Prompt;
        };
        if rows.contains(&cursor) {
            self.table_state
                .select(Some(if down { cursor + 1 } else { cursor - 1 }));
        }
        self.selection.select_rows(moved);
        self.marks.clear();
        self.found = Found::default();
        Phase::Render
    }

    /// Deletes every selected profile, and its saves, after asking once.
    fn delete_selection(&mut self, grimoire: &mut Grimoire) -> Result<()> {
        let rows = self
            .selection
            .rows(self.cursor(), self.vault.profiles.len());
        let question = match rows.len() {
            1 => format!(
                "Are you sure you want to delete '{}'?",
                self.vault.profiles[rows[0]].name
            ),
            n => format!(
                "Are you sure you want to delete the {n} selected profiles, and their saves?"
            ),
        };
        if !Dialog::summon(grimoire, &question)? {
            return Ok(());
        }
        self.selection.clear();
        self.marks.clear();
        self.found = Found::default();
        // 🪦 from the bottom up, so the rows still to go keep their places
        for &row in rows.iter().rev() {
            let mut at = row;
            if self.vault.remove(&mut at).is_err() {
                Warning::save_failed(grimoire)?;
                break;
            }
        }
        let first = rows.first().copied().unwrap_or(0);
        self.table_state
            .select(Some(first.min(self.vault.profiles.len())));
        Ok(())
    }

    /// Seals profiles into a scroll: the one under the cursor or the ones selected, or the whole
    /// vault, as the user picks. On the new-profile row with nothing selected, it offers the whole
    /// vault alone.
    fn seal_profiles(&mut self, grimoire: &mut Grimoire) -> Result<Phase<()>> {
        let count = self.vault.profiles.len();
        if count == 0 {
            return Ok(Phase::Prompt);
        }
        let picked = self.selection.rows(self.cursor(), count);
        let all: Vec<usize> = (0..count).collect();
        let whole = format!("the whole vault ({count} profiles)");
        let rows = if picked.is_empty() {
            if !Dialog::summon(grimoire, &format!("Seal {whole}?"))? {
                return Ok(Phase::Render);
            }
            all
        } else if picked.len() == count {
            all
        } else {
            let (option, these) = match picked.as_slice() {
                [row] => (
                    "This profile",
                    format!("'{}'", self.vault.profiles[*row].name),
                ),
                _ => (
                    "Selected",
                    format!("the {} selected profiles", picked.len()),
                ),
            };
            let question = format!("Seal {these}, or {whole}?");
            match ThreeWayDialog::summon(grimoire, &question, [option, "Whole vault", "Back"])? {
                Some(0) => picked,
                Some(1) => all,
                _ => return Ok(Phase::Render),
            }
        };
        let name = match rows.as_slice() {
            [row] => self.vault.profiles[*row].name.clone(),
            _ if rows.len() == count => "tdl_vault".to_string(),
            _ => "profiles".to_string(),
        };
        self.selection.clear();
        share::seal(
            grimoire,
            &self.vault,
            &rows,
            &name,
            self.vault.global.shared(),
        )?;
        Ok(Phase::Render)
    }

    /// The row the cursor is on.
    fn cursor(&self) -> usize {
        self.table_state.selected().unwrap_or(0)
    }

    /// Takes a key while a search is being typed. Only Enter moves the cursor: to the best match.
    fn search_key(&mut self, key_code: KeyCode) -> Phase<()> {
        let Some(search) = &mut self.search else {
            return Phase::Prompt;
        };
        match search.key(key_code) {
            Typed::Typing => Phase::Render,
            Typed::Ignored => Phase::Prompt,
            Typed::Cancelled => {
                self.search = None;
                Phase::Render
            }
            Typed::Done(query) => {
                self.search = None;
                let profiles = &self.vault.profiles;
                let ranked = search::rank(self.cursor(), profiles.len(), |row| {
                    profile_score(&profiles[row], &query)
                });
                self.found = Found::new(ranked);
                if let Some(row) = self.found.current() {
                    self.table_state.select(Some(row));
                }
                Phase::Render
            }
        }
    }

    /// Moves the cursor to the next of the last search's matches, or with `back` the one before.
    fn search_again(&mut self, back: bool) -> bool {
        let row = if back {
            self.found.previous()
        } else {
            self.found.next()
        };
        row.is_some_and(|row| {
            self.table_state.select(Some(row));
            true
        })
    }

    /// Cuts the picked profiles, or the one under the cursor: onto the clipboard, to be moved to
    /// wherever they are pasted. Nothing moves or is deleted until then. Returns whether there was
    /// anything to cut.
    fn cut_profiles(&mut self) -> bool {
        if !self.copy_profiles(false) {
            return false;
        }
        if let Clipboard::Profiles(cut) = std::mem::take(&mut self.vault.clipboard) {
            self.vault.clipboard = Clipboard::CutProfiles(cut);
        }
        true
    }

    /// Pastes cut profiles by moving them, in the order they were cut, to just in front of the
    /// cursor, which lands on the first. Ones renamed or deleted since are left out. A cut pastes
    /// once, so the clipboard is emptied. Like the other moves, the new order is saved on quit.
    fn move_profiles(&mut self, cut: &[Profile]) {
        self.vault.clipboard = Clipboard::Empty;
        let rows: Vec<usize> = cut
            .iter()
            .filter_map(|cut| self.vault.profiles.iter().position(|p| p.name == cut.name))
            .collect();
        if rows.is_empty() {
            return;
        }
        let cursor = self.cursor();
        let at = cursor - rows.iter().filter(|&&row| row < cursor).count();
        let moved: Vec<Profile> = rows
            .iter()
            .map(|&row| self.vault.profiles[row].clone())
            .collect();
        let mut row = 0;
        self.vault.profiles.retain(|_| {
            row += 1;
            !rows.contains(&(row - 1))
        });
        self.vault.profiles.splice(at..at, moved);
        self.table_state.select(Some(at));
    }

    /// Copies the picked profiles (see [`Selection`]), or with `all` every profile, onto the
    /// clipboard, and lets go of the selection. Returns whether there was anything to copy.
    fn copy_profiles(&mut self, all: bool) -> bool {
        let count = self.vault.profiles.len();
        let rows = if all {
            (0..count).collect()
        } else {
            self.selection.rows(self.cursor(), count)
        };
        self.selection.clear();
        if rows.is_empty() {
            return false;
        }
        let copied = rows
            .iter()
            .map(|&row| self.vault.profiles[row].clone())
            .collect();
        self.vault.clipboard = Clipboard::Profiles(copied);
        true
    }

    /// Pastes copies of `profiles` in front of the cursor (see [`Vault::insert_copies`]), asking about
    /// each name that is taken: Tag, Name or Skip. The cursor lands on the first copy.
    fn paste_profiles(&mut self, grimoire: &mut Grimoire, profiles: Vec<Profile>) -> Result<()> {
        let mut at = self.cursor();
        let pasted = self
            .vault
            .insert_copies(&mut at, profiles, |profiles, name, tagged| {
                ask_copy_name(grimoire, profiles, name, tagged)
            })?;
        if pasted.failed {
            Warning::save_failed(grimoire)?;
        }
        if let Some(first) = pasted.first {
            self.table_state.select(Some(first));
        }
        Ok(())
    }

    /// Adds the copied `wads` to the end of the profile under the cursor, after asking, or from the
    /// new-profile row starts a new profile with them.
    fn paste_wads(&mut self, grimoire: &mut Grimoire, wads: Vec<Wad>) -> Result<()> {
        let mut index = self.cursor();
        if index == self.vault.profiles.len() {
            Editor::summon_with(grimoire, &mut self.vault, &mut index, wads)?;
            self.table_state.select(Some(index));
            return Ok(());
        }
        let mut profile = self.vault.profiles[index].clone();
        let copied = match wads.len() {
            1 => "the copied wad".to_string(),
            n => format!("the {n} copied wads"),
        };
        let question = format!("Add {copied} to the end of '{}'?", profile.name);
        if Dialog::summon(grimoire, &question)? {
            let wads = editor::paste_asking(grimoire, &profile.wads, wads)?;
            if wads.is_empty() {
                return Ok(());
            }
            profile.wads.extend(wads);
            if self.vault.replace(index, profile).is_err() {
                Warning::save_failed(grimoire)?;
            }
        }
        Ok(())
    }
}

/// The position of the profile called `name`: that exact name, or else the only one that matches
/// it but for case (`Brutal_Doom` for `brutal_doom`), or `None`.
fn find_profile(profiles: &[Profile], name: &str) -> Option<usize> {
    if let Some(exact) = profiles.iter().position(|p| p.name == name) {
        return Some(exact);
    }
    let mut alike = profiles
        .iter()
        .enumerate()
        .filter(|(_, p)| p.name.eq_ignore_ascii_case(name));
    match (alike.next(), alike.next()) {
        (Some((index, _)), None) => Some(index),
        _ => None,
    }
}

/// How well the profile matches `query`: by its name, or with a `*` in front by the best of its
/// wads' file names.
fn profile_score(profile: &Profile, query: &str) -> Option<search::Score> {
    match query.strip_prefix('*') {
        Some(wad_query) => profile
            .wads
            .iter()
            .filter_map(|wad| search::score(&wad.file_name(), wad_query))
            .min(),
        None => search::score(&profile.name, query),
    }
}

impl Ritual for Launcher {
    type Return = ();

    /// Gets everything ready before the launcher is first drawn (see [`Launcher::prepare`]).
    fn attach(&mut self, grimoire: &mut Grimoire) -> Result<()> {
        self.prepare(grimoire)
    }

    /// The profile list, each row with its badge, and under it a panel for whatever the cursor is
    /// on: what that profile would launch with, with the global settings beside it while the
    /// terminal is wide enough, or the global settings alone on the new-profile row.
    fn render(&mut self, page: &mut Page) {
        let selected = self.table_state.selected();
        let profile = selected.and_then(|i| self.vault.profiles.get(i));
        let global = Details::global(&self.vault.global);
        let global_panel = layout::window(global.size().0, global.size().1);
        let (details, beside) = match profile {
            Some(profile) => (
                Details::launch(
                    profile,
                    &self.vault.global,
                    &profile.faults(&self.vault.global),
                ),
                Some(global),
            ),
            None => (global, None),
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
        .style(theme::worn().styles.header);
        let (details_width, details_height) = details.size();
        let own_panel = layout::window(details_width, details_height);
        // 🌐 the global panel goes to the right of the profile's, if both fit across the terminal
        let room = page.area().width.saturating_sub(2 * layout::MARGIN);
        let beside = beside.filter(|_| own_panel.0 + Self::PANEL_GAP + global_panel.0 <= room);
        let panel = match beside {
            Some(_) => (
                own_panel.0 + Self::PANEL_GAP + global_panel.0,
                own_panel.1.max(global_panel.1),
            ),
            None => own_panel,
        };
        // the badge, the name and the count, then the header, the profiles and the new-profile row
        let list_width = mark::WIDTH + name_width + Self::WADS_WIDTH + 2 * Self::COLUMN_SPACING;
        let (width, height) = layout::window(list_width, self.vault.profiles.len() as u16 + 2);
        let list = layout::grow(width, height, panel.1, page.area());
        let [list_area, details_area] = layout::stack(&[list, panel], page.area())[..] else {
            return;
        };

        let veil = ritual::conjure_veil(Self::TITLE, Self::border_color());
        let inner = veil.inner(list_area);
        // 🔎 scroll first, so the rows about to be shown are known, then look only at those
        let visible = inner.height.saturating_sub(1) as usize;
        let offset = layout::scroll(self.table_state.offset(), selected.unwrap_or(0), visible);
        *self.table_state.offset_mut() = offset;
        let shown = offset..(offset + visible).min(self.vault.profiles.len());
        for i in shown.clone() {
            self.marks
                .entry(i)
                .or_insert_with(|| Mark::of_profile(&self.vault.profiles[i], &self.vault.global));
        }

        // 🩸 a broken profile's row is discolored as well as badged, as in the editor
        let (cursor, count) = (selected.unwrap_or(0), self.vault.profiles.len());
        let rows = self
            .vault
            .profiles
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let row = Row::new([
                    Line::default(),
                    Line::from(p.name.as_str()),
                    Line::from(p.wads.len().to_string()).right_aligned(),
                ]);
                match self.marks.get(&i) {
                    // 📋 picked to be copied: the badge still says what else is going on
                    _ if self.selection.contains(i, cursor, count) => {
                        row.style(theme::worn().styles.selected)
                    }
                    Some(Some(Mark::Broken)) => row.style(theme::worn().styles.broken_row),
                    _ => row,
                }
            })
            .chain([Row::new([
                Line::default(),
                Line::styled(Self::NEW_ROW, Style::new().fg(theme::worn().colors.muted)),
                Line::default(),
            ])]);

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
            theme::worn().styles.broken_cursor
        } else {
            theme::worn().styles.cursor
        });
        page.render_stateful_widget(table, list_area, &mut self.table_state);
        // 🩸 the badges go on last, so even the cursor's highlight can't hide them
        for i in shown {
            if let Some(Some(mark)) = self.marks.get(&i) {
                let y = inner.y + 1 + (i - offset) as u16;
                mark.paint(inner.x, y, inner, page.buffer_mut());
            }
        }

        let (own_area, global_area) = match beside {
            Some(_) => {
                let own = Rect {
                    width: own_panel.0.min(details_area.width),
                    ..details_area
                };
                let global = Rect {
                    x: own.right() + Self::PANEL_GAP,
                    width: details_area
                        .right()
                        .saturating_sub(own.right() + Self::PANEL_GAP),
                    ..details_area
                };
                (own, Some(global))
            }
            None => (details_area, None),
        };
        for (details, area) in [
            (Some(&details), Some(own_area)),
            (beside.as_ref(), global_area),
        ] {
            let (Some(details), Some(area)) = (details, area) else {
                continue;
            };
            let panel = ritual::conjure_banner(
                details.title,
                theme::worn().colors.panel,
                details.title_color,
            );
            let fitting = details.lines_fitting(panel.inner(area).width);
            page.render_widget(Paragraph::new(fitting).block(panel), area);
        }

        let (sigil, sigil_area) = ritual::conjure_sigil(details_area);
        // 📏 the search and the clipboard's note get the row as wide as the main window
        let hint_row = Rect {
            x: list_area.x,
            width: list_area.width,
            ..sigil_area
        };
        match &self.search {
            Some(search) => {
                let matches = self
                    .vault
                    .profiles
                    .iter()
                    .filter(|profile| profile_score(profile, &search.query).is_some())
                    .count();
                search.render(matches, "a profile, or *a wad", hint_row, page);
            }
            None => {
                page.render_widget(sigil, sigil_area);
                let picked = self.selection.picked(cursor, count);
                notes::render(
                    selection::note(picked),
                    self.vault.clipboard.note(),
                    hint_row,
                    page,
                );
            }
        }
    }

    fn prompt(
        &mut self,
        grimoire: &mut Grimoire,
        key_code: KeyCode,
    ) -> Result<Phase<Self::Return>> {
        // 🔎 a search being typed takes every key until it is kept or called off
        if self.search.is_some() {
            return Ok(self.search_key(key_code));
        }
        // 📋 Esc lets go of a selection before it offers to quit
        if key_code == KeyCode::Esc && self.selection.is_active() {
            self.selection.clear();
            return Ok(Phase::Render);
        }
        // 📋 with rows selected, moving and deleting act on every one of them
        if self.selection.is_active() {
            match key_code {
                KeyCode::Char('}') => return Ok(self.shift_selection(true)),
                KeyCode::Char('{') => return Ok(self.shift_selection(false)),
                KeyCode::Char('d') => {
                    self.delete_selection(grimoire)?;
                    return Ok(Phase::Render);
                }
                _ => {}
            }
        }
        // 🧹 moving the cursor changes nothing on disk; anything else might, so the marks are
        // redone, and the selection and the search's matches are let go, since rows could move
        if !matches!(
            key_code,
            KeyCode::Char('j' | 'k' | '?' | '/' | ';' | ',' | 'V' | 'm' | 's' | 'x' | 'y' | 'Y')
                | KeyCode::F(1)
                | KeyCode::Up
                | KeyCode::Down
                | KeyCode::Home
                | KeyCode::End
        ) {
            self.marks.clear();
            self.selection.clear();
            self.found = Found::default();
        }
        let count = self.vault.profiles.len();
        let index = self.table_state.selected_mut().as_mut().unwrap();
        match key_code {
            KeyCode::Enter => {
                if *index != self.vault.profiles.len() {
                    let index = *index;
                    self.launch_profile(grimoire, index)?;
                    Ok(Phase::Render)
                } else {
                    Editor::summon_blank(grimoire, &mut self.vault, index)?;
                    Ok(Phase::Render)
                }
            }
            KeyCode::Char('h') => {
                if *index != self.vault.profiles.len() {
                    let index = *index;
                    self.host_profile(grimoire, index, None)?;
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Prompt)
                }
            }
            KeyCode::Char('n') => {
                if let Some(joining) = JoinAddressForm::summon(grimoire)? {
                    join_game::summon(
                        grimoire,
                        &mut self.vault,
                        index,
                        &joining.address,
                        false,
                        joining.host_ini,
                    )?;
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
            KeyCode::Home => Ok(Phase::render_if_then(*index != 0, || *index = 0)),
            KeyCode::End => Ok(Phase::render_if_then(*index != count, || *index = count)),
            KeyCode::Char('/') => {
                self.search = Some(Search::default());
                Ok(Phase::Render)
            }
            KeyCode::Char(';') => Ok(Phase::render_if(self.search_again(false))),
            KeyCode::Char(',') => Ok(Phase::render_if(self.search_again(true))),
            KeyCode::Char('V') => {
                self.selection.toggle_range(*index, count);
                Ok(Phase::Render)
            }
            KeyCode::Char('m') => {
                self.selection.toggle_mark(*index, count);
                Ok(Phase::Render)
            }
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
            KeyCode::Char('f') => {
                watch::create(grimoire, &mut self.vault, index)?;
                Ok(Phase::Render)
            }
            KeyCode::Char('a') => {
                Editor::summon_blank(grimoire, &mut self.vault, index)?;
                Ok(Phase::Render)
            }
            KeyCode::Char('y') => Ok(Phase::render_if(self.copy_profiles(false))),
            KeyCode::Char('Y') => Ok(Phase::render_if(self.copy_profiles(true))),
            KeyCode::Char('p') => {
                if self.vault.clipboard.is_empty() {
                    return Ok(Phase::Prompt);
                }
                match self.vault.clipboard.clone() {
                    Clipboard::Empty => {}
                    Clipboard::Wads(wads) => self.paste_wads(grimoire, wads)?,
                    Clipboard::Profiles(profiles) => self.paste_profiles(grimoire, profiles)?,
                    Clipboard::CutProfiles(profiles) => self.move_profiles(&profiles),
                    // 🧮 files from the wads screen are hashed before they go in a profile
                    Clipboard::Files { paths, .. } => {
                        if let Some(wads) = hashing::wads(grimoire, &paths)? {
                            self.paste_wads(grimoire, wads)?;
                        }
                    }
                }
                Ok(Phase::Render)
            }
            KeyCode::Char('x') => Ok(Phase::render_if(self.cut_profiles())),
            KeyCode::Char('s') => self.seal_profiles(grimoire),
            KeyCode::Char('i') => {
                let current = self.vault.global.shared();
                if let Some(settings) = share::unseal(grimoire, &mut self.vault, index, &current)? {
                    self.take_shared(grimoire, settings)?;
                }
                Ok(Phase::Render)
            }
            KeyCode::Char('e') => {
                if *index != self.vault.profiles.len() {
                    Editor::summon_existing(grimoire, &mut self.vault, index)?;
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
                    self.vault.global.uzdoom = Some(path.into());
                    if self.vault.keep_global().is_err() {
                        Warning::save_failed(grimoire)?;
                    }
                    return Ok(Phase::Render);
                }
                Ok(Phase::Prompt)
            }
            KeyCode::Char('w') => {
                if let Some(path) =
                    picker::file(grimoire, Some(("base wad", Wad::EXTENSIONS)), None)?
                {
                    // 🖼️ redrawn either way, since the hashing window may have covered the launcher
                    if let Some(wad) = hashing::wad(grimoire, &path)? {
                        self.vault.global.base_wad = Some(wad);
                        self.vault.forget_lookup();
                        if self.vault.keep_global().is_err() {
                            Warning::save_failed(grimoire)?;
                        }
                    }
                    return Ok(Phase::Render);
                }
                Ok(Phase::Prompt)
            }
            KeyCode::Char('l') => {
                let mut shelf = self.vault.shelf();
                Library::summon(grimoire, &mut shelf, &mut self.vault.clipboard)?;
                if self.vault.restock(shelf).is_err() {
                    Warning::save_failed(grimoire)?;
                }
                Ok(Phase::Render)
            }
            KeyCode::Char('o') => {
                self.change_settings(grimoire)?;
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
            KeyCode::Char('?') | KeyCode::F(1) => {
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
    use crate::{testing, vault::InsertedCopies, vault::scribe::tests::Altar};

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

    // 🔎 searching and copying profiles

    fn names(profiles: &[Profile]) -> Vec<&str> {
        profiles.iter().map(|p| p.name.as_str()).collect()
    }

    #[test]
    fn a_search_is_of_profile_names_unless_it_starts_with_a_star() {
        let wad = |name: &str| Wad::new(format!("/wads/{name}").into(), String::new(), 1);
        let profile = Profile::new(
            "nightmare".into(),
            vec![wad("brutalv22.pk3"), wad("brutal.wad")],
        );
        assert!(profile_score(&profile, "night").is_some());
        assert_eq!(profile_score(&profile, "brutal"), None);
        // with a star, the best of its wads counts: here the exact `brutal.wad`
        assert_eq!(
            profile_score(&profile, "*brutal"),
            search::score("brutal.wad", "brutal")
        );
        assert_eq!(profile_score(&profile, "*night"), None);
        assert_eq!(profile_score(&profile, "*"), None);
    }

    #[test]
    fn enter_takes_the_cursor_to_the_best_profile_and_esc_leaves_it() {
        let altar = Altar::new("launcher-search");
        let mut launcher = launcher(&altar, 0);
        let all = launcher.vault.profiles.clone();
        let last = all.len() - 1;
        launcher.search = Some(Search::default());
        for c in all[last].name.chars() {
            assert!(launcher.search_key(KeyCode::Char(c)) == Phase::Render);
        }
        assert_eq!(
            launcher.table_state.selected(),
            Some(0),
            "nothing moves while typing"
        );
        let _ = launcher.search_key(KeyCode::Enter);
        assert_eq!(launcher.table_state.selected(), Some(last));

        launcher.search = Some(Search::default());
        let _ = launcher.search_key(KeyCode::Char('x'));
        let _ = launcher.search_key(KeyCode::Esc);
        assert!(launcher.search.is_none());
        assert_eq!(launcher.table_state.selected(), Some(last));
    }

    #[test]
    fn pasted_profiles_go_in_front_of_the_cursor_tagged_named_or_skipped() {
        let altar = Altar::new("launcher-paste-profiles");
        let mut launcher = launcher(&altar, 0);
        let all = launcher.vault.profiles.clone();
        let copies = vec![all[0].clone(), all[1].clone(), all[2].clone()];
        let mut answers = vec![
            Some(format!("{}_2", all[0].name)),
            None,
            Some("my_copy".to_string()),
        ]
        .into_iter();
        let mut asked = Vec::new();
        let mut at = 1;
        let pasted = launcher
            .vault
            .insert_copies(&mut at, copies, |_, name, tagged| {
                asked.push((name.to_string(), tagged.to_string()));
                Ok(answers.next().unwrap())
            })
            .unwrap();
        // every original is still here, so each name was asked about, with its tag
        assert_eq!(
            asked[0],
            (all[0].name.clone(), format!("{}_2", all[0].name))
        );
        assert_eq!(asked.len(), 3);
        assert_eq!(
            pasted,
            InsertedCopies {
                first: Some(1),
                count: 2,
                failed: false
            }
        );
        let names: Vec<_> = launcher
            .vault
            .profiles
            .iter()
            .map(|p| p.name.clone())
            .collect();
        let first_copy = format!("{}_2", all[0].name);
        assert_eq!(
            names,
            [
                all[0].name.as_str(),
                &first_copy,
                "my_copy",
                &all[1].name,
                &all[2].name
            ]
        );
        // a copy has the original's wads, and is saved
        assert_eq!(launcher.vault.profiles[2].wads, all[2].wads);
        assert!(Profile::path(&launcher.vault.config_dir, "my_copy").exists());
        assert_eq!(at, 3);
    }

    #[test]
    fn pasting_on_the_new_profile_row_adds_at_the_end_and_a_free_name_is_kept() {
        let altar = Altar::new("launcher-paste-end");
        let mut launcher = launcher(&altar, 0);
        let count = launcher.vault.profiles.len();
        let fresh = Profile {
            name: "fresh".into(),
            ..launcher.vault.profiles[0].clone()
        };
        let mut at = count;
        let pasted = launcher
            .vault
            .insert_copies(&mut at, vec![fresh], |_, _, _| {
                panic!("a free name isn't asked about")
            })
            .unwrap();
        assert_eq!(pasted.first, Some(count));
        assert_eq!(launcher.vault.profiles[count].name, "fresh");
        assert_eq!(at, count + 1, "the cursor stays on the new-profile row");
    }

    #[test]
    fn a_name_given_for_a_copy_that_is_taken_after_all_leaves_it_out() {
        let altar = Altar::new("launcher-paste-taken");
        let mut launcher = launcher(&altar, 0);
        let all = launcher.vault.profiles.clone();
        let mut at = 0;
        let pasted = launcher
            .vault
            .insert_copies(&mut at, vec![all[0].clone()], |_, _, _| {
                Ok(Some(all[1].name.clone()))
            })
            .unwrap();
        assert_eq!(
            pasted,
            InsertedCopies {
                first: None,
                count: 0,
                failed: true
            }
        );
        assert_eq!(launcher.vault.profiles, all);
    }

    #[test]
    fn a_profile_is_found_by_its_name_or_the_only_one_that_matches_but_for_case() {
        let named = |names: &[&str]| -> Vec<Profile> {
            names
                .iter()
                .map(|name| Profile::new(name.to_string(), Vec::new()))
                .collect()
        };
        let profiles = named(&["brutal_doom", "Sigil", "sigil_run"]);
        assert_eq!(find_profile(&profiles, "brutal_doom"), Some(0));
        assert_eq!(find_profile(&profiles, "BRUTAL_DOOM"), Some(0));
        assert_eq!(find_profile(&profiles, "sigil"), Some(1));
        assert_eq!(find_profile(&profiles, "nope"), None);
        // two that differ only by case: only the exact one is taken, never a guess
        let twins = named(&["Doom", "doom"]);
        assert_eq!(find_profile(&twins, "doom"), Some(1));
        assert_eq!(find_profile(&twins, "DOOM"), None);
    }

    #[test]
    fn cut_profiles_stay_until_pasted_and_then_move_in_front_of_the_cursor() {
        let altar = Altar::new("launcher-cut");
        let mut launcher = launcher(&altar, 0);
        let all = launcher.vault.profiles.clone();
        let order = [
            all[2].name.clone(),
            all[0].name.clone(),
            all[1].name.clone(),
        ];
        let names = |launcher: &Launcher| -> Vec<String> {
            launcher
                .vault
                .profiles
                .iter()
                .map(|p| p.name.clone())
                .collect()
        };
        launcher.selection.toggle_mark(0, all.len());
        launcher.selection.toggle_mark(1, all.len());
        assert!(launcher.cut_profiles());
        assert_eq!(launcher.vault.profiles, all, "nothing moves on the cut");
        assert_eq!(
            launcher.vault.clipboard.note().as_deref(),
            Some("2 profiles cut")
        );
        assert!(!launcher.selection.is_active());

        // pasted on the new-profile row, they go to the end in the order they were cut
        launcher.table_state.select(Some(all.len()));
        let Clipboard::CutProfiles(cut) = launcher.vault.clipboard.clone() else {
            panic!("the profiles should be cut");
        };
        launcher.move_profiles(&cut);
        assert_eq!(names(&launcher), order);
        assert_eq!(launcher.table_state.selected(), Some(1));
        assert_eq!(
            launcher.vault.clipboard,
            Clipboard::Empty,
            "a cut pastes once"
        );
        // moved, not copied: every profile is still there once, and nothing was written
        assert_eq!(launcher.vault.profiles.len(), all.len());

        // one cut and pasted on itself stays where it is
        launcher.table_state.select(Some(0));
        assert!(launcher.cut_profiles());
        launcher.move_profiles(&[all[2].clone()]);
        assert_eq!(names(&launcher), order);
        // one deleted since its cut is left out
        launcher.move_profiles(&[Profile::new("gone".into(), Vec::new())]);
        assert_eq!(launcher.vault.profiles.len(), all.len());
    }

    #[test]
    fn a_selection_of_profiles_moves_together() {
        let altar = Altar::new("launcher-shift");
        let mut launcher = launcher(&altar, 0);
        let all = launcher.vault.profiles.clone();
        launcher.selection.toggle_mark(0, all.len());
        launcher.selection.toggle_mark(1, all.len());
        assert!(launcher.shift_selection(true) == Phase::Render);
        let names: Vec<_> = launcher.vault.profiles.iter().map(|p| &p.name).collect();
        assert_eq!(names, [&all[2].name, &all[0].name, &all[1].name]);
        assert_eq!(
            launcher.table_state.selected(),
            Some(1),
            "the cursor went along"
        );
        assert!(
            launcher.shift_selection(true) == Phase::Prompt,
            "already at the bottom"
        );
        assert!(launcher.shift_selection(false) == Phase::Render);
        assert_eq!(launcher.vault.profiles, all);
    }

    #[test]
    fn copying_picked_profiles_puts_them_on_the_clipboard_in_list_order() {
        let altar = Altar::new("launcher-copy");
        let mut launcher = launcher(&altar, 2);
        let all = launcher.vault.profiles.clone();
        launcher.selection.toggle_mark(2, all.len());
        launcher.selection.toggle_mark(0, all.len());
        assert!(launcher.copy_profiles(false));
        match &launcher.vault.clipboard {
            Clipboard::Profiles(copied) => assert_eq!(names(copied), [&all[0].name, &all[2].name]),
            other => panic!("expected profiles, found {other:?}"),
        }
        assert!(!launcher.selection.is_active());
        // the new-profile row alone has nothing to copy, and leaves the clipboard be
        launcher.table_state.select(Some(all.len()));
        assert!(!launcher.copy_profiles(false));
        assert!(launcher.copy_profiles(true));
        assert_eq!(launcher.vault.clipboard, Clipboard::Profiles(all));
    }

    // 🚪 what the command line opens, and what is kept from it

    #[test]
    fn kept_dirs_are_the_vaults_then_the_global_ones_each_once() {
        let dirs = remembered_dirs(
            &["/v1".into(), "/both".into()],
            &["/both".into(), "/g1".into()],
        );
        let dirs: Vec<_> = dirs.iter().map(|f| f.to_str().unwrap()).collect();
        assert_eq!(dirs, ["/v1", "/both", "/g1"]);
    }

    #[test]
    fn passed_dirs_are_made_absolute_and_replace_the_kept_ones() {
        let altar = Altar::new("launcher-passed-wads");
        let standard = altar.file("standard");
        Global {
            wad_dirs: vec!["/kept".into()],
            ..Global::default()
        }
        .keep(&Global::path(&standard))
        .unwrap();

        let kept = Launcher::new(Start::default(), &standard, &standard);
        assert_eq!(kept.vault.wad_dirs, [PathBuf::from("/kept")]);

        let start = Start {
            wads: vec!["relative/wads".into()],
            ..Start::default()
        };
        let passed = Launcher::new(start, &standard, &standard);
        let dir = &passed.vault.wad_dirs[0];
        assert!(dir.is_absolute());
        assert!(dir.ends_with("relative/wads"));
        assert_eq!(passed.vault.wad_dirs.len(), 1);
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
    fn forgetting_the_wads_leaves_out_the_kept_dirs_this_run_but_keeps_them_saved() {
        let altar = Altar::new("launcher-forget-wads");
        let standard = altar.file("standard");
        Global {
            wad_dirs: vec!["/kept".into()],
            ..Global::default()
        }
        .keep(&Global::path(&standard))
        .unwrap();
        for forget in [Forget::Once, Forget::Asking] {
            let start = Start {
                forget_wads: forget,
                ..Start::default()
            };
            let opened = Launcher::new(start, &standard, &standard);
            assert!(opened.vault.wad_dirs.is_empty(), "{forget:?}");
            assert_eq!(opened.vault.global.wad_dirs, [OsString::from("/kept")]);
        }
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
            forget_vault: Forget::Asking,
            ..Start::default()
        };
        let opened = Launcher::new(start, &standard, &standard);
        assert_eq!(opened.vault.config_dir, standard);
        // nothing is forgotten yet: that waits for the question
        assert!(opened.vault.global.vault.is_some());
        assert!(opened.vault.notices.is_empty());
    }
}
