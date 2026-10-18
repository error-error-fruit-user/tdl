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
    cli::{Ask, Command, Forget, Keep, Seal},
    clipboard::Clipboard,
    error::{Error, Result},
    layout,
    loadout::Loadout,
    loadout::edition::{Edition, Loaded},
    loadout::host_settings::HostSettings,
    loadout::legacy,
    loadout::profile::Profile,
    loadout::scribe,
    loadout::uac::{Music, Shared, Uac},
    loadout::wad::Wad,
    music, picker,
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
    theme::{self, Dressing, Theme, pride::Pride, splash::Splash},
    uzdoom,
    widgets::{
        details::Details,
        mark::{self, Mark},
        notes,
    },
};

/// The main screen: a table of profiles followed by a `+` row for creating a new one.
///
/// Owns the [`Loadout`] and writes the config on quit.
pub struct Launcher {
    loadout: Loadout,
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
    /// `--uzdoom`, `--base-wad` and `--ini`, until they are put into the global settings for the
    /// session (after any upgrade, since an upgrade compares the files with what it saved), each
    /// with whether to keep it.
    session_uzdoom: Option<PathBuf>,
    keep_uzdoom: Keep,
    session_base_wad: Option<PathBuf>,
    keep_base_wad: Keep,
    session_ini: Option<PathBuf>,
    keep_ini: Keep,
    /// `--theme`, worn already, and whether to keep wearing it.
    session_theme: Option<PathBuf>,
    keep_theme: Keep,
    /// `--splash`, worn already, and whether to keep it.
    session_splash: Option<PathBuf>,
    keep_splash: Keep,
    /// The command from the command line, until it is done.
    command: Option<Command>,
    /// Each profile's mark, by position. A profile is only checked once its row is on screen, and
    /// the marks are forgotten whenever something could have changed them.
    marks: HashMap<usize, Option<Mark>>,
    /// What was passed on the command line, made absolute, for the startup questions about
    /// keeping it.
    passed_loadout: Option<PathBuf>,
    keep_loadout: Keep,
    forget_loadout: Forget,
    passed_wads: Vec<PathBuf>,
    keep_wads: Keep,
    forget_wads: Forget,
    /// The scroll from `--unseal`, until it has been imported.
    passed_scroll: Option<PathBuf>,
    /// Whether the scroll replaces the loadout's profiles (`--unseal-delete`), and asks first.
    unseal_delete: Option<Ask>,
    /// Files from before tdl 1.0.0 that were found, which `upgrade_legacy.py` has to upgrade
    /// before anything is done.
    legacy: Vec<PathBuf>,
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
    pub loadout: Option<String>,
    /// Whether to open the passed loadout every time: asked, not, or without asking.
    pub keep_loadout: Keep,
    /// Opens the standard loadout, and maybe forgets the remembered one.
    pub forget_loadout: Forget,
    pub wads: Vec<String>,
    /// Whether and where to keep the passed wad dirs.
    pub keep_wads: Keep,
    /// Uses no wad dirs but the downloads, and maybe offers to forget the kept ones.
    pub forget_wads: Forget,
    /// A scroll to import as soon as the launcher is up.
    pub unseal: Option<String>,
    /// The scroll replaces every profile in the loadout, asking first or not.
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
    /// The UZDoom to play with this run, over every profile's own and the global one, and whether
    /// to keep it as the global one.
    pub uzdoom: Option<PathBuf>,
    pub keep_uzdoom: Keep,
    /// The base wad, the same way. Hashed once the launcher is up.
    pub base_wad: Option<PathBuf>,
    pub keep_base_wad: Keep,
    /// The ini, the same way.
    pub ini: Option<PathBuf>,
    pub keep_ini: Keep,
    /// The `theme.toml` worn this run (already worn by then), and whether to keep wearing it.
    pub theme: Option<PathBuf>,
    pub keep_theme: Keep,
    /// The `splash.toml` the splash wears this run (already worn by then), which plays the splash
    /// even if the settings skip it, and whether to keep it.
    pub splash_look: Option<PathBuf>,
    pub keep_splash: Keep,
    /// `run`, `host` or `join`: done without the launcher, after which tdl closes.
    pub command: Option<Command>,
}

/// Wears the theme, splash or pride kept as `given` (a path or a name, looked for in
/// `config_dir`), if one is kept: one that can't be worn is said, and the built-in one is worn
/// instead.
fn wear_kept<T: Dressing>(
    grimoire: &mut Grimoire,
    given: Option<&std::ffi::OsStr>,
    config_dir: &Path,
    wear: fn(T),
) -> Result<()> {
    let Some(given) = given else {
        return Ok(());
    };
    match T::load(given, config_dir) {
        Ok(dressing) => wear(dressing),
        Err(problem) => {
            wear(T::default());
            Warning::summon(grimoire, &format!("{problem}, so the built-in one is worn"))?;
        }
    }
    Ok(())
}

/// Wears the theme, splash or pride just picked in the settings as `given`, or the built-in one
/// for none. One that can't be worn after all is said, and the one worn stays.
fn wear_picked<T: Dressing>(
    grimoire: &mut Grimoire,
    given: Option<&std::ffi::OsStr>,
    config_dir: &Path,
    wear: fn(T),
) -> Result<()> {
    match given.map(|given| T::load(given, config_dir)) {
        Some(Err(problem)) => Warning::summon(grimoire, &problem)?,
        Some(Ok(dressing)) => wear(dressing),
        None => wear(T::default()),
    }
    Ok(())
}

/// `path` as an absolute path, so a dir kept from `tdl --wads .` still means the same dir
/// when tdl is started from somewhere else.
fn absolute(path: &str) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| PathBuf::from(path))
}

/// The wad dirs to use when none are passed: the loadout's own, then the global ones, each once.
fn remembered_dirs(loadout: &[OsString], global: &[OsString]) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for dir in loadout.iter().chain(global) {
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

    /// Opens the loadout: `<loadout>/{config,data}` when one is passed, the standard dirs when
    /// asked to forget the kept one, otherwise the one kept in the global settings if it still
    /// exists, otherwise the standard dirs (`standard_config_dir`
    /// and `standard_data_dir`, which also hold `uac.rite`). The wad dirs are the passed
    /// ones, or else the ones kept for this loadout and then the global ones.
    pub fn new(start: Start, standard_config_dir: &Path, standard_data_dir: &Path) -> Self {
        let uac_path = Uac::path(standard_config_dir);
        let mut notices = Vec::new();
        let mut uac_writable = true;
        let global = match Uac::load(&uac_path) {
            Loaded::Fine(global) => global,
            Loaded::Missing => Uac::default(),
            Loaded::Restored(global) => {
                notices.push("Restored your global settings from their backup".to_string());
                global
            }
            Loaded::Lost => {
                notices.push("Your global settings were unreadable, saved as .corrupt".to_string());
                Uac::default()
            }
            Loaded::TooNew | Loaded::Legacy => {
                uac_writable = false;
                Uac::default()
            }
        };
        let passed_loadout = start.loadout.as_deref().map(absolute);
        let kept_loadout = global.loadout.as_ref().map(PathBuf::from).filter(|kept| {
            if start.forget_loadout.is_on() {
                return false;
            }
            let there = kept.is_dir();
            if !there && passed_loadout.is_none() {
                notices.push(format!(
                    "Your usual loadout {} is missing, so the standard one is open",
                    kept.display()
                ));
            }
            there
        });
        let (config_dir, data_dir) = match passed_loadout.clone().or(kept_loadout) {
            Some(root) => (root.join("config"), root.join("data")),
            None => (standard_config_dir.into(), standard_data_dir.into()),
        };
        // 🏚️ looked for before the loadout is opened, which makes its profiles dir
        let legacy = legacy::found(standard_config_dir, &config_dir);
        let mut loadout = Loadout::new(config_dir, data_dir);
        loadout.notices.extend(notices);
        let passed_wads: Vec<_> = start.wads.iter().map(|dir| absolute(dir)).collect();
        loadout.wad_dirs = if !passed_wads.is_empty() {
            passed_wads.clone()
        } else if start.forget_wads.is_on() {
            Vec::new()
        } else {
            remembered_dirs(&loadout.config.wad_dirs, &global.wad_dirs)
        };
        loadout.uac = global;
        loadout.uac_path = uac_path;
        // 🏚️ nothing is written while legacy files wait for the script
        loadout.uac_writable = uac_writable && legacy.is_empty();
        Self {
            loadout,
            table_state: TableState::new().with_selected(0),
            splash: start.splash,
            quip: start.quip,
            no_quips: start.no_quips,
            keep_music: start.keep_music,
            no_music: start.no_music,
            session_uzdoom: start.uzdoom,
            keep_uzdoom: start.keep_uzdoom,
            session_base_wad: start.base_wad,
            keep_base_wad: start.keep_base_wad,
            session_ini: start.ini,
            keep_ini: start.keep_ini,
            session_theme: start.theme,
            keep_theme: start.keep_theme,
            session_splash: start.splash_look,
            keep_splash: start.keep_splash,
            command: start.command,
            marks: HashMap::new(),
            passed_loadout,
            keep_loadout: start.keep_loadout,
            forget_loadout: start.forget_loadout,
            passed_wads,
            keep_wads: start.keep_wads,
            forget_wads: start.forget_wads,
            passed_scroll: start.unseal.as_deref().map(absolute),
            unseal_delete: start.unseal_delete,
            legacy,
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
        // 👗 the kept theme, splash and pride flags, unless the command line already put one on;
        // one that can't be worn is said
        let uac = &self.loadout.uac;
        let (kept_theme, kept_splash) = (uac.theme.clone(), uac.splash.clone());
        let kept_pride = uac.pride_flags.clone();
        let config_dir = self.standard_dir();
        if self.session_theme.is_none() {
            wear_kept(grimoire, kept_theme.as_deref(), &config_dir, theme::wear)?;
        }
        if self.session_splash.is_none() {
            wear_kept(
                grimoire,
                kept_splash.as_deref(),
                &config_dir,
                theme::splash::wear,
            )?;
        }
        wear_kept(
            grimoire,
            kept_pride.as_deref(),
            &config_dir,
            theme::pride::wear,
        )?;
        quips::show(!self.no_quips && !self.loadout.uac.no_quips);
        // 🔇 nothing plays at all, not even on the splash
        if self.no_music || self.loadout.uac.music == Music::Off {
            music::mute();
        }
        // 🎶 the song from the settings, unless `--music` already chose one; a song that has gone
        // missing is said, and the built-in track plays instead
        if !music::has_song()
            && !self.no_music
            && self.loadout.uac.music != Music::Off
            && let Some(song) = &self.loadout.uac.song
            && let Err(problem) = music::use_song(Path::new(song))
        {
            Warning::summon(grimoire, &problem)?;
        }
        // 🎵 music kept on plays from the start, through the splash and past it
        if self.keep_music || self.loadout.uac.music == Music::Kept {
            music::start(true);
        }
        // 🏳️‍🌈 the pride splash, kept on in the settings (`--with-pride` already wore it for this run)
        if self.loadout.uac.pride {
            theme::wear_pride();
        }
        // 🔥 asking for a quip or a look is asking to see the splash, whatever the settings say
        let asked = self.quip.is_some() || self.session_splash.is_some();
        if self.splash && (!self.loadout.uac.skip_splash || asked) {
            // 💬 the command line's quip, or else the one kept in the settings, or else a random one
            let kept = quips::shown()
                .then(|| self.loadout.uac.splash_quip.text())
                .flatten();
            let quip = self.quip.clone().or(kept);
            splash::summon(grimoire, quip.as_deref())?;
        }
        if self.loadout.is_too_new() {
            Warning::files_too_new(grimoire)?;
            return Err(Error::Interrupted);
        }
        // 🏚️ files from before tdl 1.0.0 are upgraded by the script, not here: nothing is touched
        if !self.legacy.is_empty() {
            let advice = legacy::advice(&self.legacy, self.passed_loadout.as_deref());
            Warning::summon_lines(grimoire, &advice)?;
            return Err(Error::Legacy(advice.join("\n")));
        }
        // 📜 anything that was fished out of the wreckage on the way in
        for notice in std::mem::take(&mut self.loadout.notices) {
            Warning::summon(grimoire, &notice)?;
        }
        if self.loadout.is_outdated() {
            if !Dialog::should_upgrade(grimoire)? {
                return Err(Error::Interrupted);
            }
            // ⏳ checking every page takes a moment, so don't leave the old dialog hanging there
            status::summon(grimoire, "Upgrading", "Upgrading...", Self::border_color())?;
            let Ok(upgraded) = self.loadout.upgrade() else {
                Warning::upgrade_failed(grimoire)?;
                return Err(Error::Interrupted);
            };
            if Dialog::should_delete_originals(grimoire)? {
                upgraded.iter().for_each(|path| scribe::retire(path));
            }
        }
        // 🧪 this run's UZDoom, base wad and ini, used over every other, and kept only if asked to
        self.use_session_files();
        if let Some(path) = self.session_base_wad.take() {
            match hashing::wad(grimoire, &path)? {
                Some(wad) => {
                    self.loadout.uac.session_base_wad = Some(wad);
                    self.loadout.forget_lookup();
                }
                None => Warning::summon(
                    grimoire,
                    "The base wad from --base-wad wasn't read, so the usual ones are used",
                )?,
            }
        }
        self.offer_to_keep(grimoire)?;
        picker::set_wad_dirs(&self.loadout.wad_dirs);
        if let Some(scroll) = self.passed_scroll.take() {
            // 📜 the new profile goes last, and the cursor lands on it
            let before = self.loadout.profiles.len();
            let mut index = before;
            let current = self.loadout.uac.shared();
            let replacing = self.unseal_delete;
            if let Some(settings) = share::unseal_file(
                grimoire,
                &mut self.loadout,
                &mut index,
                &scroll,
                &current,
                replacing,
            )? {
                self.take_shared(grimoire, settings)?;
            }
            if replacing.is_some() {
                self.table_state.select(Some(0));
            } else if self.loadout.profiles.len() > before {
                self.table_state.select(Some(before));
            }
        }
        // 👀 the dirs profiles watch may have had wads put in or taken out since last time
        watch::refresh_all(grimoire, &mut self.loadout)?;
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
        if launcher.passed_wads.iter().any(|dir| !dir.is_dir()) {
            launcher.leave_out_missing_wads();
        }
        launcher.loadout.wad_dirs.retain(|dir| dir.is_dir());
        launcher.use_session_files();
        if let Some(path) = launcher.session_base_wad.take() {
            let wad = Wad::hashed(&path, &mut ())
                .map_err(|error| format!("Could not read --base-wad: {error}"))?;
            launcher.loadout.uac.session_base_wad = Some(wad);
            launcher.loadout.forget_lookup();
        }
        // 🗝️ with no windows nothing can be asked, so only the --keep- flags keep anything
        for problem in launcher.keep_unasked() {
            eprintln!("{problem}");
        }
        let loadout = &mut launcher.loadout;
        match command {
            Command::Host { profile, overrides } => {
                let index = find_profile(&loadout.profiles, &profile)
                    .ok_or(format!("There is no profile called '{profile}'"))?;
                watch::refresh_plain(loadout, index)?;
                plain::host(loadout, index, &overrides)
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
                plain::join(loadout, &address, always_download, host_ini)
            }
            Command::Launch { profile } => {
                let index = find_profile(&loadout.profiles, &profile)
                    .ok_or(format!("There is no profile called '{profile}'"))?;
                watch::refresh_plain(loadout, index)?;
                plain::launch(loadout, index)
            }
        }
    }

    /// The launcher for a run with no windows, with the loadout checked as fine to use: notices are
    /// printed, and files from another edition of tdl are an error.
    fn open_plain(start: Start) -> std::result::Result<Self, String> {
        let project_dirs = ProjectDirs::from("error.error.fruit", "eef", "tdl")
            .ok_or("Could not find the home directory")?;
        let mut launcher = Self::new(start, project_dirs.config_dir(), project_dirs.data_dir());
        let loadout = &mut launcher.loadout;
        if loadout.is_too_new() {
            return Err("Your files are from a newer tdl, please update".to_string());
        }
        if !launcher.legacy.is_empty() {
            let advice = legacy::advice(&launcher.legacy, launcher.passed_loadout.as_deref());
            return Err(advice.join("\n"));
        }
        for notice in std::mem::take(&mut loadout.notices) {
            eprintln!("{notice}");
        }
        if loadout.is_outdated() {
            return Err(
                "Your files are from an older tdl: open tdl once to upgrade them, then try again"
                    .to_string(),
            );
        }
        Ok(launcher)
    }

    /// `tdl --seal`: seals the whole loadout into a scroll, with the global settings only if
    /// `--with-settings` asks for them, with no windows.
    pub fn seal_plain(start: Start, seal: &Seal) -> std::result::Result<(), String> {
        let launcher = Self::open_plain(start)?;
        let settings = seal.with_settings.then(|| launcher.loadout.uac.shared());
        plain::seal(&launcher.loadout, settings, &seal.scroll)?;
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
                    let defaults = self.loadout.profiles[index]
                        .host_defaults
                        .clone()
                        .unwrap_or_else(|| HostSettings::new(self.loadout.uac.host_port));
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
                let mut index = self.loadout.profiles.len();
                join_game::summon(
                    grimoire,
                    &mut self.loadout,
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
        let found = find_profile(&self.loadout.profiles, name);
        if found.is_none() {
            Warning::summon(grimoire, &format!("There is no profile called '{name}'"))?;
        }
        Ok(found)
    }

    /// Plays the profile at `index`, unless it has no UZDoom or something it needs is broken, which
    /// is said instead.
    fn launch_profile(&mut self, grimoire: &mut Grimoire, index: usize) -> Result<()> {
        watch::refresh_saved(grimoire, &mut self.loadout, index)?;
        let profile = &self.loadout.profiles[index];
        let Some(uzdoom) = profile.uzdoom(&self.loadout.uac) else {
            return Warning::uzdoom_not_selected(grimoire);
        };
        // 🚧 a profile with a broken part is not fixed for you, but it isn't launched either
        if Warning::faults(grimoire, &profile.faults(&self.loadout.uac))? {
            return Ok(());
        }
        let details = [format!("profile {}", profile.name)];
        let result = uzdoom::launch(
            uzdoom,
            profile.base_wad(&self.loadout.uac).map(Wad::deref),
            profile.ini(&self.loadout.uac).map(OsString::as_os_str),
            profile,
            profile.uzdoom_args(&self.loadout.uac),
            &self.loadout.data_dir.join(&profile.name),
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
        watch::refresh_saved(grimoire, &mut self.loadout, index)?;
        let profile = &self.loadout.profiles[index];
        if profile.uzdoom(&self.loadout.uac).is_none() {
            return Warning::uzdoom_not_selected(grimoire);
        }
        if Warning::faults(grimoire, &profile.faults(&self.loadout.uac))? {
            return Ok(());
        }
        let Some(settings) = HostSettingsForm::summon(grimoire, &mut self.loadout, index, start)?
        else {
            return Ok(());
        };
        host_game::summon(grimoire, &mut self.loadout, index, settings)
    }

    /// The startup questions about what was passed on the command line, for whatever is to be
    /// asked about: whether to open the passed loadout every time, whether to keep the passed wad
    /// dirs for this session, globally, or for this loadout, whether to forget what was kept, and
    /// whether to keep this run's UZDoom, base wad, ini and theme as the global ones. What the
    /// `--keep-` flags keep is kept without asking (see [`Launcher::keep_unasked`]). Dirs that
    /// don't exist are dropped first, with a warning.
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
            self.leave_out_missing_wads();
        }
        for problem in self.keep_unasked() {
            Warning::summon(grimoire, &problem)?;
        }

        if self.forget_loadout == Forget::Asking {
            match self.loadout.uac.loadout.clone() {
                Some(kept) if self.loadout.uac_writable => {
                    let question =
                        format!("Stop opening {} by default?", Path::new(&kept).display());
                    if Dialog::summon(grimoire, &question)? {
                        self.loadout.uac.loadout = None;
                        if self.loadout.keep_uac().is_err() {
                            Warning::save_failed(grimoire)?;
                        }
                    }
                }
                Some(_) => Warning::summon(grimoire, "The global settings are from a newer tdl")?,
                None => Warning::summon(grimoire, "No loadout was being opened by default")?,
            }
        }

        if let Some(passed) = &self.passed_loadout
            && self.keep_loadout == Keep::Ask
            && self.loadout.uac_writable
            && self.loadout.uac.loadout.as_deref() != Some(passed.as_os_str())
            && Dialog::summon(
                grimoire,
                &format!("Open {} every time tdl starts?", passed.display()),
            )?
        {
            self.loadout.uac.loadout = Some(passed.clone().into_os_string());
            if self.loadout.keep_uac().is_err() {
                Warning::save_failed(grimoire)?;
            }
        }

        if self.forget_wads == Forget::Asking {
            self.offer_to_forget_wads(grimoire)?;
        }
        if self.keep_wads == Keep::Ask {
            self.offer_to_keep_wads(grimoire)?;
        }
        self.offer_to_keep_files(grimoire)
    }

    /// Drops the passed wad dirs that aren't dirs, falling back on the kept ones if none are left.
    fn leave_out_missing_wads(&mut self) {
        self.passed_wads.retain(|dir| dir.is_dir());
        // 📂 none of them left: the kept dirs are better than none
        self.loadout.wad_dirs = if self.passed_wads.is_empty() {
            remembered_dirs(&self.loadout.config.wad_dirs, &self.loadout.uac.wad_dirs)
        } else {
            self.passed_wads.clone()
        };
    }

    /// `--wads`: asks whether to keep the passed wad dirs for this session, globally, or for this
    /// loadout, unless they are the kept ones already.
    fn offer_to_keep_wads(&mut self, grimoire: &mut Grimoire) -> Result<()> {
        let passed: Vec<OsString> = self
            .passed_wads
            .iter()
            .map(|dir| dir.clone().into_os_string())
            .collect();
        let remembered = remembered_dirs(&self.loadout.config.wad_dirs, &self.loadout.uac.wad_dirs);
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
        let options = ["Session", "Global", "Loadout"];
        match ThreeWayDialog::summon(grimoire, &question, options)? {
            Some(1) if self.loadout.uac_writable => {
                self.loadout.uac.wad_dirs = passed;
                if self.loadout.keep_uac().is_err() {
                    Warning::save_failed(grimoire)?;
                }
            }
            Some(1) => Warning::summon(grimoire, "The global settings are from a newer tdl")?,
            Some(2) => {
                self.loadout.config.wad_dirs = passed;
                if self
                    .loadout
                    .config
                    .write(&self.loadout.config_dir, &self.loadout.profiles)
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

    /// `--uzdoom`, `--base-wad`, `--ini` and `--theme`: asks, for each that is to be asked about
    /// and isn't the global one already, whether to keep it as the global one.
    fn offer_to_keep_files(&mut self, grimoire: &mut Grimoire) -> Result<()> {
        if !self.loadout.uac_writable {
            return Ok(());
        }
        let uac = &self.loadout.uac;
        let shown = |path: &OsString| Path::new(path).display().to_string();
        let mut kept = false;
        if self.keep_uzdoom == Keep::Ask
            && let Some(uzdoom) = uac.session_uzdoom.clone()
            && uac.uzdoom.as_ref() != Some(&uzdoom)
            && Dialog::summon(
                grimoire,
                &format!("Keep {} as the UZDoom for every profile?", shown(&uzdoom)),
            )?
        {
            self.loadout.uac.uzdoom = Some(uzdoom);
            kept = true;
        }
        let uac = &self.loadout.uac;
        if self.keep_base_wad == Keep::Ask
            && let Some(base_wad) = uac.session_base_wad.clone()
            && uac.base_wad.as_ref() != Some(&base_wad)
            && Dialog::summon(
                grimoire,
                &format!(
                    "Keep {} as the base wad for every profile?",
                    shown(&base_wad.path)
                ),
            )?
        {
            self.loadout.uac.base_wad = Some(base_wad);
            self.loadout.forget_lookup();
            kept = true;
        }
        let uac = &self.loadout.uac;
        if self.keep_ini == Keep::Ask
            && let Some(ini) = uac.session_ini.clone()
            && uac.ini.as_ref() != Some(&ini)
            && Dialog::summon(
                grimoire,
                &format!("Keep {} as the ini for every profile?", shown(&ini)),
            )?
        {
            self.loadout.uac.ini = Some(ini);
            kept = true;
        }
        if self.keep_theme == Keep::Ask
            && let Some(theme) = self.session_theme.clone().map(PathBuf::into_os_string)
            && self.loadout.uac.theme.as_ref() != Some(&theme)
            && Dialog::summon(
                grimoire,
                &format!("Keep wearing the theme {}?", shown(&theme)),
            )?
        {
            self.loadout.uac.theme = Some(theme);
            kept = true;
        }
        if self.keep_splash == Keep::Ask
            && let Some(splash) = self.session_splash.clone().map(PathBuf::into_os_string)
            && self.loadout.uac.splash.as_ref() != Some(&splash)
            && Dialog::summon(grimoire, &format!("Keep the splash {}?", shown(&splash)))?
        {
            self.loadout.uac.splash = Some(splash);
            kept = true;
        }
        if kept && self.loadout.keep_uac().is_err() {
            Warning::save_failed(grimoire)?;
        }
        Ok(())
    }

    /// Keeps and forgets what the `--keep-` flags ask for, without asking: the loadout to open
    /// every time, the wad dirs (globally or for this loadout), this run's UZDoom, base wad, ini
    /// and theme, and forgetting the kept loadout or wad dirs for good. This run's UZDoom, base wad
    /// and ini must be in the global settings' session ones by now. Returns what went wrong, one
    /// line each, for the user.
    fn keep_unasked(&mut self) -> Vec<String> {
        let mut problems = Vec::new();
        let loadout = &mut self.loadout;
        let uac = &mut loadout.uac;
        let mut global = false;
        let mut kept_here = false;
        if self.forget_loadout == Forget::ForGood {
            global |= uac.loadout.take().is_some();
        }
        if self.keep_loadout == Keep::Global
            && let Some(passed) = &self.passed_loadout
        {
            uac.loadout = Some(passed.clone().into_os_string());
            global = true;
        }
        if self.forget_wads == Forget::ForGood {
            global |= !std::mem::take(&mut uac.wad_dirs).is_empty();
            kept_here |= !std::mem::take(&mut loadout.config.wad_dirs).is_empty();
        }
        let passed: Vec<OsString> = self
            .passed_wads
            .iter()
            .map(|dir| dir.clone().into_os_string())
            .collect();
        match self.keep_wads {
            Keep::Global if !passed.is_empty() => {
                loadout.uac.wad_dirs = passed;
                global = true;
            }
            Keep::Loadout if !passed.is_empty() => {
                loadout.config.wad_dirs = passed;
                kept_here = true;
            }
            _ => {}
        }
        let uac = &mut loadout.uac;
        if self.keep_uzdoom == Keep::Global && uac.session_uzdoom.is_some() {
            uac.uzdoom.clone_from(&uac.session_uzdoom);
            global = true;
        }
        if self.keep_base_wad == Keep::Global && uac.session_base_wad.is_some() {
            uac.base_wad.clone_from(&uac.session_base_wad);
            global = true;
        }
        if self.keep_ini == Keep::Global && uac.session_ini.is_some() {
            uac.ini.clone_from(&uac.session_ini);
            global = true;
        }
        if self.keep_theme == Keep::Global
            && let Some(theme) = &self.session_theme
        {
            uac.theme = Some(theme.clone().into_os_string());
            global = true;
        }
        if self.keep_splash == Keep::Global
            && let Some(splash) = &self.session_splash
        {
            uac.splash = Some(splash.clone().into_os_string());
            global = true;
        }
        if global {
            loadout.forget_lookup();
            if !loadout.uac_writable {
                problems.push(
                    "The global settings are from a newer tdl, so nothing was kept in them"
                        .to_string(),
                );
            } else if loadout.keep_uac().is_err() {
                problems.push("Could not save the global settings".to_string());
            }
        }
        if kept_here
            && loadout
                .config
                .write(&loadout.config_dir, &loadout.profiles)
                .is_err()
        {
            problems.push("Could not save the loadout's wad dirs".to_string());
        }
        problems
    }

    /// Puts this run's UZDoom and ini into the global settings' session ones, used over every
    /// other and never saved there. The base wad is hashed first, by the caller.
    fn use_session_files(&mut self) {
        if let Some(uzdoom) = self.session_uzdoom.take() {
            self.loadout.uac.session_uzdoom = Some(uzdoom.into_os_string());
        }
        if let Some(ini) = self.session_ini.take() {
            self.loadout.uac.session_ini = Some(ini.into_os_string());
        }
    }

    /// The theme kept in the global settings, from the standard config dir, before any window is
    /// up: `None` if there is none, else whether it can be worn.
    pub fn kept_theme() -> Option<std::result::Result<Theme, String>> {
        let config_dir = Self::standard_config_dir()?;
        let uac = Uac::open(&Uac::path(&config_dir))?;
        Some(Theme::load(&uac.theme?, &config_dir))
    }

    /// The platform's standard config dir, where `uac.rite` is and where a theme or splash given
    /// by name is looked for.
    pub fn standard_config_dir() -> Option<PathBuf> {
        ProjectDirs::from("error.error.fruit", "eef", "tdl").map(|dirs| dirs.config_dir().into())
    }

    /// The standard config dir this launcher was opened with: where `uac.rite` is.
    fn standard_dir(&self) -> PathBuf {
        self.loadout
            .uac_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default()
    }

    /// `--forget-wads`: asks whether to forget the wad dirs kept for every loadout, for this one, or
    /// both, and forgets them for good if so.
    fn offer_to_forget_wads(&mut self, grimoire: &mut Grimoire) -> Result<()> {
        let list = |dirs: &[OsString]| {
            dirs.iter()
                .map(|dir| Path::new(dir).display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let (global, loadout) = match (
            self.loadout.uac.wad_dirs.is_empty(),
            self.loadout.config.wad_dirs.is_empty(),
        ) {
            (true, true) => return Warning::summon(grimoire, "No wad dirs were kept"),
            (false, true) => {
                let question = format!(
                    "Forget the wad dirs kept for every loadout, {}?",
                    list(&self.loadout.uac.wad_dirs)
                );
                (Dialog::summon(grimoire, &question)?, false)
            }
            (true, false) => {
                let question = format!(
                    "Forget the wad dirs kept for this loadout, {}?",
                    list(&self.loadout.config.wad_dirs)
                );
                (false, Dialog::summon(grimoire, &question)?)
            }
            (false, false) => {
                let question = format!(
                    "Forget the wad dirs kept for every loadout ({}), for this loadout ({}), or both?",
                    list(&self.loadout.uac.wad_dirs),
                    list(&self.loadout.config.wad_dirs)
                );
                match ThreeWayDialog::summon(grimoire, &question, ["Global", "Loadout", "Both"])? {
                    Some(0) => (true, false),
                    Some(1) => (false, true),
                    Some(2) => (true, true),
                    _ => (false, false),
                }
            }
        };
        if global && !self.loadout.uac_writable {
            Warning::summon(grimoire, "The global settings are from a newer tdl")?;
        } else if global {
            self.loadout.uac.wad_dirs.clear();
            if self.loadout.uac.keep(&self.loadout.uac_path).is_err() {
                Warning::save_failed(grimoire)?;
            }
        }
        if loadout {
            self.loadout.config.wad_dirs.clear();
            if self
                .loadout
                .config
                .write(&self.loadout.config_dir, &self.loadout.profiles)
                .is_err()
            {
                Warning::save_failed(grimoire)?;
            }
        }
        Ok(())
    }

    /// Opens the settings, and saves whatever was changed straight away in `uac.rite`, since
    /// the splash is the first thing the next start shows. Settings from a newer tdl can't be
    /// changed at all.
    fn change_settings(&mut self, grimoire: &mut Grimoire) -> Result<()> {
        let before = Settings::of(&self.loadout.uac);
        let config_dir = self.standard_dir();
        let Some(settings) = SettingsForm::summon(grimoire, before.clone(), config_dir)? else {
            return Ok(());
        };
        if settings == before {
            return Ok(());
        }
        if !self.loadout.uac_writable {
            return Warning::summon(
                grimoire,
                "The global settings are from a newer tdl, so they can't be changed",
            );
        }
        let kept_song = self.loadout.uac.song.clone();
        let uac = &self.loadout.uac;
        let kept = (
            uac.theme.clone(),
            uac.splash.clone(),
            uac.pride_flags.clone(),
        );
        let kept_music = self.loadout.uac.music;
        if let Err(problem) = settings.apply(&mut self.loadout.uac) {
            return Warning::summon(grimoire, &problem);
        }
        // 🎶 the song and the music change now
        if self.loadout.uac.song != kept_song {
            match &self.loadout.uac.song {
                Some(song) if music::use_song(Path::new(song)).is_ok() => {}
                _ => music::forget_song(),
            }
            music::restart();
        }
        self.follow_music(kept_music, self.loadout.uac.music);
        // 💬 the quips come and go now, unless `--no-quips` said otherwise for this run
        quips::show(!self.no_quips && !self.loadout.uac.no_quips);
        // 👗 and the theme changes from the next frame on, and the splash and pride from the next
        // splash (the form read each, so it can be worn)
        let uac = &self.loadout.uac;
        let now = (
            uac.theme.clone(),
            uac.splash.clone(),
            uac.pride_flags.clone(),
        );
        let config_dir = self.standard_dir();
        if now.0 != kept.0 {
            wear_picked::<Theme>(grimoire, now.0.as_deref(), &config_dir, theme::wear)?;
        }
        if now.1 != kept.1 {
            wear_picked::<Splash>(grimoire, now.1.as_deref(), &config_dir, theme::splash::wear)?;
        }
        if now.2 != kept.2 {
            wear_picked::<Pride>(grimoire, now.2.as_deref(), &config_dir, theme::pride::wear)?;
        }
        if self.loadout.uac.keep(&self.loadout.uac_path).is_err() {
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
        if !self.loadout.uac_writable {
            return Warning::summon(
                grimoire,
                "The global settings are from a newer tdl, so the scroll's can't be kept",
            );
        }
        let before = self.loadout.uac.music;
        self.loadout.uac.take_shared(settings);
        self.follow_music(before, self.loadout.uac.music);
        quips::show(!self.no_quips && !self.loadout.uac.no_quips);
        if self.loadout.uac.keep(&self.loadout.uac_path).is_err() {
            Warning::save_failed(grimoire)?;
        }
        Ok(())
    }

    /// Moves every selected profile one place down, or up, keeping them selected; nothing moves if
    /// one is already at the end. The cursor goes along if it is on one of them.
    fn shift_selection(&mut self, down: bool) -> Phase<()> {
        let (cursor, count) = (self.cursor(), self.loadout.profiles.len());
        let rows = self.selection.rows(cursor, count);
        let Some(moved) = selection::shift(&mut self.loadout.profiles, &rows, down) else {
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
            .rows(self.cursor(), self.loadout.profiles.len());
        let question = match rows.len() {
            1 => format!(
                "Are you sure you want to delete '{}'?",
                self.loadout.profiles[rows[0]].name
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
            if self.loadout.remove(&mut at).is_err() {
                Warning::save_failed(grimoire)?;
                break;
            }
        }
        let first = rows.first().copied().unwrap_or(0);
        self.table_state
            .select(Some(first.min(self.loadout.profiles.len())));
        Ok(())
    }

    /// Seals profiles into a scroll: the one under the cursor or the ones selected, or the whole
    /// loadout, as the user picks. On the new-profile row with nothing selected, it offers the whole
    /// loadout alone.
    fn seal_profiles(&mut self, grimoire: &mut Grimoire) -> Result<Phase<()>> {
        let count = self.loadout.profiles.len();
        if count == 0 {
            return Ok(Phase::Prompt);
        }
        let picked = self.selection.rows(self.cursor(), count);
        let all: Vec<usize> = (0..count).collect();
        let whole = format!("the whole loadout ({count} profiles)");
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
                    format!("'{}'", self.loadout.profiles[*row].name),
                ),
                _ => (
                    "Selected",
                    format!("the {} selected profiles", picked.len()),
                ),
            };
            let question = format!("Seal {these}, or {whole}?");
            match ThreeWayDialog::summon(grimoire, &question, [option, "Whole loadout", "Back"])? {
                Some(0) => picked,
                Some(1) => all,
                _ => return Ok(Phase::Render),
            }
        };
        let name = match rows.as_slice() {
            [row] => self.loadout.profiles[*row].name.clone(),
            _ if rows.len() == count => "tdl_loadout".to_string(),
            _ => "profiles".to_string(),
        };
        self.selection.clear();
        share::seal(
            grimoire,
            &self.loadout,
            &rows,
            &name,
            self.loadout.uac.shared(),
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
                let profiles = &self.loadout.profiles;
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
        if let Clipboard::Profiles(cut) = std::mem::take(&mut self.loadout.clipboard) {
            self.loadout.clipboard = Clipboard::CutProfiles(cut);
        }
        true
    }

    /// Pastes cut profiles by moving them, in the order they were cut, to just in front of the
    /// cursor, which lands on the first. Ones renamed or deleted since are left out. A cut pastes
    /// once, so the clipboard is emptied. Like the other moves, the new order is saved on quit.
    fn move_profiles(&mut self, cut: &[Profile]) {
        self.loadout.clipboard = Clipboard::Empty;
        let rows: Vec<usize> = cut
            .iter()
            .filter_map(|cut| {
                self.loadout
                    .profiles
                    .iter()
                    .position(|p| p.name == cut.name)
            })
            .collect();
        if rows.is_empty() {
            return;
        }
        let cursor = self.cursor();
        let at = cursor - rows.iter().filter(|&&row| row < cursor).count();
        let moved: Vec<Profile> = rows
            .iter()
            .map(|&row| self.loadout.profiles[row].clone())
            .collect();
        let mut row = 0;
        self.loadout.profiles.retain(|_| {
            row += 1;
            !rows.contains(&(row - 1))
        });
        self.loadout.profiles.splice(at..at, moved);
        self.table_state.select(Some(at));
    }

    /// Copies the picked profiles (see [`Selection`]), or with `all` every profile, onto the
    /// clipboard, and lets go of the selection. Returns whether there was anything to copy.
    fn copy_profiles(&mut self, all: bool) -> bool {
        let count = self.loadout.profiles.len();
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
            .map(|&row| self.loadout.profiles[row].clone())
            .collect();
        self.loadout.clipboard = Clipboard::Profiles(copied);
        true
    }

    /// Pastes copies of `profiles` in front of the cursor (see [`Loadout::insert_copies`]), asking about
    /// each name that is taken: Tag, Name or Skip. The cursor lands on the first copy.
    fn paste_profiles(&mut self, grimoire: &mut Grimoire, profiles: Vec<Profile>) -> Result<()> {
        let mut at = self.cursor();
        let pasted = self
            .loadout
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
        if index == self.loadout.profiles.len() {
            Editor::summon_with(grimoire, &mut self.loadout, &mut index, wads)?;
            self.table_state.select(Some(index));
            return Ok(());
        }
        let mut profile = self.loadout.profiles[index].clone();
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
            if self.loadout.replace(index, profile).is_err() {
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
        let profile = selected.and_then(|i| self.loadout.profiles.get(i));
        let global = Details::global(&self.loadout.uac);
        let global_panel = layout::window(global.size().0, global.size().1);
        let (details, beside) = match profile {
            Some(profile) => (
                Details::launch(
                    profile,
                    &self.loadout.uac,
                    &profile.faults(&self.loadout.uac),
                ),
                Some(global),
            ),
            None => (global, None),
        };
        let is_broken = profile.is_some() && details.is_broken();

        let name_width = self
            .loadout
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
        let (width, height) = layout::window(list_width, self.loadout.profiles.len() as u16 + 2);
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
        let shown = offset..(offset + visible).min(self.loadout.profiles.len());
        for i in shown.clone() {
            self.marks
                .entry(i)
                .or_insert_with(|| Mark::of_profile(&self.loadout.profiles[i], &self.loadout.uac));
        }

        // 🩸 a broken profile's row is discolored as well as badged, as in the editor
        let (cursor, count) = (selected.unwrap_or(0), self.loadout.profiles.len());
        let rows = self
            .loadout
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
                    .loadout
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
                    self.loadout.clipboard.note(),
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
        let count = self.loadout.profiles.len();
        let index = self.table_state.selected_mut().as_mut().unwrap();
        match key_code {
            KeyCode::Enter => {
                if *index != self.loadout.profiles.len() {
                    let index = *index;
                    self.launch_profile(grimoire, index)?;
                    Ok(Phase::Render)
                } else {
                    Editor::summon_blank(grimoire, &mut self.loadout, index)?;
                    Ok(Phase::Render)
                }
            }
            KeyCode::Char('h') => {
                if *index != self.loadout.profiles.len() {
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
                        &mut self.loadout,
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
                self.loadout.profiles.len() + 1,
            ))),
            KeyCode::Char('k') | KeyCode::Up => Ok(Phase::render_if(wrap_around::select_previous(
                self.table_state.selected_mut(),
                self.loadout.profiles.len() + 1,
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
                self.loadout.profiles.len() != 1 && *index != self.loadout.profiles.len(),
                || {
                    let next_index = wrap_around::next(*index, self.loadout.profiles.len());
                    self.loadout.profiles.swap(*index, next_index);
                    *index = next_index;
                },
            )),
            KeyCode::Char('{') => Ok(Phase::render_if_then(
                self.loadout.profiles.len() != 1 && *index != self.loadout.profiles.len(),
                || {
                    let previous_index = wrap_around::previous(*index, self.loadout.profiles.len());
                    self.loadout.profiles.swap(*index, previous_index);
                    *index = previous_index;
                },
            )),
            KeyCode::Char('f') => {
                watch::create(grimoire, &mut self.loadout, index)?;
                Ok(Phase::Render)
            }
            KeyCode::Char('a') => {
                Editor::summon_blank(grimoire, &mut self.loadout, index)?;
                Ok(Phase::Render)
            }
            KeyCode::Char('y') => Ok(Phase::render_if(self.copy_profiles(false))),
            KeyCode::Char('Y') => Ok(Phase::render_if(self.copy_profiles(true))),
            KeyCode::Char('p') => {
                if self.loadout.clipboard.is_empty() {
                    return Ok(Phase::Prompt);
                }
                match self.loadout.clipboard.clone() {
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
                let current = self.loadout.uac.shared();
                if let Some(settings) = share::unseal(grimoire, &mut self.loadout, index, &current)?
                {
                    self.take_shared(grimoire, settings)?;
                }
                Ok(Phase::Render)
            }
            KeyCode::Char('e') => {
                if *index != self.loadout.profiles.len() {
                    Editor::summon_existing(grimoire, &mut self.loadout, index)?;
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Prompt)
                }
            }
            KeyCode::Char('d') => {
                if *index != self.loadout.profiles.len() {
                    if Dialog::should_delete(grimoire, &self.loadout.profiles[*index].name)?
                        && self.loadout.remove(index).is_err()
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
                    self.loadout.uac.uzdoom = Some(path.into());
                    if self.loadout.keep_uac().is_err() {
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
                        self.loadout.uac.base_wad = Some(wad);
                        self.loadout.forget_lookup();
                        if self.loadout.keep_uac().is_err() {
                            Warning::save_failed(grimoire)?;
                        }
                    }
                    return Ok(Phase::Render);
                }
                Ok(Phase::Prompt)
            }
            KeyCode::Char('l') => {
                let mut shelf = self.loadout.shelf();
                Library::summon(grimoire, &mut shelf, &mut self.loadout.clipboard)?;
                if self.loadout.restock(shelf).is_err() {
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
                        .loadout
                        .config
                        .write(&self.loadout.config_dir, &self.loadout.profiles)
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
    use crate::{
        loadout::InsertedCopies, loadout::config::Config, loadout::scribe::tests::Altar, testing,
    };

    fn launcher(altar: &Altar, row: usize) -> Launcher {
        let loadout = testing::example_loadout(altar);
        let start = Start {
            loadout: Some(loadout.to_string_lossy().into_owned()),
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
        let all = launcher.loadout.profiles.clone();
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
        let all = launcher.loadout.profiles.clone();
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
            .loadout
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
            .loadout
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
        assert_eq!(launcher.loadout.profiles[2].wads, all[2].wads);
        assert!(Profile::path(&launcher.loadout.config_dir, "my_copy").exists());
        assert_eq!(at, 3);
    }

    #[test]
    fn pasting_on_the_new_profile_row_adds_at_the_end_and_a_free_name_is_kept() {
        let altar = Altar::new("launcher-paste-end");
        let mut launcher = launcher(&altar, 0);
        let count = launcher.loadout.profiles.len();
        let fresh = Profile {
            name: "fresh".into(),
            ..launcher.loadout.profiles[0].clone()
        };
        let mut at = count;
        let pasted = launcher
            .loadout
            .insert_copies(&mut at, vec![fresh], |_, _, _| {
                panic!("a free name isn't asked about")
            })
            .unwrap();
        assert_eq!(pasted.first, Some(count));
        assert_eq!(launcher.loadout.profiles[count].name, "fresh");
        assert_eq!(at, count + 1, "the cursor stays on the new-profile row");
    }

    #[test]
    fn a_name_given_for_a_copy_that_is_taken_after_all_leaves_it_out() {
        let altar = Altar::new("launcher-paste-taken");
        let mut launcher = launcher(&altar, 0);
        let all = launcher.loadout.profiles.clone();
        let mut at = 0;
        let pasted = launcher
            .loadout
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
        assert_eq!(launcher.loadout.profiles, all);
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
        let all = launcher.loadout.profiles.clone();
        let order = [
            all[2].name.clone(),
            all[0].name.clone(),
            all[1].name.clone(),
        ];
        let names = |launcher: &Launcher| -> Vec<String> {
            launcher
                .loadout
                .profiles
                .iter()
                .map(|p| p.name.clone())
                .collect()
        };
        launcher.selection.toggle_mark(0, all.len());
        launcher.selection.toggle_mark(1, all.len());
        assert!(launcher.cut_profiles());
        assert_eq!(launcher.loadout.profiles, all, "nothing moves on the cut");
        assert_eq!(
            launcher.loadout.clipboard.note().as_deref(),
            Some("2 profiles cut")
        );
        assert!(!launcher.selection.is_active());

        // pasted on the new-profile row, they go to the end in the order they were cut
        launcher.table_state.select(Some(all.len()));
        let Clipboard::CutProfiles(cut) = launcher.loadout.clipboard.clone() else {
            panic!("the profiles should be cut");
        };
        launcher.move_profiles(&cut);
        assert_eq!(names(&launcher), order);
        assert_eq!(launcher.table_state.selected(), Some(1));
        assert_eq!(
            launcher.loadout.clipboard,
            Clipboard::Empty,
            "a cut pastes once"
        );
        // moved, not copied: every profile is still there once, and nothing was written
        assert_eq!(launcher.loadout.profiles.len(), all.len());

        // one cut and pasted on itself stays where it is
        launcher.table_state.select(Some(0));
        assert!(launcher.cut_profiles());
        launcher.move_profiles(&[all[2].clone()]);
        assert_eq!(names(&launcher), order);
        // one deleted since its cut is left out
        launcher.move_profiles(&[Profile::new("gone".into(), Vec::new())]);
        assert_eq!(launcher.loadout.profiles.len(), all.len());
    }

    #[test]
    fn a_selection_of_profiles_moves_together() {
        let altar = Altar::new("launcher-shift");
        let mut launcher = launcher(&altar, 0);
        let all = launcher.loadout.profiles.clone();
        launcher.selection.toggle_mark(0, all.len());
        launcher.selection.toggle_mark(1, all.len());
        assert!(launcher.shift_selection(true) == Phase::Render);
        let names: Vec<_> = launcher.loadout.profiles.iter().map(|p| &p.name).collect();
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
        assert_eq!(launcher.loadout.profiles, all);
    }

    #[test]
    fn copying_picked_profiles_puts_them_on_the_clipboard_in_list_order() {
        let altar = Altar::new("launcher-copy");
        let mut launcher = launcher(&altar, 2);
        let all = launcher.loadout.profiles.clone();
        launcher.selection.toggle_mark(2, all.len());
        launcher.selection.toggle_mark(0, all.len());
        assert!(launcher.copy_profiles(false));
        match &launcher.loadout.clipboard {
            Clipboard::Profiles(copied) => assert_eq!(names(copied), [&all[0].name, &all[2].name]),
            other => panic!("expected profiles, found {other:?}"),
        }
        assert!(!launcher.selection.is_active());
        // the new-profile row alone has nothing to copy, and leaves the clipboard be
        launcher.table_state.select(Some(all.len()));
        assert!(!launcher.copy_profiles(false));
        assert!(launcher.copy_profiles(true));
        assert_eq!(launcher.loadout.clipboard, Clipboard::Profiles(all));
    }

    // 🚪 what the command line opens, and what is kept from it

    #[test]
    fn kept_dirs_are_the_loadouts_then_the_global_ones_each_once() {
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
        Uac {
            wad_dirs: vec!["/kept".into()],
            ..Uac::default()
        }
        .keep(&Uac::path(&standard))
        .unwrap();

        let kept = Launcher::new(Start::default(), &standard, &standard);
        assert_eq!(kept.loadout.wad_dirs, [PathBuf::from("/kept")]);

        let start = Start {
            wads: vec!["relative/wads".into()],
            ..Start::default()
        };
        let passed = Launcher::new(start, &standard, &standard);
        let dir = &passed.loadout.wad_dirs[0];
        assert!(dir.is_absolute());
        assert!(dir.ends_with("relative/wads"));
        assert_eq!(passed.loadout.wad_dirs.len(), 1);
    }

    #[test]
    fn the_kept_loadout_opens_unless_another_is_passed() {
        let altar = Altar::new("launcher-kept-loadout");
        let standard = altar.file("standard");
        let kept = altar.file("kept");
        fs::create_dir_all(&kept).unwrap();
        Uac {
            loadout: Some(kept.clone().into_os_string()),
            ..Uac::default()
        }
        .keep(&Uac::path(&standard))
        .unwrap();

        let opened = Launcher::new(Start::default(), &standard, &standard);
        assert_eq!(opened.loadout.config_dir, kept.join("config"));

        let other = altar.file("other");
        let start = Start {
            loadout: Some(other.to_string_lossy().into_owned()),
            ..Start::default()
        };
        let passed = Launcher::new(start, &standard, &standard);
        assert_eq!(passed.loadout.config_dir, other.join("config"));
        assert_eq!(passed.passed_loadout, Some(other));
    }

    #[test]
    fn a_kept_loadout_that_is_gone_falls_back_to_the_standard_one_and_says_so() {
        let altar = Altar::new("launcher-gone-loadout");
        let standard = altar.file("standard");
        Uac {
            loadout: Some(altar.file("gone").into_os_string()),
            ..Uac::default()
        }
        .keep(&Uac::path(&standard))
        .unwrap();
        let opened = Launcher::new(Start::default(), &standard, &standard);
        assert_eq!(opened.loadout.config_dir, standard);
        assert!(
            opened
                .loadout
                .notices
                .iter()
                .any(|n| n.contains("is missing"))
        );
    }

    #[test]
    fn forgetting_the_wads_leaves_out_the_kept_dirs_this_run_but_keeps_them_saved() {
        let altar = Altar::new("launcher-forget-wads");
        let standard = altar.file("standard");
        Uac {
            wad_dirs: vec!["/kept".into()],
            ..Uac::default()
        }
        .keep(&Uac::path(&standard))
        .unwrap();
        for forget in [Forget::Once, Forget::Asking] {
            let start = Start {
                forget_wads: forget,
                ..Start::default()
            };
            let opened = Launcher::new(start, &standard, &standard);
            assert!(opened.loadout.wad_dirs.is_empty(), "{forget:?}");
            assert_eq!(opened.loadout.uac.wad_dirs, [OsString::from("/kept")]);
        }
    }

    #[test]
    fn forgetting_opens_the_standard_loadout_even_with_one_kept() {
        let altar = Altar::new("launcher-forget-loadout");
        let standard = altar.file("standard");
        let kept = altar.file("kept");
        fs::create_dir_all(&kept).unwrap();
        Uac {
            loadout: Some(kept.into_os_string()),
            ..Uac::default()
        }
        .keep(&Uac::path(&standard))
        .unwrap();
        let start = Start {
            forget_loadout: Forget::Asking,
            ..Start::default()
        };
        let opened = Launcher::new(start, &standard, &standard);
        assert_eq!(opened.loadout.config_dir, standard);
        // nothing is forgotten yet: that waits for the question
        assert!(opened.loadout.uac.loadout.is_some());
        assert!(opened.loadout.notices.is_empty());
    }

    #[test]
    fn the_keep_flags_keep_without_asking_and_session_keeps_nothing() {
        let altar = Altar::new("launcher-keep-unasked");
        let standard = altar.file("standard");
        let other = altar.file("other");
        let start = |keep| Start {
            loadout: Some(other.to_string_lossy().into_owned()),
            keep_loadout: keep,
            wads: vec!["/wads".into()],
            keep_wads: keep,
            uzdoom: Some("/opt/uzdoom".into()),
            keep_uzdoom: keep,
            ini: Some("/inis/mine.ini".into()),
            keep_ini: keep,
            theme: Some("/themes/blue.toml".into()),
            keep_theme: keep,
            ..Start::default()
        };
        for keep in [Keep::Ask, Keep::Session] {
            let mut launcher = Launcher::new(start(keep), &standard, &standard);
            launcher.use_session_files();
            assert!(launcher.keep_unasked().is_empty());
            // used this run, but nothing is saved
            assert_eq!(
                launcher.loadout.uac.session_uzdoom,
                Some("/opt/uzdoom".into())
            );
            assert_eq!(
                launcher.loadout.uac.session_ini,
                Some("/inis/mine.ini".into())
            );
            assert_eq!(Uac::open(&Uac::path(&standard)), None, "{keep:?}");
        }
        let mut launcher = Launcher::new(start(Keep::Global), &standard, &standard);
        launcher.use_session_files();
        assert!(launcher.keep_unasked().is_empty());
        let kept = Uac::open(&Uac::path(&standard)).unwrap();
        assert_eq!(kept.loadout, Some(other.clone().into_os_string()));
        assert_eq!(kept.wad_dirs, [OsString::from("/wads")]);
        assert_eq!(kept.uzdoom, Some("/opt/uzdoom".into()));
        assert_eq!(kept.ini, Some("/inis/mine.ini".into()));
        assert_eq!(kept.theme, Some("/themes/blue.toml".into()));
        // the wad dirs can go to the loadout instead
        let start = Start {
            wads: vec!["/mods".into()],
            keep_wads: Keep::Loadout,
            ..Start::default()
        };
        let mut launcher = Launcher::new(start, &standard, &standard);
        assert!(launcher.keep_unasked().is_empty());
        let reopened = Launcher::new(Start::default(), &standard, &standard);
        assert_eq!(reopened.loadout.config.wad_dirs, [OsString::from("/mods")]);
        assert_eq!(reopened.loadout.uac.wad_dirs, [OsString::from("/wads")]);
    }

    #[test]
    fn forgetting_for_good_forgets_the_kept_loadout_and_every_kept_wad_dir() {
        let altar = Altar::new("launcher-forget-for-good");
        let standard = altar.file("standard");
        let kept = altar.file("kept");
        fs::create_dir_all(&kept).unwrap();
        Uac {
            loadout: Some(kept.into_os_string()),
            wad_dirs: vec!["/kept".into()],
            ..Uac::default()
        }
        .keep(&Uac::path(&standard))
        .unwrap();
        // the standard loadout, opened once forgetting, keeps wad dirs of its own
        Config {
            wad_dirs: vec!["/kept-here".into()],
            ..Config::default()
        }
        .write(&standard, &[])
        .unwrap();
        let start = Start {
            forget_loadout: Forget::ForGood,
            forget_wads: Forget::ForGood,
            ..Start::default()
        };
        let mut launcher = Launcher::new(start, &standard, &standard);
        assert!(launcher.loadout.wad_dirs.is_empty());
        assert_eq!(
            launcher.loadout.config.wad_dirs,
            [OsString::from("/kept-here")]
        );
        assert!(launcher.keep_unasked().is_empty());
        let reopened = Launcher::new(Start::default(), &standard, &standard);
        assert_eq!(reopened.loadout.uac.loadout, None);
        assert!(reopened.loadout.uac.wad_dirs.is_empty());
        assert!(reopened.loadout.config.wad_dirs.is_empty());
    }

    #[test]
    fn legacy_files_are_found_before_the_loadout_is_opened_and_left_as_they_are() {
        let altar = Altar::new("launcher-legacy");
        let standard = altar.file("standard");
        fs::create_dir_all(standard.join("profiles")).unwrap();
        let old = rmp_serde::to_vec(&("brutal", Vec::<()>::new())).unwrap();
        fs::write(standard.join("profiles/brutal.doom"), &old).unwrap();
        fs::write(standard.join("global.rite"), b"old").unwrap();
        let launcher = Launcher::new(Start::default(), &standard, &standard);
        assert_eq!(
            launcher.legacy,
            [standard.join("global.rite"), standard.join("profiles")]
        );
        assert!(!launcher.loadout.uac_writable, "nothing is saved over them");
        assert_eq!(
            fs::read(standard.join("profiles/brutal.doom")).unwrap(),
            old
        );
        assert_eq!(fs::read(standard.join("global.rite")).unwrap(), b"old");
    }
}
