//! The command line:
//! `tdl [--vault <folder> | --vault-once <folder> | --forget-vault] [--wads <folder>]...
//! [--unseal <scroll>] [--no-splash] [--splash-quip <quip> | --no-quips]
//! [[--music <song>] [--keep-music] | --no-music]`, or `tdl --list-quips`.

use crate::quips;

pub const USAGE: &str = "\
Usage: tdl [--vault <folder> | --vault-once <folder> | --forget-vault]
           [--wads <folder>]... [--unseal <scroll>] [--no-splash]
           [--splash-quip <quip> | --no-quips]
           [[--music <song>] [--keep-music] | --no-music]
       tdl --list-quips

Options:
  --vault <folder>   Keep the config and data in <folder>/config and <folder>/data
                     (the folder on its own, `tdl <folder>`, still works).
                     tdl asks whether to open it every time
  --vault-once <folder>
                     Like --vault, but just for this run: never asks to open it
                     every time
  --forget-vault     Open the standard vault, and ask whether to stop opening the
                     one tdl remembered from --vault
  --wads <folder>    Where your wads are: file pickers open here, and joining looks
                     here for wads it already has. Give it more than once for more
                     folders. tdl asks whether to keep them
  --unseal <scroll>  Import a shared .scroll as a new profile as soon as tdl
                     starts, looking for its wads in the wad folders
  --no-splash        Go straight to the launcher
  --splash-quip <quip>
                     Show <quip> under the splash logo instead of a random one,
                     just this once (plays the splash even if the settings skip
                     it). Every #<n> in it is built-in quip n: `#12`, or
                     `#12, hun!` with your own words after it; `##` is a plain #
  --no-quips         Show no quips anywhere, just this once
  --list-quips       List the built-in quips with their numbers, and exit
  --music <song>     Play <song> (a wav, mp3, flac or ogg file) instead of the
                     built-in track
  --keep-music       Keep the music playing after the splash screen
  --no-music         Play no music at all, just this once
  -h, --help         Show this help";

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
}

impl Cli {
    /// Reads the arguments after the program name. The vault can be given as `--vault <folder>`,
    /// `--vault=<folder>`, or, as before flags existed, as the folder alone.
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut args = args.into_iter();
        let mut vault = None;
        let mut wads = Vec::new();
        let mut forget_vault = false;
        let mut vault_once = false;
        let mut unseal = None;
        let mut no_splash = false;
        let mut keep_music = false;
        let mut no_music = false;
        let mut music = None;
        let mut quip = None;
        let mut no_quips = false;
        while let Some(arg) = args.next() {
            if arg == "--no-quips" {
                no_quips = true;
                continue;
            }
            if arg == "--splash-quip" || arg.starts_with("--splash-quip=") {
                let given = match arg.strip_prefix("--splash-quip=") {
                    Some(given) => given.to_string(),
                    None => args.next().ok_or("--splash-quip needs a quip")?,
                };
                let expanded = quips::expand(&given)?;
                if expanded.trim().is_empty() {
                    return Err("--splash-quip needs a quip".to_string());
                }
                if quip.replace(expanded).is_some() {
                    return Err("Only one quip can be shown at a time".to_string());
                }
                continue;
            }
            if arg == "--no-splash" {
                no_splash = true;
                continue;
            }
            if arg == "--keep-music" {
                keep_music = true;
                continue;
            }
            if arg == "--no-music" {
                no_music = true;
                continue;
            }
            if arg == "--music" || arg.starts_with("--music=") {
                let song = match arg.strip_prefix("--music=") {
                    Some(song) => song.to_string(),
                    None => args.next().ok_or("--music needs a song")?,
                };
                if song.is_empty() {
                    return Err("--music needs a song".to_string());
                }
                if music.replace(song).is_some() {
                    return Err("Only one song can be played at a time".to_string());
                }
                continue;
            }
            if arg == "--unseal" || arg.starts_with("--unseal=") {
                let scroll = match arg.strip_prefix("--unseal=") {
                    Some(scroll) => scroll.to_string(),
                    None => args.next().ok_or("--unseal needs a scroll")?,
                };
                if unseal.replace(scroll).is_some() {
                    return Err("Only one scroll can be unsealed at a time".to_string());
                }
                continue;
            }
            if arg == "--forget-vault" {
                forget_vault = true;
                continue;
            }
            if arg == "--wads" {
                wads.push(args.next().ok_or("--wads needs a folder")?);
                continue;
            }
            if let Some(folder) = arg.strip_prefix("--wads=") {
                wads.push(folder.to_string());
                continue;
            }
            let folder = if arg == "--vault" {
                args.next().ok_or("--vault needs a folder")?
            } else if let Some(folder) = arg.strip_prefix("--vault=") {
                folder.to_string()
            } else if arg == "--vault-once" {
                vault_once = true;
                args.next().ok_or("--vault-once needs a folder")?
            } else if let Some(folder) = arg.strip_prefix("--vault-once=") {
                vault_once = true;
                folder.to_string()
            } else if arg.starts_with('-') {
                return Err(format!("Unknown option: {arg}"));
            } else {
                arg
            };
            if vault.replace(folder).is_some() {
                return Err("The vault folder was given more than once".to_string());
            }
        }
        if forget_vault && vault.is_some() {
            return Err(
                "--forget-vault opens the standard vault, so it can't go with a vault folder"
                    .to_string(),
            );
        }
        if no_splash && quip.is_some() {
            return Err(
                "The quip is shown on the splash screen, so it can't go with --no-splash"
                    .to_string(),
            );
        }
        if no_music && (keep_music || music.is_some()) {
            return Err("--no-music can't go with --music or --keep-music".to_string());
        }
        if no_quips && quip.is_some() {
            return Err("--no-quips can't go with a quip to show".to_string());
        }
        Ok(Self {
            vault,
            vault_once,
            forget_vault,
            wads,
            unseal,
            no_splash,
            keep_music,
            music,
            no_music,
            quip,
            no_quips,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, String> {
        Cli::parse(args.iter().map(|arg| arg.to_string()))
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
