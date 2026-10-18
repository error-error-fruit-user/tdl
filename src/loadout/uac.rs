//! Settings that belong to no loadout: which loadout to open when none is passed, wad dirs every
//! loadout uses, UZDoom, the base wad, the ini and UZDoom's arguments, the theme, the splash, the
//! music, and the quips. Kept in `uac.rite` in the platform's standard config dir, the one place
//! every loadout can find. Before tdl 1.0.0 it was `global.rite`, which `upgrade_legacy.py`
//! upgrades.

use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    loadout::{
        edition::Edition,
        saved::{optional_path, paths},
        wad::Wad,
    },
    quips,
};

const FILE: &str = "uac.rite";

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Uac {
    /// The loadout to open when tdl starts without `--loadout`, kept from an earlier `--loadout`.
    #[serde(default, with = "optional_path")]
    pub loadout: Option<OsString>,
    /// Wad dirs kept from `--wads` for every loadout, used after the loadout's own.
    #[serde(default, with = "paths")]
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
    /// Path to the UZDoom executable, used by profiles that don't set their own.
    #[serde(default, with = "optional_path")]
    pub uzdoom: Option<OsString>,
    /// The base wad used by profiles that don't set their own, the same way.
    #[serde(default)]
    pub base_wad: Option<Wad>,
    /// Extra command-line arguments for UZDoom, split at spaces and passed on every launch, host
    /// and join, before the wads. Set in the settings.
    #[serde(default)]
    pub uzdoom_args: String,
    /// Goes straight to the launcher without the splash screen. Set in the settings.
    #[serde(default)]
    pub skip_splash: bool,
    /// The port the host served its wads on last time, offered again in the host form. `None`
    /// until a game is hosted on a port other than the default.
    #[serde(default)]
    pub host_port: Option<u16>,
    /// Plays the pride splash every time, as `--with-pride` does for one run. Set in the settings;
    /// missing in older files, which count as the usual splash.
    #[serde(default)]
    pub pride: bool,
    /// The UZDoom ini used by profiles that don't set their own, instead of UZDoom's usual one.
    /// Picked in the settings or kept from `--ini`.
    #[serde(default, with = "optional_path")]
    pub ini: Option<OsString>,
    /// The `theme.toml` tdl wears instead of its built-in theme. Picked in the settings or kept
    /// from `--theme`.
    #[serde(default, with = "optional_path")]
    pub theme: Option<OsString>,
    /// UZDoom from `--uzdoom`, for this session only: it is never saved, and it wins over every
    /// profile's own and the global one.
    #[serde(skip)]
    pub session_uzdoom: Option<OsString>,
    /// The base wad from `--base-wad`, for this session only, the same way.
    #[serde(skip)]
    pub session_base_wad: Option<Wad>,
    /// The ini from `--ini`, for this session only, the same way.
    #[serde(skip)]
    pub session_ini: Option<OsString>,
}

/// The global settings a scroll can carry to someone else: how tdl looks and sounds, and nothing
/// that names a place on this machine (the loadout, the wad dirs, the song).
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

impl Edition for Uac {
    /// Edition 1 was `global.rite`, the last before tdl 1.0.0; edition 2 calls its `vault` the
    /// `loadout`.
    const FIRST: u32 = 2;
    const UPGRADES: &[fn(Self) -> Self] = &[];
}

impl Uac {
    /// The UZDoom a profile without its own uses: this session's, else the global one.
    pub fn uzdoom_in_effect(&self) -> Option<&OsString> {
        self.session_uzdoom.as_ref().or(self.uzdoom.as_ref())
    }

    /// The base wad a profile without its own uses: this session's, else the global one.
    pub fn base_wad_in_effect(&self) -> Option<&Wad> {
        self.session_base_wad.as_ref().or(self.base_wad.as_ref())
    }

    /// The ini a profile without its own uses: this session's, else the global one, else none
    /// (UZDoom's usual one).
    pub fn ini_in_effect(&self) -> Option<&OsString> {
        self.session_ini.as_ref().or(self.ini.as_ref())
    }

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

    /// Saves to `path`, making its dir first: someone who only ever opens other loadouts may
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
    use crate::loadout::{edition::Loaded, scribe::tests::Altar};

    #[test]
    fn global_settings_save_and_load_back() {
        let altar = Altar::new("global-round-trip");
        // in a dir that doesn't exist yet
        let path = Uac::path(&altar.file("standard"));
        let global = Uac {
            loadout: Some("/home/me/doom".into()),
            wad_dirs: vec!["/wads".into(), "/mods".into()],
            splash_quip: SplashQuip::Custom("Rip and tear!".into()),
            no_quips: true,
            song: Some("/songs/e1m1.ogg".into()),
            music: Music::Kept,
            uzdoom: Some("/usr/bin/uzdoom".into()),
            base_wad: Some(Wad::new("/iwads/DOOM2.WAD".into(), "ab".repeat(32), 3)),
            uzdoom_args: "-nomusic".into(),
            skip_splash: true,
            host_port: Some(9000),
            ini: Some("/inis/mine.ini".into()),
            theme: Some("/themes/blue.toml".into()),
            ..Uac::default()
        };
        global.keep(&path).unwrap();
        assert_eq!(Uac::open(&path), Some(global.clone()));
        // this session's UZDoom and base wad never reach the file
        let session = Uac {
            session_uzdoom: Some("/test/uzdoom-dev".into()),
            session_base_wad: Some(Wad::new("/test/freedoom2.wad".into(), "cd".repeat(32), 1)),
            session_ini: Some("/test/session.ini".into()),
            ..global.clone()
        };
        session.keep(&path).unwrap();
        assert_eq!(Uac::open(&path), Some(global));
    }

    #[test]
    fn global_settings_from_before_tdl_1_are_legacy() {
        #[derive(Serialize)]
        struct Before {
            vault: Option<String>,
            wad_dirs: Vec<String>,
        }
        #[derive(Serialize)]
        struct Sealed {
            version: u32,
            data: Before,
        }
        let altar = Altar::new("uac-legacy");
        let path = Uac::path(&altar.0);
        let bytes = rmp_serde::to_vec_named(&Sealed {
            version: 1,
            data: Before {
                vault: Some("/home/me/doom".into()),
                wad_dirs: vec!["/wads".into()],
            },
        })
        .unwrap();
        fs::write(&path, bytes).unwrap();
        assert!(Uac::is_legacy(&path));
        assert_eq!(Uac::load(&path), Loaded::Legacy);
        assert_eq!(Uac::current(), 2);
    }

    #[test]
    fn sharing_leaves_out_the_song_and_everything_about_this_machine() {
        let mine = Uac {
            loadout: Some("/home/me/doom".into()),
            wad_dirs: vec!["/wads".into()],
            splash_quip: SplashQuip::Custom("#2, hun!".into()),
            no_quips: false,
            song: Some("/songs/e1m1.ogg".into()),
            music: Music::Kept,
            ..Uac::default()
        };
        let shared = mine.shared();
        let mut theirs = Uac {
            loadout: Some("/home/them/doom".into()),
            song: Some("/songs/theirs.mp3".into()),
            no_quips: true,
            ..Uac::default()
        };
        theirs.take_shared(shared.clone());
        assert_eq!(theirs.music, Music::Kept);
        assert!(!theirs.no_quips);
        assert_eq!(theirs.splash_quip, mine.splash_quip);
        // their own loadout and song are theirs to keep
        assert_eq!(theirs.loadout, Some("/home/them/doom".into()));
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
