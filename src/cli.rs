//! The command line: `tdl [--vault <folder>] [--no-splash] [--music]`.

pub const USAGE: &str = "\
Usage: tdl [--vault <folder>] [--no-splash] [--music]

Options:
  --vault <folder>   Keep the config and data in <folder>/config and <folder>/data
                     (the folder on its own, `tdl <folder>`, still works)
  --no-splash        Go straight to the launcher
  --music            Keep the music playing after the splash screen
  -h, --help         Show this help";

/// What was asked for on the command line.
#[derive(Debug, Default, PartialEq)]
pub struct Cli {
    /// Replaces the platform's standard config and data folders.
    pub vault: Option<String>,
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
            let folder = if arg == "--vault" {
                args.next().ok_or("--vault needs a folder")?
            } else if let Some(folder) = arg.strip_prefix("--vault=") {
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
        Ok(Self {
            vault,
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
}
