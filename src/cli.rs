//! The command line:
//! `tdl [--vault <folder> | --vault-once <folder> | --forget-vault] [--wads <folder>]...
//! [--no-splash] [--music]`.

pub const USAGE: &str = "\
Usage: tdl [--vault <folder> | --vault-once <folder> | --forget-vault]
           [--wads <folder>]... [--no-splash] [--music]

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
  --no-splash        Go straight to the launcher
  --music            Keep the music playing after the splash screen
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
    /// Skips the splash screen.
    pub no_splash: bool,
    /// Keeps the music playing in the launcher, not just on the splash screen.
    pub music: bool,
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
        let mut no_splash = false;
        let mut music = false;
        while let Some(arg) = args.next() {
            if arg == "--no-splash" {
                no_splash = true;
                continue;
            }
            if arg == "--music" {
                music = true;
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
        Ok(Self {
            vault,
            vault_once,
            forget_vault,
            wads,
            no_splash,
            music,
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
    fn music_is_off_unless_asked_for() {
        assert!(!parse(&[]).unwrap().music);
        assert!(parse(&["--music"]).unwrap().music);
        assert!(parse(&["--music", "--vault", "/tmp/v"]).unwrap().music);
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
}
