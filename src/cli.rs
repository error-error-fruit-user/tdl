//! The command line, read by clap: `tdl [options] [folder]` opens the launcher, `tdl launch`,
//! `tdl host` and `tdl join` do one thing and close, and `tdl --seal` and `tdl --list-quips` do
//! theirs without any windows. `tdl --help` lists it all, in the theme's colors.

use std::path::PathBuf;

use clap::{
    Args as ClapArgs, CommandFactory, Parser, Subcommand,
    builder::{NonEmptyStringValueParser, Styles, styling},
    error::ErrorKind,
};
use ratatui::style::Color;

use crate::{
    net::{self, Address},
    quips, theme,
    vault::host_settings::HostSettings,
};

/// What was asked for on the command line.
#[derive(Debug, Default, PartialEq)]
pub struct Cli {
    /// Replaces the platform's standard config and data folders.
    pub vault: Option<String>,
    /// The vault came from `--vault-once`: open it without asking to keep it.
    pub vault_once: bool,
    /// Opens the standard vault, offering to forget the one remembered from `--vault`.
    pub forget_vault: bool,
    /// Folders the user keeps wads in, in the order given.
    pub wads: Vec<String>,
    /// A scroll to import on startup.
    pub unseal: Option<String>,
    /// Seals the whole vault into a scroll, without any windows, and maybe deletes the vault.
    pub seal: Option<Seal>,
    /// Lists the built-in quips and does nothing else.
    pub list_quips: bool,
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
    /// The UZDoom to play with for this run only, over every other.
    pub uzdoom: Option<String>,
    /// The base wad to play with for this run only, over every other.
    pub base_wad: Option<String>,
    /// `launch`, `host` or `join`: something to do straight away, after which tdl closes.
    pub command: Option<Command>,
    /// `--headless`: host or join with no forms or windows, only plain lines in the terminal.
    pub headless: bool,
}

/// `--seal`: the scroll to write the whole vault to, whether the global settings go in too
/// (`--with-settings`), and whether to delete the vault afterwards (`--delete`), without asking
/// first (`--force`).
#[derive(Debug, PartialEq)]
pub struct Seal {
    pub scroll: PathBuf,
    pub with_settings: bool,
    pub delete: bool,
    pub force: bool,
}

/// What `tdl launch`, `tdl host` and `tdl join` ask for. Each is done without the launcher, and
/// then tdl closes.
#[derive(Debug, PartialEq)]
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
    /// are downloaded without asking first.
    Join {
        host: String,
        port: Option<u16>,
        code: Option<String>,
        always_download: bool,
    },
}

/// The host settings `tdl host` was given, each over the profile's own default.
#[derive(Debug, Default, PartialEq)]
pub struct HostOverrides {
    pub players: Option<u8>,
    pub deathmatch: Option<bool>,
    pub map: Option<String>,
    pub port: Option<u16>,
    pub code: Option<String>,
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
const VAULT: &str = "Vault";
const SHARING: &str = "Sharing";
const SPLASH: &str = "Splash and music";
const THIS_RUN: &str = "Just for this run";

/// The arguments as clap reads them, turned into a [`Cli`] by [`Cli::parse`].
#[derive(Parser, Debug)]
#[command(
    name = "tdl",
    version,
    about = "A terminal launcher for UZDoom: profiles of wads, one key to play, and multiplayer \
             that hands joiners the host's wads",
    styles = styles(),
    max_term_width = 100
)]
struct Args {
    /// The vault folder on its own, as before --vault existed
    #[arg(value_name = "FOLDER", conflicts_with_all = ["vault", "vault_once", "forget_vault"])]
    folder: Option<String>,

    /// Keep the config and data in FOLDER/config and FOLDER/data; tdl asks whether to open it
    /// every time
    #[arg(long, value_name = "FOLDER", global = true, help_heading = VAULT)]
    vault: Option<String>,

    /// Like --vault, but just for this run: never asks to open it every time
    #[arg(
        long,
        value_name = "FOLDER",
        global = true,
        conflicts_with = "vault",
        help_heading = VAULT
    )]
    vault_once: Option<String>,

    /// Open the standard vault, and ask whether to stop opening the one remembered from --vault
    #[arg(
        long,
        global = true,
        conflicts_with_all = ["vault", "vault_once"],
        help_heading = VAULT
    )]
    forget_vault: bool,

    /// Where your wads are: file pickers open here, and joining looks here for wads it already
    /// has. Give it more than once for more folders; tdl asks whether to keep them
    #[arg(long, value_name = "FOLDER", global = true, help_heading = VAULT)]
    wads: Vec<String>,

    /// Import a shared .scroll as soon as tdl starts, looking for its wads in the wad folders.
    /// Global settings sealed in it replace yours
    #[arg(long, value_name = "SCROLL", global = true, help_heading = SHARING)]
    unseal: Option<String>,

    /// Seal every profile in the vault into SCROLL, with no windows, and close
    #[arg(
        long,
        value_name = "SCROLL",
        global = true,
        conflicts_with = "unseal",
        help_heading = SHARING
    )]
    seal: Option<PathBuf>,

    /// With --seal: seal your global settings in too (never your song). Whoever unseals it gets
    /// them in place of their own
    #[arg(long, global = true, requires = "seal", help_heading = SHARING)]
    with_settings: bool,

    /// With --seal: then delete the vault (its profiles, saved games and logs; never the
    /// downloaded wads or your global settings), after asking
    #[arg(long, global = true, requires = "seal", help_heading = SHARING)]
    delete: bool,

    /// With --delete: don't ask first
    #[arg(long, global = true, requires = "delete", help_heading = SHARING)]
    force: bool,

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

    /// Play with this UZDoom instead of every profile's own and the global one; nothing is kept
    #[arg(
        long,
        value_name = "FILE",
        global = true,
        value_parser = NonEmptyStringValueParser::new(),
        help_heading = THIS_RUN
    )]
    uzdoom: Option<String>,

    /// Play with this base wad, the same way
    #[arg(
        long,
        value_name = "FILE",
        global = true,
        value_parser = NonEmptyStringValueParser::new(),
        help_heading = THIS_RUN
    )]
    base_wad: Option<String>,

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
    },
    /// Open the host form for a profile, filled in from its saved host defaults and these
    Host {
        /// The profile's name (case doesn't matter if only one fits)
        profile: String,
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
        connection: Connection,
        /// Download the wads that aren't installed without asking first
        #[arg(long)]
        always_download: bool,
        /// No form and no windows, only plain lines in the terminal (needs --code)
        #[arg(long, requires = "code")]
        headless: bool,
    },
}

/// Where a game is found: on both `host` and `join`.
#[derive(ClapArgs, Debug)]
struct Connection {
    /// The port the host serves its wads on [default: 8080]
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..))]
    port: Option<u16>,
    /// The join code, letters and digits (the host picks a random one if there is none)
    #[arg(long, value_parser = join_code)]
    code: Option<String>,
}

/// The help in the theme's colors: hellfire headings, flame for what is typed as it is, bone for
/// what to fill in, and blood for what went wrong.
fn styles() -> Styles {
    let paint = |color: Color| {
        let Color::Rgb(red, green, blue) = color else {
            unreachable!("the palette is all RGB")
        };
        styling::Style::new().fg_color(Some(styling::RgbColor(red, green, blue).into()))
    };
    Styles::styled()
        .header(paint(theme::HELLFIRE).bold())
        .usage(paint(theme::HELLFIRE).bold())
        .literal(paint(theme::FLAME).bold())
        .placeholder(paint(theme::BONE))
        .error(paint(theme::BLOOD).bold())
        .valid(paint(theme::NUKAGE))
        .invalid(paint(theme::FLAME))
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
        let mut headless = false;
        let command = args.command.map(|command| match command {
            Commands::Launch { profile } => Command::Launch { profile },
            Commands::Host {
                profile,
                players,
                coop,
                deathmatch,
                map,
                connection,
                headless: plain,
            } => {
                headless = plain;
                Command::Host {
                    profile,
                    overrides: HostOverrides {
                        players,
                        deathmatch: (coop || deathmatch).then_some(deathmatch),
                        map,
                        port: connection.port,
                        code: connection.code,
                    },
                }
            }
            Commands::Join {
                host,
                connection,
                always_download,
                headless: plain,
            } => {
                headless = plain;
                Command::Join {
                    host,
                    port: connection.port,
                    code: connection.code,
                    always_download,
                }
            }
        });
        Ok(Self {
            vault_once: args.vault_once.is_some(),
            vault: args.vault_once.or(args.vault).or(args.folder),
            forget_vault: args.forget_vault,
            wads: args.wads,
            unseal: args.unseal,
            seal: args.seal.map(|scroll| Seal {
                scroll,
                with_settings: args.with_settings,
                delete: args.delete,
                force: args.force,
            }),
            list_quips: args.list_quips,
            pride: args.with_pride,
            no_splash: args.no_splash,
            keep_music: args.keep_music,
            music: args.music,
            no_music: args.no_music,
            quip: args.splash_quip,
            no_quips: args.no_quips,
            uzdoom: args.uzdoom,
            base_wad: args.base_wad,
            command,
            headless,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, String> {
        Cli::parse(args.iter().map(|arg| arg.to_string())).map_err(|error| error.to_string())
    }

    fn vault(folder: &str) -> Result<Cli, String> {
        Ok(Cli {
            vault: Some(folder.to_string()),
            ..Cli::default()
        })
    }

    #[test]
    fn no_arguments_uses_the_standard_folders() {
        assert_eq!(parse(&[]), Ok(Cli::default()));
    }

    #[test]
    fn the_vault_flag_takes_the_next_argument() {
        assert_eq!(parse(&["--vault", "/tmp/v"]), vault("/tmp/v"));
    }

    #[test]
    fn the_vault_flag_accepts_an_equals_sign() {
        assert_eq!(parse(&["--vault=/tmp/v"]), vault("/tmp/v"));
    }

    #[test]
    fn keeping_the_music_is_off_unless_asked_for() {
        assert!(!parse(&[]).unwrap().keep_music);
        assert!(parse(&["--keep-music"]).unwrap().keep_music);
        assert!(
            parse(&["--keep-music", "--vault", "/tmp/v"])
                .unwrap()
                .keep_music
        );
    }

    #[test]
    fn no_music_is_its_own_flag_and_cannot_go_with_any_music() {
        assert!(parse(&["--no-music"]).unwrap().no_music);
        assert!(!parse(&[]).unwrap().no_music);
        assert!(parse(&["--no-music", "--no-splash"]).unwrap().no_music);
        assert!(parse(&["--no-music", "--keep-music"]).is_err());
        assert!(parse(&["--music", "a.wav", "--no-music"]).is_err());
    }

    #[test]
    fn a_song_of_your_own_is_given_with_music() {
        let cli = parse(&["--music", "/songs/e1m1.mp3", "--keep-music"]).unwrap();
        assert_eq!(cli.music.as_deref(), Some("/songs/e1m1.mp3"));
        assert!(cli.keep_music);
        assert_eq!(
            parse(&["--music=/songs/e1m1.ogg"])
                .unwrap()
                .music
                .as_deref(),
            Some("/songs/e1m1.ogg")
        );
        // the song alone doesn't keep it playing past the splash
        assert!(!parse(&["--music", "a.wav"]).unwrap().keep_music);
        assert!(parse(&["--music"]).is_err());
        assert!(parse(&["--music="]).is_err());
        assert!(parse(&["--music", "a.wav", "--music", "b.wav"]).is_err());
        // the song is not mistaken for a vault folder
        assert_eq!(parse(&["--music", "a.wav"]).unwrap().vault, None);
    }

    #[test]
    fn the_folder_alone_still_works() {
        assert_eq!(parse(&["/tmp/v"]), vault("/tmp/v"));
    }

    #[test]
    fn a_missing_folder_is_an_error() {
        assert!(parse(&["--vault"]).is_err());
    }

    #[test]
    fn unknown_options_are_errors() {
        assert!(parse(&["--nope"]).is_err());
    }

    #[test]
    fn giving_the_vault_twice_is_an_error() {
        assert!(parse(&["--vault", "a", "--vault", "b"]).is_err());
        assert!(parse(&["a", "--vault", "b"]).is_err());
        assert!(parse(&["a", "b"]).is_err());
    }

    #[test]
    fn no_splash_works_alone_and_beside_the_vault() {
        let alone = Cli {
            no_splash: true,
            ..Cli::default()
        };
        assert_eq!(parse(&["--no-splash"]), Ok(alone));
        let both = Cli {
            vault: Some("/tmp/v".into()),
            no_splash: true,
            ..Cli::default()
        };
        assert_eq!(parse(&["--vault", "/tmp/v", "--no-splash"]), Ok(both));
        assert_eq!(
            parse(&["--no-splash", "/tmp/v"]).unwrap().vault,
            Some("/tmp/v".into())
        );
    }

    #[test]
    fn wad_folders_can_be_given_several_times_in_order() {
        let cli = parse(&["--wads", "/a", "--wads=/b", "--vault", "/v", "--wads", "/c"]).unwrap();
        assert_eq!(cli.wads, ["/a", "/b", "/c"]);
        assert_eq!(cli.vault, Some("/v".into()));
        assert!(parse(&["--wads"]).is_err());
        assert!(parse(&[]).unwrap().wads.is_empty());
    }

    #[test]
    fn forgetting_the_vault_is_its_own_flag_and_cannot_go_with_one() {
        assert!(parse(&["--forget-vault"]).unwrap().forget_vault);
        assert!(!parse(&[]).unwrap().forget_vault);
        assert!(parse(&["--forget-vault", "--vault", "/v"]).is_err());
        assert!(parse(&["/v", "--forget-vault"]).is_err());
    }

    #[test]
    fn a_vault_for_once_opens_the_same_but_is_marked_not_to_ask() {
        let once = parse(&["--vault-once", "/tmp/v"]).unwrap();
        assert_eq!(once.vault, Some("/tmp/v".into()));
        assert!(once.vault_once);
        assert!(parse(&["--vault-once=/tmp/v"]).unwrap().vault_once);
        assert!(!parse(&["--vault", "/tmp/v"]).unwrap().vault_once);
        assert!(parse(&["--vault-once"]).is_err());
        // still only one vault, and still not with forgetting
        assert!(parse(&["--vault-once", "a", "--vault", "b"]).is_err());
        assert!(parse(&["--vault-once", "a", "--forget-vault"]).is_err());
    }

    #[test]
    fn a_splash_quip_is_shown_as_given() {
        let cli = parse(&["--splash-quip", "Rip and tear, Tyler!", "--vault", "/v"]).unwrap();
        assert_eq!(cli.quip.as_deref(), Some("Rip and tear, Tyler!"));
        assert_eq!(cli.vault, Some("/v".into()));
        assert_eq!(
            parse(&["--splash-quip=Hi"]).unwrap().quip.as_deref(),
            Some("Hi")
        );
        // a quip that looks like a flag is still the quip
        let flaggy = parse(&["--splash-quip", "--keep-music"]).unwrap();
        assert_eq!(flaggy.quip.as_deref(), Some("--keep-music"));
        assert!(!flaggy.keep_music);
        assert_eq!(parse(&[]).unwrap().quip, None);
    }

    #[test]
    fn every_number_in_a_splash_quip_is_replaced_by_its_quip() {
        let first = quips::QUIPS[0];
        let picked = parse(&["--splash-quip", "#1"]).unwrap();
        assert_eq!(picked.quip.as_deref(), Some(first));
        let added = parse(&["--splash-quip=#1message"]).unwrap();
        assert_eq!(added.quip, Some(format!("{first}message")));
        let last = quips::QUIPS.len();
        assert_eq!(
            parse(&["--splash-quip", &format!("#{last}")])
                .unwrap()
                .quip
                .as_deref(),
            quips::QUIPS.last().copied()
        );
        for wrong in ["#0", &format!("#{}", last + 1), "ok #0"] {
            assert!(parse(&["--splash-quip", wrong]).is_err(), "{wrong:?}");
        }
    }

    #[test]
    fn only_one_quip_and_never_without_the_splash() {
        assert!(parse(&["--splash-quip"]).is_err());
        assert!(parse(&["--splash-quip", "  "]).is_err());
        assert!(parse(&["--splash-quip", "a", "--splash-quip", "b"]).is_err());
        assert!(parse(&["--splash-quip", "a", "--no-splash"]).is_err());
        assert!(parse(&["--no-splash", "--splash-quip", "#2"]).is_err());
        // the old flags are gone
        assert!(parse(&["--quip", "a"]).is_err());
        assert!(parse(&["--pick-quip", "2"]).is_err());
    }

    #[test]
    fn no_quips_is_its_own_flag_and_cannot_go_with_a_quip() {
        assert!(parse(&["--no-quips"]).unwrap().no_quips);
        assert!(!parse(&[]).unwrap().no_quips);
        // the splash can still be skipped too
        assert!(parse(&["--no-quips", "--no-splash"]).unwrap().no_quips);
        assert!(parse(&["--no-quips", "--splash-quip", "hi"]).is_err());
        assert!(parse(&["--splash-quip", "#2", "--no-quips"]).is_err());
    }

    #[test]
    fn a_uzdoom_and_a_base_wad_can_be_given_for_the_run() {
        let cli = parse(&[
            "--uzdoom",
            "/opt/uzdoom-dev",
            "--base-wad=/iwads/freedoom2.wad",
        ])
        .unwrap();
        assert_eq!(cli.uzdoom.as_deref(), Some("/opt/uzdoom-dev"));
        assert_eq!(cli.base_wad.as_deref(), Some("/iwads/freedoom2.wad"));
        // neither is mistaken for the vault folder
        assert_eq!(cli.vault, None);
        assert!(parse(&["--uzdoom"]).is_err());
        assert!(parse(&["--base-wad="]).is_err());
        assert!(parse(&["--uzdoom", "a", "--uzdoom", "b"]).is_err());
        assert!(parse(&["--base-wad", "a", "--base-wad=b"]).is_err());
        // a flag that only starts the same is still unknown
        assert!(parse(&["--uzdoomx"]).is_err());
        assert_eq!(parse(&[]).unwrap().uzdoom, None);
    }

    #[test]
    fn launch_names_a_profile_and_takes_the_other_options_too() {
        let cli = parse(&[
            "launch",
            "brutal_doom",
            "--no-splash",
            "--uzdoom",
            "/opt/uzdoom-dev",
        ])
        .unwrap();
        assert_eq!(
            cli.command,
            Some(Command::Launch {
                profile: "brutal_doom".into()
            })
        );
        assert_eq!(cli.uzdoom.as_deref(), Some("/opt/uzdoom-dev"));
        // the profile isn't taken for a vault folder
        assert_eq!(cli.vault, None);
        assert!(parse(&["launch"]).is_err());
        assert!(parse(&["launch", "--no-splash"]).is_err());
    }

    #[test]
    fn host_takes_its_settings_over_the_profiles_defaults() {
        let cli = parse(&[
            "host",
            "brutal_doom",
            "--players",
            "4",
            "--coop",
            "--map=MAP07",
            "--port",
            "9000",
            "--code",
            "Doom2",
        ])
        .unwrap();
        let Some(Command::Host { profile, overrides }) = cli.command else {
            panic!("expected host");
        };
        assert_eq!(profile, "brutal_doom");
        let settings = overrides.apply(HostSettings::new(None));
        assert_eq!(settings.player_count, "4");
        assert!(!settings.is_deathmatch);
        assert_eq!(settings.map, "MAP07");
        assert_eq!(settings.port, "9000");
        assert_eq!(settings.code, "doom2", "codes are lowercase");
        // with nothing given, the defaults stand
        let defaults = HostSettings {
            map: "E1M1".into(),
            ..HostSettings::new(Some(7000))
        };
        assert_eq!(HostOverrides::default().apply(defaults.clone()), defaults);
    }

    #[test]
    fn host_settings_that_could_not_be_typed_in_the_form_are_refused() {
        for wrong in [
            &["host", "p", "--players", "9"][..],
            &["host", "p", "--players", "one"],
            &["host", "p", "--coop", "--deathmatch"],
            &["host", "p", "--map", "../x"],
            &["host", "p", "--port", "0"],
            &["host", "p", "--port", "70000"],
            &["host", "p", "--code", "no spaces"],
        ] {
            assert!(parse(wrong).is_err(), "{wrong:?}");
        }
    }

    #[test]
    fn join_takes_the_host_and_maybe_a_port_and_code_for_the_form() {
        let cli = parse(&["join", "10.0.0.2", "--code", "abc234"]).unwrap();
        assert_eq!(
            cli.command,
            Some(Command::Join {
                host: "10.0.0.2".into(),
                port: None,
                code: Some("abc234".into()),
                always_download: false,
            })
        );
        let with_port = parse(&["join", "10.0.0.2", "--port=9000", "--code=abc234"]).unwrap();
        assert!(matches!(
            with_port.command,
            Some(Command::Join {
                port: Some(9000),
                ..
            })
        ));
        // the form asks for the code if it isn't given, but headless has no form to ask with
        assert!(parse(&["join", "10.0.0.2"]).is_ok());
        assert!(parse(&["join", "10.0.0.2", "--headless"]).is_err());
        assert!(parse(&["join"]).is_err());
        assert!(parse(&["join", "10.0.0.2", "--code", "abc", "--map", "MAP01"]).is_err());
    }

    #[test]
    fn a_join_address_has_the_default_port_and_needs_a_code() {
        let address = Command::address("10.0.0.2", None, Some("abc234")).unwrap();
        assert_eq!(address.port, net::DEFAULT_PORT);
        assert_eq!(
            Command::address("10.0.0.2", Some(9000), Some("x"))
                .unwrap()
                .port,
            9000
        );
        assert_eq!(Command::address("10.0.0.2", None, None), None);
    }

    #[test]
    fn always_download_goes_with_join_only() {
        let cli = parse(&["join", "10.0.0.2", "--code", "abc", "--always-download"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Join {
                always_download: true,
                ..
            })
        ));
        assert!(parse(&["host", "p", "--always-download"]).is_err());
        assert!(parse(&["--always-download"]).is_err());
    }

    #[test]
    fn headless_goes_with_host_and_join_only() {
        assert!(parse(&["host", "p", "--headless"]).unwrap().headless);
        assert!(
            parse(&["join", "10.0.0.2", "--code", "abc", "--headless"])
                .unwrap()
                .headless
        );
        assert!(!parse(&["host", "p"]).unwrap().headless);
        assert!(parse(&["launch", "p", "--headless"]).is_err());
        assert!(parse(&["--headless"]).is_err());
    }

    #[test]
    fn host_and_join_flags_only_go_with_their_commands() {
        assert!(parse(&["--players", "4"]).is_err());
        assert!(parse(&["--code", "abc"]).is_err());
        assert!(parse(&["launch", "p", "--port", "9000"]).is_err());
        assert_eq!(parse(&["/tmp/v"]).unwrap().command, None);
    }

    #[test]
    fn a_command_can_come_after_the_options_and_beside_a_vault_folder() {
        let cli = parse(&[
            "--vault-once",
            "/tmp/v",
            "--no-music",
            "launch",
            "brutal_doom",
        ])
        .unwrap();
        assert_eq!(
            cli.command,
            Some(Command::Launch {
                profile: "brutal_doom".into()
            })
        );
        assert_eq!(cli.vault, Some("/tmp/v".into()));
        // the old-style vault folder on its own still works next to a command
        let both = parse(&["/tmp/v", "host", "p", "--coop"]).unwrap();
        assert_eq!(both.vault, Some("/tmp/v".into()));
        assert!(matches!(both.command, Some(Command::Host { .. })));
        // tdl's own options can come after the command too
        let after = parse(&["host", "p", "--vault-once", "/tmp/v", "--no-music"]).unwrap();
        assert_eq!(after.vault, Some("/tmp/v".into()));
        assert!(after.no_music);
    }

    #[test]
    fn seal_takes_a_scroll_and_delete_and_force_only_go_with_it() {
        let cli = parse(&["--seal", "all.scroll", "--vault", "/v"]).unwrap();
        assert_eq!(
            cli.seal,
            Some(Seal {
                scroll: "all.scroll".into(),
                with_settings: false,
                delete: false,
                force: false,
            })
        );
        assert!(matches!(
            parse(&["--seal", "a", "--with-settings"]).unwrap().seal,
            Some(Seal {
                with_settings: true,
                ..
            })
        ));
        assert!(
            parse(&["--with-settings"]).is_err(),
            "with-settings needs seal"
        );
        assert_eq!(cli.vault, Some("/v".into()));
        let both = parse(&["--seal=all.scroll", "--delete", "--force"]).unwrap();
        assert!(matches!(
            both.seal,
            Some(Seal {
                delete: true,
                force: true,
                ..
            })
        ));
        assert!(parse(&["--delete"]).is_err(), "delete needs seal");
        assert!(
            parse(&["--seal", "a", "--force"]).is_err(),
            "force needs delete"
        );
        assert!(parse(&["--seal"]).is_err());
        assert!(parse(&["--seal", "a", "--unseal", "b"]).is_err());
        assert!(parse(&["--seal", "a", "launch", "p"]).is_err());
    }

    #[test]
    fn pride_can_be_worn_with_anything() {
        assert!(parse(&["--with-pride"]).unwrap().pride);
        assert!(!parse(&[]).unwrap().pride);
        assert!(parse(&["launch", "p", "--with-pride"]).unwrap().pride);
    }

    #[test]
    fn listing_the_quips_goes_alone() {
        assert!(parse(&["--list-quips"]).unwrap().list_quips);
        assert!(parse(&["--list-quips", "--no-splash"]).is_err());
    }

    #[test]
    fn help_and_version_are_answered_by_clap() {
        for (flag, kind) in [
            ("--help", ErrorKind::DisplayHelp),
            ("-h", ErrorKind::DisplayHelp),
            ("--version", ErrorKind::DisplayVersion),
            ("-V", ErrorKind::DisplayVersion),
        ] {
            let error = Cli::parse([flag.to_string()]).unwrap_err();
            assert_eq!(error.kind(), kind, "{flag}");
        }
    }

    #[test]
    fn the_command_line_is_put_together_soundly() {
        Args::command().debug_assert();
    }

    #[test]
    fn a_scroll_to_unseal_goes_with_the_wad_folders() {
        let cli = parse(&["--unseal", "shared.scroll", "--wads", "/w"]).unwrap();
        assert_eq!(cli.unseal, Some("shared.scroll".into()));
        assert_eq!(cli.wads, ["/w"]);
        assert_eq!(
            parse(&["--unseal=shared.scroll"]).unwrap().unseal,
            Some("shared.scroll".into())
        );
        assert!(parse(&["--unseal"]).is_err());
        assert!(parse(&["--unseal", "a.scroll", "--unseal", "b.scroll"]).is_err());
        // the scroll is not mistaken for a vault folder
        assert_eq!(parse(&["--unseal", "a.scroll"]).unwrap().vault, None);
    }
}
