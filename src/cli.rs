//! The command line, read by clap: `tdl [options] [dir]` opens the launcher, `tdl launch`,
//! `tdl host` and `tdl join` do one thing and close, `tdl cli` does the launcher's work from the command line,
//! `tdl headless` presses keys from a script
//! (see the possession in [`ritual`](crate::ritual)), and `tdl --seal` and `tdl --list-quips` do theirs without any
//! windows. `tdl --help` lists it all, in the theme's colors.

use std::path::PathBuf;

use clap::{
    ArgGroup, Args as ClapArgs, CommandFactory, Parser, Subcommand,
    builder::{NonEmptyStringValueParser, Styles, styling},
    error::ErrorKind,
};
use ratatui::style::Color;

use crate::{
    loadout::host_settings::HostSettings,
    net::{self, Address, ini::HostIni},
    quips, theme,
};

/// What was asked for on the command line.
#[derive(Debug, Default, PartialEq)]
pub struct Cli {
    /// Replaces the platform's standard config and data dirs.
    pub loadout: Option<String>,
    /// Whether to open the loadout every time: asked (`--loadout`), not (with `--session`), or so
    /// without asking (`--keep-loadout`).
    pub keep_loadout: Keep,
    /// `--forget-loadout` or `--keep-forget-loadout`: the standard loadout instead of the
    /// remembered one.
    pub forget_loadout: Forget,
    /// Dirs the user keeps wads in, in the order given.
    pub wads: Vec<String>,
    /// Whether and where to keep the wad dirs: asked (`--wads`), not (with `--session`), or
    /// without asking, for every loadout (`--keep-wads`) or this one (`--keep-loadout-wads`).
    pub keep_wads: Keep,
    /// `--forget-wads` or `--keep-forget-wads`: no wad dirs but the downloads instead of the kept
    /// ones.
    pub forget_wads: Forget,
    /// A scroll to import on startup.
    pub unseal: Option<String>,
    /// The scroll came from `--unseal-delete` or `--unseal-delete-force`: it replaces every
    /// profile in the loadout instead of joining them.
    pub unseal_delete: Option<Ask>,
    /// Seals the whole loadout into a scroll, without any windows.
    pub seal: Option<Seal>,
    /// Lists the built-in quips and does nothing else.
    pub list_quips: bool,
    /// Says what a controller sends, with no windows, until Ctrl+C.
    pub controller_test: bool,
    /// `tdl setup [TOML]`: keeps what the TOML gives (see [`setup`](crate::setup)) without any
    /// windows, and closes. The inner `None` is the default file.
    pub setup: Option<Option<PathBuf>>,
    /// Plays the pride splash for this run.
    pub pride: bool,
    /// Skips the splash screen.
    pub no_splash: bool,
    /// Keeps the music playing in the launcher, not just on the splash screen.
    pub keep_music: bool,
    /// A song of the user's own to play instead of the built-in track.
    pub music: Option<String>,
    /// Plays no music at all this run.
    pub no_music: bool,
    /// The splash screen's quip from `--splash-quip`, its numbers already replaced by their quips.
    pub quip: Option<String>,
    /// Leaves the quips out of every window for this run.
    pub no_quips: bool,
    /// The UZDoom to play with this run, over every other, and whether to keep it as the global
    /// one, the same way as the loadout.
    pub uzdoom: Option<String>,
    pub keep_uzdoom: Keep,
    /// The base wad to play with this run, over every other, the same way.
    pub base_wad: Option<String>,
    pub keep_base_wad: Keep,
    /// The ini to play with this run, over every other, the same way.
    pub ini: Option<String>,
    pub keep_ini: Keep,
    /// The theme to wear this run, and whether to keep wearing it, the same way.
    pub theme: Option<String>,
    pub keep_theme: Keep,
    /// The splash's look (a `splash.toml`) to wear this run, and whether to keep wearing it.
    pub splash: Option<String>,
    pub keep_splash: Keep,
    /// `launch`, `host` or `join`: something to do straight away, after which tdl closes.
    pub command: Option<Command>,
    /// `--headless`: launch, host or join with no forms or windows, only plain lines in the
    /// terminal.
    pub headless: bool,
    /// `tdl headless`: a script of presses, with no splash and nothing drawn.
    pub play: Option<Play>,
    /// `tdl cli ...`: one of the launcher's rites, done from the command line.
    pub rite: Option<Rite>,
}

/// `tdl headless`: the script of presses (stdin if there is none), and where `--log` writes
/// `tdl.log` and `uzdoom.log`, if it was given.
#[derive(Debug, PartialEq)]
pub struct Play {
    pub script: Option<PathBuf>,
    pub log: Option<PathBuf>,
}

/// `--seal`: the scroll to write the whole loadout to, and whether the global settings go in too
/// (`--with-settings`).
#[derive(Debug, PartialEq)]
pub struct Seal {
    pub scroll: PathBuf,
    pub with_settings: bool,
}

/// What to do about keeping something passed on the command line. The plain flag asks,
/// `--session` makes it just for this run, and its `--keep-` flag keeps it without asking, even
/// with `--session`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Keep {
    /// Asked once tdl is up.
    #[default]
    Ask,
    /// Just for this run: nothing is asked or kept.
    Session,
    /// Kept for every loadout (in `uac.rite`), without asking.
    Global,
    /// Kept for the loadout that is open, without asking: only `--keep-loadout-wads`.
    Loadout,
}

impl Keep {
    /// `--keep-<flag>` keeps it; else `--session` makes it just for this run; else it is asked.
    fn from_flags(keep: bool, session: bool) -> Self {
        match (keep, session) {
            (true, _) => Self::Global,
            (_, true) => Self::Session,
            _ => Self::Ask,
        }
    }
}

/// What `--forget-loadout` and `--forget-wads` ask for: to leave what was kept out of this run,
/// and then to ask whether to forget it for good (the plain flag), not to (with `--session`), or to
/// forget it without asking (`--keep-forget-`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Forget {
    #[default]
    Nothing,
    /// Left out of this run only, without asking.
    Once,
    /// Left out of this run, and the user is asked whether to forget it.
    Asking,
    /// Forgotten for good, without asking.
    ForGood,
}

impl Forget {
    fn from_flags(forget: bool, for_good: bool, session: bool) -> Self {
        match (forget, for_good, session) {
            (_, true, _) => Self::ForGood,
            (true, _, true) => Self::Once,
            (true, _, false) => Self::Asking,
            _ => Self::Nothing,
        }
    }

    /// Whether what was kept is left out of this run.
    pub fn is_on(self) -> bool {
        self != Self::Nothing
    }
}

/// Whether something that can't be undone asks first: `--unseal-delete` does, and
/// `--unseal-delete-force` doesn't.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ask {
    First,
    Never,
}

/// What `tdl launch`, `tdl host` and `tdl join` ask for. Each is done without the launcher, and
/// then tdl closes.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Play the profile with this name.
    Launch { profile: String },
    /// Host the profile with this name, with its host defaults changed by `overrides`.
    Host {
        profile: String,
        overrides: HostOverrides,
    },
    /// Join the game hosted at `host`, on `port` (the default if `None`) with `code`, which the
    /// join form asks for if it wasn't given. With `always_download`, wads that aren't installed
    /// are downloaded without asking first. `host_ini` says whether the game is played with the
    /// host's ini (and its keys), if it shares it.
    Join {
        host: String,
        port: Option<u16>,
        code: Option<String>,
        always_download: bool,
        host_ini: HostIni,
    },
}

/// The host settings `tdl host` was given, each over the profile's own default.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HostOverrides {
    pub players: Option<u8>,
    pub deathmatch: Option<bool>,
    pub map: Option<String>,
    pub port: Option<u16>,
    pub code: Option<String>,
    pub share_ini: Option<bool>,
}

impl HostOverrides {
    /// `settings` with every override given put over it.
    pub fn apply(&self, settings: HostSettings) -> HostSettings {
        HostSettings {
            player_count: self
                .players
                .map_or(settings.player_count, |players| players.to_string()),
            is_deathmatch: self.deathmatch.unwrap_or(settings.is_deathmatch),
            map: self.map.clone().unwrap_or(settings.map),
            port: self.port.map_or(settings.port, |port| port.to_string()),
            code: self.code.clone().unwrap_or(settings.code),
            share_ini: self.share_ini.unwrap_or(settings.share_ini),
        }
    }
}

impl Command {
    /// For `join`: the address to join, if there is a code (a headless join must have one).
    pub fn address(host: &str, port: Option<u16>, code: Option<&str>) -> Option<Address> {
        Some(Address {
            host: host.to_string(),
            port: port.unwrap_or(net::DEFAULT_PORT),
            code: code?.to_string(),
        })
    }
}

/// The help's headings and the options under them.
const LOADOUT: &str = "Loadout";
const GAME: &str = "Playing";
const SHARING: &str = "Sharing";
const SPLASH: &str = "Looks and sound";
const THIS_RUN: &str = "Just for this run";
/// Every flag that opens or forgets a loadout, of which only one can be given (a group, in clap).
const LOADOUT_FLAGS: [&str; 5] = [
    "dir",
    "loadout",
    "keep_loadout",
    "forget_loadout",
    "keep_forget_loadout",
];
/// Every flag that gives or forgets wad dirs, of which only one kind can be given.
const WADS_FLAGS: [&str; 5] = [
    "wads",
    "keep_wads",
    "keep_loadout_wads",
    "forget_wads",
    "keep_forget_wads",
];

/// The arguments as clap reads them, turned into a [`Cli`] by [`Cli::parse`].
#[derive(Parser, Debug)]
#[command(
    name = "tdl",
    version,
    about = "A terminal launcher for UZDoom: profiles of wads, one key to play, and multiplayer \
             that hands joiners the host's wads",
    styles = styles(),
    max_term_width = 100,
    group = ArgGroup::new("loadouts").args(LOADOUT_FLAGS),
    group = ArgGroup::new("wad_dirs").args(WADS_FLAGS)
)]
struct Args {
    /// The loadout dir on its own, the same as --loadout DIR
    #[arg(value_name = "DIR")]
    dir: Option<String>,

    /// Keep the config and data in DIR/config and DIR/data; tdl asks whether to open it
    /// every time
    #[arg(
        long,
        value_name = "DIR",
        global = true,
        help_heading = LOADOUT
    )]
    loadout: Option<String>,

    /// Like --loadout, and open it every time without asking
    #[arg(
        long,
        value_name = "DIR",
        global = true,
        help_heading = LOADOUT
    )]
    keep_loadout: Option<String>,

    /// Open the standard loadout, and ask whether to stop opening the one remembered from
    /// --loadout
    #[arg(long, global = true, help_heading = LOADOUT)]
    forget_loadout: bool,

    /// Like --forget-loadout, and stop opening the remembered one without asking
    #[arg(long, global = true, help_heading = LOADOUT)]
    keep_forget_loadout: bool,

    /// Where your wads are: file pickers open here, and joining looks here for wads it already
    /// has. Give it more than once for more dirs; tdl asks whether to keep them
    #[arg(
        long,
        value_name = "DIR",
        global = true,
        help_heading = LOADOUT
    )]
    wads: Vec<String>,

    /// Like --wads, and keep them for every loadout without asking
    #[arg(
        long,
        value_name = "DIR",
        global = true,
        help_heading = LOADOUT
    )]
    keep_wads: Vec<String>,

    /// Like --wads, and keep them for this loadout without asking
    #[arg(
        long,
        value_name = "DIR",
        global = true,
        help_heading = LOADOUT
    )]
    keep_loadout_wads: Vec<String>,

    /// Look for wads only in the downloads this run, and ask whether to forget the wad dirs kept
    /// from --wads
    #[arg(long, global = true, help_heading = LOADOUT)]
    forget_wads: bool,

    /// Like --forget-wads, and forget every kept wad dir (this loadout's and every loadout's)
    /// without asking
    #[arg(long, global = true, help_heading = LOADOUT)]
    keep_forget_wads: bool,

    /// Play with this UZDoom over every profile's own and the global one; tdl asks whether to
    /// keep it as the global one
    #[arg(
        long,
        value_name = "FILE",
        global = true,
        value_parser = NonEmptyStringValueParser::new(),
        conflicts_with = "keep_uzdoom",
        help_heading = GAME
    )]
    uzdoom: Option<String>,

    /// Like --uzdoom, and keep it as the global one without asking
    #[arg(
        long,
        value_name = "FILE",
        global = true,
        value_parser = NonEmptyStringValueParser::new(),
        help_heading = GAME
    )]
    keep_uzdoom: Option<String>,

    /// Play with this base wad, the same way
    #[arg(
        long,
        value_name = "FILE",
        global = true,
        value_parser = NonEmptyStringValueParser::new(),
        conflicts_with = "keep_base_wad",
        help_heading = GAME
    )]
    base_wad: Option<String>,

    /// Like --base-wad, and keep it as the global one without asking
    #[arg(
        long,
        value_name = "FILE",
        global = true,
        value_parser = NonEmptyStringValueParser::new(),
        help_heading = GAME
    )]
    keep_base_wad: Option<String>,

    /// Play with this UZDoom ini, the same way
    #[arg(
        long,
        value_name = "FILE",
        global = true,
        value_parser = NonEmptyStringValueParser::new(),
        conflicts_with = "keep_ini",
        help_heading = GAME
    )]
    ini: Option<String>,

    /// Like --ini, and keep it as the global one without asking
    #[arg(
        long,
        value_name = "FILE",
        global = true,
        value_parser = NonEmptyStringValueParser::new(),
        help_heading = GAME
    )]
    keep_ini: Option<String>,

    /// Import a shared .scroll as soon as tdl starts, looking for its wads in the wad dirs.
    /// Global settings sealed in it replace yours
    #[arg(long, value_name = "SCROLL", global = true, help_heading = SHARING)]
    unseal: Option<String>,

    /// Like --unseal, but the scroll's profiles replace the loadout's: every profile in it is
    /// deleted first (with its saved games and logs; never the downloaded wads or your global
    /// settings), after asking
    #[arg(
        long,
        value_name = "SCROLL",
        global = true,
        conflicts_with = "unseal",
        help_heading = SHARING
    )]
    unseal_delete: Option<String>,

    /// Like --unseal-delete, without asking first
    #[arg(
        long,
        value_name = "SCROLL",
        global = true,
        conflicts_with_all = ["unseal", "unseal_delete"],
        help_heading = SHARING
    )]
    unseal_delete_force: Option<String>,

    /// Seal every profile in the loadout into SCROLL, with no windows, and close
    #[arg(
        long,
        value_name = "SCROLL",
        global = true,
        conflicts_with_all = ["unseal", "unseal_delete", "unseal_delete_force"],
        help_heading = SHARING
    )]
    seal: Option<PathBuf>,

    /// With --seal: seal your global settings in too (never your song). Whoever unseals it gets
    /// them in place of their own
    #[arg(long, global = true, requires = "seal", help_heading = SHARING)]
    with_settings: bool,

    /// Wear the theme in this theme.toml; tdl asks whether to keep wearing it
    #[arg(
        long,
        value_name = "FILE",
        global = true,
        value_parser = NonEmptyStringValueParser::new(),
        conflicts_with = "keep_theme",
        help_heading = SPLASH
    )]
    theme: Option<String>,

    /// Like --theme, and keep wearing it without asking
    #[arg(
        long,
        value_name = "FILE",
        global = true,
        value_parser = NonEmptyStringValueParser::new(),
        help_heading = SPLASH
    )]
    keep_theme: Option<String>,

    /// Play the splash screen in the look of this splash.toml, even if the splash setting skips
    /// it; tdl asks whether to keep it
    #[arg(
        long,
        value_name = "FILE",
        global = true,
        value_parser = NonEmptyStringValueParser::new(),
        conflicts_with = "keep_splash",
        help_heading = SPLASH
    )]
    splash: Option<String>,

    /// Like --splash, and keep it without asking
    #[arg(
        long,
        value_name = "FILE",
        global = true,
        value_parser = NonEmptyStringValueParser::new(),
        help_heading = SPLASH
    )]
    keep_splash: Option<String>,

    /// Go straight to the launcher
    #[arg(long, global = true, help_heading = SPLASH)]
    no_splash: bool,

    /// Show QUIP under the splash logo just this once. Every #N in it is built-in quip N (`#12`,
    /// or `#12, hun!`); `##` is a plain #
    #[arg(
        long,
        value_name = "QUIP",
        global = true,
        allow_hyphen_values = true,
        value_parser = quip,
        conflicts_with_all = ["no_splash", "no_quips"],
        help_heading = SPLASH
    )]
    splash_quip: Option<String>,

    /// Show no quips anywhere, just this once
    #[arg(long, global = true, help_heading = SPLASH)]
    no_quips: bool,

    /// List the built-in quips with their numbers, and exit
    #[arg(long, exclusive = true, help_heading = SPLASH)]
    list_quips: bool,

    /// With no windows, print what each controller button, trigger and stick sends, as the
    /// system names it, where tdl takes it to be and the key it makes, until Ctrl+C
    #[arg(long, exclusive = true)]
    controller_test: bool,

    /// Play SONG (a wav, mp3, flac or ogg file) instead of the built-in track
    #[arg(
        long,
        value_name = "SONG",
        global = true,
        value_parser = NonEmptyStringValueParser::new(),
        help_heading = SPLASH
    )]
    music: Option<String>,

    /// Keep the music playing after the splash screen
    #[arg(long, global = true, help_heading = SPLASH)]
    keep_music: bool,

    /// Play no music at all, just this once
    #[arg(
        long,
        global = true,
        conflicts_with_all = ["music", "keep_music"],
        help_heading = SPLASH
    )]
    no_music: bool,

    /// Nothing given on the command line is kept or asked about: the loadout, the wad dirs, UZDoom,
    /// the base wad, the ini, the theme and the splash are just for this run, and forgetting is too. A
    /// --keep- flag still keeps its own
    #[arg(long, global = true, help_heading = THIS_RUN)]
    session: bool,

    /// Play the pride splash: a fire that burns through every color of the rainbow, under the logo
    /// in the trans flag's
    #[arg(long, global = true, help_heading = THIS_RUN)]
    with_pride: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

/// Each does its one thing and then tdl closes; Ctrl+C stops the game.
#[derive(Subcommand, Debug)]
enum Commands {
    /// Play a profile
    Launch {
        /// The profile's name (case doesn't matter if only one fits)
        profile: String,
        /// No windows, only plain lines in the terminal
        #[arg(long)]
        headless: bool,
    },
    /// Open the host form for a profile, filled in from its saved host defaults and these
    Host {
        /// The profile's name (case doesn't matter if only one fits)
        profile: String,
        #[command(flatten)]
        game: HostArgs,
        /// No form and no windows, only plain lines in the terminal
        #[arg(long)]
        headless: bool,
    },
    /// Open the join form for a host, filled in with these, then find or download a profile
    Join {
        /// The host's IP address
        #[arg(value_name = "IP")]
        host: String,
        #[command(flatten)]
        game: JoinArgs,
        /// No form and no windows, only plain lines in the terminal (needs --code)
        #[arg(long, requires = "code")]
        headless: bool,
    },
    /// Keep what a TOML says (uzdoom, base_wad, ini, theme, splash, loadout, wads), with no
    /// windows, and close: how the installer points tdl at the UZDoom and base wad it put beside it
    Setup {
        /// The TOML [default: setup.toml in the working dir, or else beside tdl]
        #[arg(value_name = "TOML")]
        file: Option<PathBuf>,
    },
    /// Use the launcher from the command line: profiles, their wads, playing, the settings, the
    /// wad pools and sharing, with no windows
    Cli {
        #[command(subcommand)]
        rite: Rite,
    },
    /// Press keys and buttons from a script, with no splash and nothing drawn: each line is
    /// presses separated by spaces (keys like j, enter, shift+b, ctrl+c; buttons as the legends
    /// write them, like ⟨a⟩, ⟨lb+a⟩, ⟨rb+down⟩, or <lb+a>) or `wait 2s`, from SCRIPT or else typed
    /// on stdin
    Headless {
        /// The script, a text file; without it the lines are read from stdin
        #[arg(long, value_name = "SCRIPT")]
        script: Option<PathBuf>,
        /// Show each press, the screen that heard it and what it did there, and keep them in
        /// DIR/tdl.log (DIR is the working directory if not given), with everything UZDoom
        /// printed in DIR/uzdoom.log
        #[arg(long, value_name = "DIR", num_args = 0..=1, default_missing_value = ".")]
        log: Option<PathBuf>,
    },
}

/// How a game is hosted: `tdl host`'s flags, and `tdl cli host`'s.
#[derive(ClapArgs, Clone, Debug, PartialEq)]
pub struct HostArgs {
    /// How many players, 2 to 8
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u8).range(2..=8))]
    players: Option<u8>,
    /// Play cooperatively
    #[arg(long, conflicts_with = "deathmatch")]
    coop: bool,
    /// Play deathmatch
    #[arg(long)]
    deathmatch: bool,
    /// The map to start on, like MAP07 or E1M1
    #[arg(long, value_parser = map_name)]
    map: Option<String>,
    #[command(flatten)]
    connection: Connection,
    /// Serve the ini you host with too, for joiners who want to play with it
    #[arg(long, conflicts_with = "no_share_ini")]
    share_ini: bool,
    /// Keep your ini to yourself, even if the profile's host defaults share it
    #[arg(long)]
    no_share_ini: bool,
}

impl HostArgs {
    /// Hosting `profile` with these over its host defaults.
    pub fn command(self, profile: String) -> Command {
        Command::Host {
            profile,
            overrides: HostOverrides {
                players: self.players,
                deathmatch: (self.coop || self.deathmatch).then_some(self.deathmatch),
                map: self.map,
                port: self.connection.port,
                code: self.connection.code,
                share_ini: (self.share_ini || self.no_share_ini).then_some(self.share_ini),
            },
        }
    }
}

/// How a game is joined: `tdl join`'s flags, and `tdl cli join`'s.
#[derive(ClapArgs, Clone, Debug, PartialEq)]
pub struct JoinArgs {
    #[command(flatten)]
    connection: Connection,
    /// Download the wads that aren't installed without asking first
    #[arg(long)]
    always_download: bool,
    /// Play with the host's ini, if it shares it (your own player, controls, screen and sound
    /// stay yours)
    #[arg(long)]
    host_ini: bool,
    /// Like --host-ini, and add the host's key bindings on keys you haven't bound (never one
    /// that could reach a file or rebind keys)
    #[arg(long)]
    host_ini_with_keys: bool,
}

impl JoinArgs {
    /// Joining the game at `host` with these.
    pub fn command(self, host: String) -> Command {
        Command::Join {
            host,
            port: self.connection.port,
            code: self.connection.code,
            always_download: self.always_download,
            host_ini: match (self.host_ini, self.host_ini_with_keys) {
                (_, true) => HostIni::WithKeys,
                (true, false) => HostIni::Settings,
                (false, false) => HostIni::Yours,
            },
        }
    }
}

/// `tdl cli`: the launcher's work from the command line, one rite at a time. A PROFILE is a
/// profile's name (case doesn't matter if only one fits); a WAD is a wad's number in its profile's
/// load order (from 1) or its file name.
#[derive(Subcommand, Clone, Debug, PartialEq)]
pub enum Rite {
    /// List the profiles, in the launcher's order
    Profiles,
    /// Show a profile: its wads in load order, and what it plays with
    Show { profile: String },
    /// Make a profile with these wads, in load order (a folder is a mod folder)
    New {
        name: String,
        #[arg(id = "new_wads", value_name = "WAD")]
        wads: Vec<PathBuf>,
    },
    /// Rename a profile (its saves go with it)
    Rename { profile: String, name: String },
    /// Copy a profile, as NAME or else as its name tagged (brutal_2)
    Copy {
        profile: String,
        name: Option<String>,
    },
    /// Delete a profile and its saves, after asking
    Delete {
        profile: String,
        /// Don't ask
        #[arg(long)]
        yes: bool,
    },
    /// Change a profile's wads
    #[command(subcommand)]
    Wads(WadRite),
    /// Show what a profile plays with, or set its own: NAME VALUE pairs (uzdoom FILE, base-wad
    /// FILE, ini FILE, args ARGS), where the VALUE `global` puts the global one back
    Set {
        profile: String,
        #[arg(
            value_name = "NAME VALUE",
            num_args = 0..,
            trailing_var_arg = true,
            allow_hyphen_values = true
        )]
        changes: Vec<String>,
    },
    /// Play a profile
    Launch { profile: String },
    /// Host a profile, with its host defaults changed by these
    Host {
        profile: String,
        #[command(flatten)]
        game: HostArgs,
    },
    /// Join the game at IP (needs --code)
    Join {
        #[arg(value_name = "IP")]
        host: String,
        #[command(flatten)]
        game: JoinArgs,
    },
    /// Show the settings in use, or change them: NAME VALUE pairs (uzdoom FILE, base-wad FILE, ini
    /// FILE|none, args ARGS, theme NAME|FILE|default, splash NAME|FILE|default, splash-screen
    /// on|off, splash-quip WORDS|random, quips on|off, music off|splash|kept, song FILE|none,
    /// pride on|off)
    Settings {
        #[arg(
            value_name = "NAME VALUE",
            num_args = 0..,
            trailing_var_arg = true,
            allow_hyphen_values = true
        )]
        changes: Vec<String>,
    },
    /// The wad pools: the dirs wads are picked from and looked for in
    #[command(subcommand)]
    Pools(PoolRite),
    /// Seal profiles (all of them if none are named) into SCROLL, to share
    Seal {
        scroll: PathBuf,
        #[arg(value_name = "PROFILE")]
        profiles: Vec<String>,
        /// Seal your global settings in too (never your song)
        #[arg(long)]
        with_settings: bool,
    },
    /// Unseal a scroll's profiles into the loadout, looking for their wads in the wad pools
    Unseal {
        scroll: PathBuf,
        /// Take the global settings sealed in it, in place of yours
        #[arg(long)]
        with_settings: bool,
        /// Delete every profile in the loadout first (with their saves), after asking
        #[arg(long)]
        replace: bool,
        /// Don't ask
        #[arg(long)]
        yes: bool,
    },
    /// Make a profile that watches DIR: wads put in it are added, wads taken out of it leave
    Watch {
        dir: PathBuf,
        /// Its name, or else the dir's
        name: Option<String>,
    },
}

impl Rite {
    /// The rite a command phrase speaks: `wads add e1 "my mods/e1.wad"`, as `tdl cli` would take
    /// it, from a headless script's `` ` `` or the launcher's command line. `Err` is what is wrong
    /// with it, in clap's words but without the help it points at, which nobody there can ask for.
    pub fn from_phrase(phrase: &str) -> Result<Self, String> {
        let words = crate::ritual::rite_words(phrase)?;
        let args = Args::try_parse_from(["tdl", "cli"].map(String::from).into_iter().chain(words))
            .map_err(|error| {
                let said = error.to_string();
                said.lines()
                    .next()
                    .unwrap_or_default()
                    .trim_start_matches("error: ")
                    .to_string()
            })?;
        match args.command {
            Some(Commands::Cli { rite }) => Ok(rite),
            _ => Err(format!("{phrase} isn't a tdl cli command")),
        }
    }
}

/// `tdl cli wads`: a profile's wads.
#[derive(Subcommand, Clone, Debug, PartialEq)]
pub enum WadRite {
    /// Add wads (or mod folders), at the end or before wad --at N
    Add {
        profile: String,
        #[arg(id = "added_wads", value_name = "WAD", required = true)]
        wads: Vec<PathBuf>,
        /// Put them before this wad (from 1)
        #[arg(long, value_name = "N")]
        at: Option<usize>,
    },
    /// Take wads out of a profile (the files stay)
    Remove {
        profile: String,
        #[arg(id = "taken_wads", value_name = "WAD", required = true)]
        wads: Vec<String>,
    },
    /// Move a wad to place TO in the load order (from 1)
    Move {
        profile: String,
        wad: String,
        to: usize,
    },
    /// Put another file (or mod folder) in a wad's place
    Replace {
        profile: String,
        wad: String,
        file: PathBuf,
    },
    /// Mark a wad volatile (it may change, so its hash isn't held to), or not with --off
    Volatile {
        profile: String,
        wad: String,
        #[arg(long)]
        off: bool,
    },
}

/// `tdl cli pools`: the wad pools.
#[derive(Subcommand, Clone, Debug, PartialEq)]
pub enum PoolRite {
    /// List the wad pools: this run's, the ones kept for every loadout and for this one, and the
    /// downloads
    List,
    /// Keep DIR as a wad pool, for every loadout (or --this-loadout, this one)
    Add {
        dir: PathBuf,
        #[arg(long)]
        this_loadout: bool,
    },
    /// Stop keeping DIR as a wad pool
    Remove { dir: PathBuf },
}

/// Where a game is found: on both `host` and `join`.
#[derive(ClapArgs, Clone, Debug, PartialEq)]
struct Connection {
    /// The port the host serves its wads on [default: 8080]
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..))]
    port: Option<u16>,
    /// The join code, letters and digits (the host picks a random one if there is none)
    #[arg(long, value_parser = join_code)]
    code: Option<String>,
}

/// The help in the theme's colors (see [`theme::Help`]).
fn styles() -> Styles {
    let paint = |color: Color| {
        let color = match color {
            Color::Rgb(red, green, blue) => Some(styling::RgbColor(red, green, blue).into()),
            Color::Indexed(index) => Some(styling::Ansi256Color(index).into()),
            Color::Reset => None,
            named => Some(styling::Color::Ansi(ansi(named))),
        };
        styling::Style::new().fg_color(color)
    };
    let help = &theme::worn().help;
    Styles::styled()
        .header(paint(help.heading).bold())
        .usage(paint(help.heading).bold())
        .literal(paint(help.literal).bold())
        .placeholder(paint(help.placeholder))
        .error(paint(help.error).bold())
        .valid(paint(help.valid))
        .invalid(paint(help.invalid))
}

/// One of the terminal's sixteen named colors, as the help paints it.
fn ansi(color: Color) -> styling::AnsiColor {
    use styling::AnsiColor as Ansi;
    match color {
        Color::Black => Ansi::Black,
        Color::Red => Ansi::Red,
        Color::Green => Ansi::Green,
        Color::Yellow => Ansi::Yellow,
        Color::Blue => Ansi::Blue,
        Color::Magenta => Ansi::Magenta,
        Color::Cyan => Ansi::Cyan,
        Color::Gray => Ansi::White,
        Color::DarkGray => Ansi::BrightBlack,
        Color::LightRed => Ansi::BrightRed,
        Color::LightGreen => Ansi::BrightGreen,
        Color::LightYellow => Ansi::BrightYellow,
        Color::LightBlue => Ansi::BrightBlue,
        Color::LightMagenta => Ansi::BrightMagenta,
        Color::LightCyan => Ansi::BrightCyan,
        _ => Ansi::BrightWhite,
    }
}

/// A `--splash-quip`, with every `#N` replaced by its quip.
fn quip(given: &str) -> Result<String, String> {
    let expanded = quips::expand(given)?;
    if expanded.trim().is_empty() {
        return Err("a quip needs some words".to_string());
    }
    Ok(expanded)
}

/// A map name, the only kind the host form takes.
fn map_name(given: &str) -> Result<String, String> {
    if given.is_empty()
        || !given
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err("a map name is letters, digits and _, like MAP07 or E1M1".to_string());
    }
    Ok(given.to_string())
}

/// A join code, lowercase like the host form makes it.
fn join_code(given: &str) -> Result<String, String> {
    let code = given.to_ascii_lowercase();
    if !net::is_valid_code(&code) {
        return Err(format!(
            "a code is letters and digits, at most {}",
            net::MAX_CODE_LENGTH
        ));
    }
    Ok(code)
}

impl Cli {
    /// Reads the arguments after the program name. `Err` is clap's own message, help or version
    /// included, to be shown with [`clap::Error::exit`].
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, clap::Error> {
        let args = Args::try_parse_from(std::iter::once("tdl".to_string()).chain(args))?;
        if args.seal.is_some() && args.command.is_some() {
            return Err(Args::command().error(
                ErrorKind::ArgumentConflict,
                "--seal does its one thing, so it can't go with launch, host or join",
            ));
        }
        if args.seal.is_some() && matches!(args.command, Some(Commands::Setup { .. })) {
            return Err(Args::command().error(
                ErrorKind::ArgumentConflict,
                "--seal and setup each do their one thing, so they can't go together",
            ));
        }
        let mut headless = false;
        let mut play = None;
        let mut rite = None;
        let mut setup = None;
        let command = args.command.and_then(|command| match command {
            Commands::Headless { script, log } => {
                play = Some(Play { script, log });
                None
            }
            Commands::Launch {
                profile,
                headless: plain,
            } => {
                headless = plain;
                Some(Command::Launch { profile })
            }
            Commands::Host {
                profile,
                game,
                headless: plain,
            } => {
                headless = plain;
                Some(game.command(profile))
            }
            Commands::Join {
                host,
                game,
                headless: plain,
            } => {
                headless = plain;
                Some(game.command(host))
            }
            Commands::Cli { rite: given } => {
                rite = Some(given);
                None
            }
            Commands::Setup { file } => {
                setup = Some(file);
                None
            }
        });
        let session = args.session;
        // 🗝️ a --keep- flag gives its value the same as the plain one, and keeps it
        let passed = |plain: Option<String>, kept: Option<String>| {
            let keep = Keep::from_flags(kept.is_some(), session);
            (kept.or(plain), keep)
        };
        let (loadout, keep_loadout) = passed(args.loadout.or(args.dir), args.keep_loadout);
        let (uzdoom, keep_uzdoom) = passed(args.uzdoom, args.keep_uzdoom);
        let (base_wad, keep_base_wad) = passed(args.base_wad, args.keep_base_wad);
        let (ini, keep_ini) = passed(args.ini, args.keep_ini);
        let (theme, keep_theme) = passed(args.theme, args.keep_theme);
        let (splash, keep_splash) = passed(args.splash, args.keep_splash);
        let (wads, keep_wads) = if !args.keep_loadout_wads.is_empty() {
            (args.keep_loadout_wads, Keep::Loadout)
        } else if !args.keep_wads.is_empty() {
            (args.keep_wads, Keep::Global)
        } else {
            (args.wads, Keep::from_flags(false, session))
        };
        Ok(Self {
            loadout,
            keep_loadout,
            forget_loadout: Forget::from_flags(
                args.forget_loadout,
                args.keep_forget_loadout,
                session,
            ),
            wads,
            keep_wads,
            forget_wads: Forget::from_flags(args.forget_wads, args.keep_forget_wads, session),
            unseal_delete: if args.unseal_delete_force.is_some() {
                Some(Ask::Never)
            } else {
                args.unseal_delete.is_some().then_some(Ask::First)
            },
            unseal: args
                .unseal_delete_force
                .or(args.unseal_delete)
                .or(args.unseal),
            seal: args.seal.map(|scroll| Seal {
                scroll,
                with_settings: args.with_settings,
            }),
            list_quips: args.list_quips,
            controller_test: args.controller_test,
            setup,
            pride: args.with_pride,
            no_splash: args.no_splash,
            keep_music: args.keep_music,
            music: args.music,
            no_music: args.no_music,
            quip: args.splash_quip,
            no_quips: args.no_quips,
            uzdoom,
            keep_uzdoom,
            base_wad,
            keep_base_wad,
            ini,
            keep_ini,
            theme,
            keep_theme,
            splash,
            keep_splash,
            command,
            headless,
            play,
            rite,
        })
    }
}
