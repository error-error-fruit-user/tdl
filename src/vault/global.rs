//! Settings that belong to no vault: which vault to open when none is passed, wad dirs every
//! vault uses, the music, and the quips. Kept in `global.rite` in the platform's standard config dir, the one place every
//! vault can find.

use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    quips,
    vault::{
        edition::Edition,
        saved::{optional_path, paths},
    },
};

const FILE: &str = "global.rite";

#[derive(Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Global {
    /// The vault to open when tdl starts without `--vault`, kept from an earlier `--vault`.
    #[serde(default, with = "optional_path")]
    pub vault: Option<OsString>,
    /// Wad dirs kept from `--wads` for every vault, used after the vault's own. Called
    /// `wad_folders` before edition 1.
    #[serde(default, alias = "wad_folders", with = "paths")]
    pub wad_dirs: Vec<OsString>,
    /// The quip under the splash logo whenever tdl starts, unless `--splash-quip` asks
    /// for another. Set in the settings (`O` in the launcher). Missing in older files, which count
    /// as a random one.
    #[serde(default)]
    pub splash_quip: SplashQuip,
    /// Leaves the quips out of every window: the splash screen and the waiting windows. Set in the
    /// settings; missing in older files, which count as showing them.
    #[serde(default)]
    pub no_quips: bool,
    /// Whether the music plays, and for how long. Set in the settings; missing in older files,
    /// which count as on the splash screen only.
    #[serde(default)]
    pub music: Music,
    /// A song of the player's own to play instead of the built-in track, unless `--music` gives
    /// another. Picked in the settings; missing in older files, which count as the built-in one.
    #[serde(default, with = "optional_path")]
    pub song: Option<OsString>,
}

/// The global settings a scroll can carry to someone else: how tdl looks and sounds, and nothing
/// that names a place on this machine (the vault, the wad dirs, the song).
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Shared {
    #[serde(default)]
    pub music: Music,
    #[serde(default)]
    pub no_quips: bool,
    #[serde(default)]
    pub splash_quip: SplashQuip,
}

impl Shared {
    /// The longest splash quip a scroll may carry, since it comes from someone else.
    pub const MAX_QUIP: usize = 120;

    /// Whether settings from someone else are fit to take: a splash quip of reasonable length with
    /// nothing in it that would garble the screen.
    pub fn check(&self) -> Result<(), &'static str> {
        let quip = self.splash_quip.typed();
        if quip.chars().count() > Self::MAX_QUIP || quip.chars().any(char::is_control) {
            return Err("The scroll's splash quip isn't one tdl can show");
        }
        Ok(())
    }

    /// One line saying what these settings are, for asking whether to take them.
    pub fn summary(&self) -> String {
        let quips = if self.no_quips { "hidden" } else { "shown" };
        let splash_quip = match &self.splash_quip {
            SplashQuip::Random => "random".to_string(),
            SplashQuip::Custom(message) => format!("'{message}'"),
        };
        format!(
            "music {}, quips {quips}, splash quip {splash_quip}",
            self.music.label()
        )
    }
}

/// Whether the music plays: never, on the splash screen only, or on past it in the launcher.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub enum Music {
    /// No music at all, as `--no-music` does for one run.
    Off,
    /// It fades out with the splash screen.
    #[default]
    Splash,
    /// It plays on in the launcher, as `--keep-music` does for one run.
    Kept,
}

impl Music {
    /// The next choice, wrapping around: off, splash only, keeps playing.
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Splash,
            Self::Splash => Self::Kept,
            Self::Kept => Self::Off,
        }
    }

    /// The choice before this one, wrapping around.
    pub fn previous(self) -> Self {
        self.next().next()
    }

    /// How the settings form shows it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Splash => "splash only",
            Self::Kept => "keeps playing",
        }
    }
}

/// Which quip the splash screen shows.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub enum SplashQuip {
    /// A different built-in one every time.
    #[default]
    Random,
    /// The user's own words, kept as typed: every `#<number>` in them stands for that built-in
    /// quip (see [`quips::expand`]).
    Custom(String),
}

impl SplashQuip {
    /// The quip to show, or `None` for a random one. A message with a number this tdl has no quip
    /// for (one saved by a tdl with other quips) is random too, rather than half a quip.
    pub fn text(&self) -> Option<String> {
        quips::expand(&self.typed())
            .ok()
            .filter(|quip| !quip.is_empty())
    }

    /// How the settings form shows it: empty for random, or the message as it was typed.
    pub fn typed(&self) -> String {
        match self {
            Self::Random => String::new(),
            Self::Custom(message) => message.clone(),
        }
    }

    /// What was typed in the settings form: nothing for random, or else a message, in which
    /// `#<number>` picks a built-in quip. `Err` says which number isn't one of the quips.
    pub fn from_typed(typed: &str) -> Result<Self, String> {
        let typed = typed.trim();
        if typed.is_empty() {
            return Ok(Self::Random);
        }
        quips::expand(typed)?;
        Ok(Self::Custom(typed.to_string()))
    }
}

impl Edition for Global {
    /// Born after editions existed, so edition 0 is already a named map, never a bare array.
    const UPGRADES: &[fn(Self) -> Self] = &[v0_to_v1];
}

/// Edition 1 calls `wad_folders` `wad_dirs`. The old name still reads (see [`Global::wad_dirs`]);
/// the new edition number keeps older builds, which would drop the field, from reading the file.
fn v0_to_v1(global: Global) -> Global {
    global
}

impl Global {
    /// The settings that can be shared in a scroll.
    pub fn shared(&self) -> Shared {
        Shared {
            music: self.music,
            no_quips: self.no_quips,
            splash_quip: self.splash_quip.clone(),
        }
    }

    /// Takes on settings shared in a scroll, leaving everything else (the song among it) as it was.
    pub fn take_shared(&mut self, shared: Shared) {
        self.music = shared.music;
        self.no_quips = shared.no_quips;
        self.splash_quip = shared.splash_quip;
    }

    /// Where the global settings are kept, given the standard config dir.
    pub fn path(standard_config_dir: &Path) -> PathBuf {
        standard_config_dir.join(FILE)
    }

    /// Saves to `path`, making its dir first: someone who only ever opens other vaults may
    /// never have had the standard config dir made.
    pub fn keep(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        self.save(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::scribe::tests::Altar;

    #[test]
    fn global_settings_save_and_load_back() {
        let altar = Altar::new("global-round-trip");
        // in a dir that doesn't exist yet
        let path = Global::path(&altar.file("standard"));
        let global = Global {
            vault: Some("/home/me/doom".into()),
            wad_dirs: vec!["/wads".into(), "/mods".into()],
            splash_quip: SplashQuip::Custom("Rip and tear!".into()),
            no_quips: true,
            song: Some("/songs/e1m1.ogg".into()),
            music: Music::Kept,
        };
        global.keep(&path).unwrap();
        assert_eq!(Global::open(&path), Some(global));
    }

    #[test]
    fn global_settings_from_before_the_quip_show_a_random_one() {
        #[derive(Serialize)]
        struct Before {
            vault: Option<String>,
            wad_folders: Vec<String>,
        }
        #[derive(Serialize)]
        struct Sealed {
            version: u32,
            data: Before,
        }
        let altar = Altar::new("global-before-quip");
        let path = Global::path(&altar.0);
        let bytes = rmp_serde::to_vec_named(&Sealed {
            version: 0,
            data: Before {
                vault: None,
                wad_folders: vec!["/wads".into()],
            },
        })
        .unwrap();
        fs::write(&path, bytes).unwrap();
        let global = Global::open(&path).unwrap();
        assert_eq!(global.splash_quip, SplashQuip::Random);
        assert!(!global.no_quips);
        assert_eq!(global.song, None);
        assert_eq!(global.music, Music::Splash);
        assert_eq!(global.wad_dirs, [OsString::from("/wads")]);
    }

    #[test]
    fn sharing_leaves_out_the_song_and_everything_about_this_machine() {
        let mine = Global {
            vault: Some("/home/me/doom".into()),
            wad_dirs: vec!["/wads".into()],
            splash_quip: SplashQuip::Custom("#2, hun!".into()),
            no_quips: false,
            song: Some("/songs/e1m1.ogg".into()),
            music: Music::Kept,
        };
        let shared = mine.shared();
        let mut theirs = Global {
            vault: Some("/home/them/doom".into()),
            song: Some("/songs/theirs.mp3".into()),
            no_quips: true,
            ..Global::default()
        };
        theirs.take_shared(shared.clone());
        assert_eq!(theirs.music, Music::Kept);
        assert!(!theirs.no_quips);
        assert_eq!(theirs.splash_quip, mine.splash_quip);
        // their own vault and song are theirs to keep
        assert_eq!(theirs.vault, Some("/home/them/doom".into()));
        assert_eq!(theirs.song, Some("/songs/theirs.mp3".into()));
        assert_eq!(
            shared.summary(),
            "music keeps playing, quips shown, splash quip '#2, hun!'"
        );
    }

    #[test]
    fn shared_settings_with_a_quip_that_would_garble_the_screen_are_refused() {
        let quip = |text: &str| Shared {
            splash_quip: SplashQuip::Custom(text.into()),
            ..Shared::default()
        };
        assert!(Shared::default().check().is_ok());
        assert!(quip("Rip and tear!").check().is_ok());
        assert!(quip("\x1b[2Jgotcha").check().is_err());
        assert!(quip(&"a".repeat(Shared::MAX_QUIP + 1)).check().is_err());
    }

    #[test]
    fn a_typed_quip_is_random_or_a_message_that_may_pick_quips() {
        assert_eq!(SplashQuip::from_typed("  "), Ok(SplashQuip::Random));
        assert_eq!(
            SplashQuip::from_typed(" #2, hun! "),
            Ok(SplashQuip::Custom("#2, hun!".into()))
        );
        assert!(SplashQuip::from_typed("#0").is_err());
        assert!(SplashQuip::from_typed(&format!("ok #{}", quips::QUIPS.len() + 1)).is_err());
        // and each shows the way it was typed
        for typed in ["", "#2", "Rip and tear!", "#1#2"] {
            assert_eq!(SplashQuip::from_typed(typed).unwrap().typed(), typed);
        }
    }

    #[test]
    fn the_quip_shown_has_its_numbers_replaced_or_is_none_for_random() {
        assert_eq!(SplashQuip::Random.text(), None);
        assert_eq!(
            SplashQuip::Custom("hi".into()).text().as_deref(),
            Some("hi")
        );
        assert_eq!(
            SplashQuip::Custom("#1message".into()).text(),
            Some(format!("{}message", quips::QUIPS[0]))
        );
        // a number alone is that quip, counting from 1
        assert_eq!(
            SplashQuip::Custom("#3".into()).text().as_deref(),
            Some(quips::QUIPS[2])
        );
        // a number past the end of this build's list falls back to random
        assert_eq!(SplashQuip::Custom(format!("#{}", u32::MAX)).text(), None);
        assert_eq!(SplashQuip::Custom("#99999 hi".into()).text(), None);
    }
}
