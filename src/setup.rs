//! `tdl setup [TOML]`: what to keep, from a TOML, with no windows: how the installer points tdl at
//! the UZDoom and the base wad it put beside it, and how anyone can set tdl up the same way twice.
//!
//! ```toml
//! uzdoom = "uzdoom/uzdoom.AppImage"
//! base_wad = "wads/base_wads/freedoom2.wad"
//! ini = "nacht.ini"
//! theme = "dracula"            # a name, or a theme.toml
//! splash = "my_splash.toml"    # a name, or a splash.toml
//! loadout = "~/doom/loadout"   # the loadout to open every time
//! remember_loadout = false     # ...or only to set up now, leaving the one tdl opens alone
//! wads = ["wads", "~/doom"]    # wad dirs, kept for every loadout
//! song = "songs/mine.wav"      # the song that plays ("none": the built-in track); tdl knows it
//! songs = ["songs/other.ogg"]  # more songs tdl knows, gone round in the settings
//!
//! [[profiles]]                 # made in the loadout, or, if it is there, its wads taken again
//! name = "Freedoom"
//! wads = []                    # files or mod folders, in load order
//! base_wad = "wads/base_wads/freedoom2.wad"   # and uzdoom, ini and args, each its own
//! place = 1                    # its place in the list (1 is first); else last, or where it was
//! ```
//!
//! Every key may be left out, and no other key is allowed. Paths are relative to the TOML's own
//! folder (`~/` is the home folder). Without a TOML named, it is `setup.toml` in the working
//! folder, or else beside tdl, or else in the `artifacts/` beside tdl.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{
    cli::{Cli, Keep},
    theme,
};

/// A profile a setup TOML makes (or takes the wads of again), with paths made whole.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub wads: Vec<String>,
    pub base_wad: Option<String>,
    pub uzdoom: Option<String>,
    pub ini: Option<String>,
    pub args: Option<String>,
    /// Where in the list it goes, from 1 (first); left out, a new profile goes last and one that
    /// was there stays where it was.
    pub place: Option<usize>,
}

/// What a setup TOML holds.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Setup {
    #[serde(default)]
    profiles: Vec<Profile>,
    remember_loadout: Option<bool>,
    uzdoom: Option<String>,
    base_wad: Option<String>,
    ini: Option<String>,
    theme: Option<String>,
    splash: Option<String>,
    loadout: Option<String>,
    #[serde(default)]
    wads: Vec<String>,
    song: Option<String>,
    #[serde(default)]
    songs: Vec<String>,
}

/// What a setup TOML does besides what it puts in the `Cli`: the profiles it makes and the songs
/// it keeps, their paths made whole.
#[derive(Debug, Default)]
pub struct Made {
    pub profiles: Vec<Profile>,
    /// The song to play: `Some(None)` for the built-in track.
    pub song: Option<Option<PathBuf>>,
    /// The songs tdl is to know (the song among them).
    pub songs: Vec<PathBuf>,
}

/// The setup TOML: `given`, or else `setup.toml` in the working folder, or else beside tdl or in
/// the `artifacts/` beside it (where install-tdl writes it).
fn find(given: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(given) = given {
        return Ok(given);
    }
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    std::iter::once(PathBuf::from("setup.toml"))
        .chain(beside.iter().flat_map(|dir| {
            [
                dir.join("setup.toml"),
                dir.join("artifacts").join("setup.toml"),
            ]
        }))
        .find(|path| path.is_file())
        .ok_or_else(|| {
            "There is no setup.toml in the working folder, beside tdl or in its artifacts: tdl \
             setup FILE names one"
                .into()
        })
}

/// `path` from the TOML, made whole: `~/` is the home folder, and anything else relative is
/// relative to `dir`, the TOML's folder.
fn whole(path: &str, dir: &Path) -> String {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
    let path = match (path.strip_prefix("~/"), home) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => dir.join(path),
    };
    path.display().to_string()
}

/// Reads the setup TOML (`given`, or the default one) into `cli`, as if each thing in it had been
/// given with its `--keep-` flag (the loadout only for this run with `remember_loadout = false`).
/// Returns where the TOML was, and what else it does ([`Made`]). `Err` says what is wrong with it,
/// a song that can't be played among it.
pub fn read(given: Option<PathBuf>, cli: &mut Cli) -> Result<(PathBuf, Made), String> {
    let path = find(given)?;
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    let setup: Setup = toml::from_str(&text).map_err(|error| {
        format!(
            "{} isn't a setup tdl can read: {}",
            path.display(),
            error.message()
        )
    })?;
    let dir = std::path::absolute(&path)
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    let keep = |value: Option<String>, field: &mut Option<String>, kept: &mut Keep| {
        if let Some(value) = value {
            *field = Some(whole(&value, &dir));
            *kept = Keep::Global;
        }
    };
    keep(setup.uzdoom, &mut cli.uzdoom, &mut cli.keep_uzdoom);
    keep(setup.base_wad, &mut cli.base_wad, &mut cli.keep_base_wad);
    keep(setup.ini, &mut cli.ini, &mut cli.keep_ini);
    keep(setup.loadout, &mut cli.loadout, &mut cli.keep_loadout);
    if setup.remember_loadout == Some(false) {
        cli.keep_loadout = Keep::Session;
    }
    // 👗 a theme or a splash may be a name instead of a file
    let dressing = |value: String| {
        if theme::dressing::is_name(&value) {
            value
        } else {
            whole(&value, &dir)
        }
    };
    if let Some(given) = setup.theme {
        cli.theme = Some(dressing(given));
        cli.keep_theme = Keep::Global;
    }
    if let Some(given) = setup.splash {
        cli.splash = Some(dressing(given));
        cli.keep_splash = Keep::Global;
    }
    if !setup.wads.is_empty() {
        cli.wads = setup
            .wads
            .iter()
            .map(|dir_given| whole(dir_given, &dir))
            .collect();
        cli.keep_wads = Keep::Global;
    }
    let profiles = setup
        .profiles
        .into_iter()
        .map(|profile| Profile {
            wads: profile.wads.iter().map(|wad| whole(wad, &dir)).collect(),
            base_wad: profile.base_wad.map(|wad| whole(&wad, &dir)),
            uzdoom: profile.uzdoom.map(|uzdoom| whole(&uzdoom, &dir)),
            ini: profile.ini.map(|ini| whole(&ini, &dir)),
            ..profile
        })
        .collect();
    let song_at = |given: &str| -> Result<PathBuf, String> {
        let song = PathBuf::from(whole(given, &dir));
        crate::music::check_song(&song)?;
        Ok(song)
    };
    let song = match setup.song.as_deref() {
        Some(none) if none.eq_ignore_ascii_case("none") => Some(None),
        Some(given) => Some(Some(song_at(given)?)),
        None => None,
    };
    let mut songs = setup
        .songs
        .iter()
        .map(|given| song_at(given))
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(Some(song)) = &song {
        songs.insert(0, song.clone());
    }
    Ok((
        path,
        Made {
            profiles,
            song,
            songs,
        },
    ))
}
