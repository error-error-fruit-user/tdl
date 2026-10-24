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
//! wads = ["wads", "~/doom"]    # wad dirs, kept for every loadout
//! ```
//!
//! Every key may be left out, and no other key is allowed. Paths are relative to the TOML's own
//! folder (`~/` is the home folder). Without a TOML named, it is `setup.toml` in the working
//! folder, or else beside tdl.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{
    cli::{Cli, Keep},
    theme,
};

/// What a setup TOML holds.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Setup {
    uzdoom: Option<String>,
    base_wad: Option<String>,
    ini: Option<String>,
    theme: Option<String>,
    splash: Option<String>,
    loadout: Option<String>,
    #[serde(default)]
    wads: Vec<String>,
}

/// The setup TOML: `given`, or else `setup.toml` in the working folder, or else beside tdl.
fn find(given: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(given) = given {
        return Ok(given);
    }
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("setup.toml")));
    [Some(PathBuf::from("setup.toml")), beside]
        .into_iter()
        .flatten()
        .find(|path| path.is_file())
        .ok_or_else(|| {
            "There is no setup.toml in the working folder or beside tdl: tdl setup FILE names one"
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
/// given with its `--keep-` flag. Returns where the TOML was. `Err` says what is wrong with it.
pub fn read(given: Option<PathBuf>, cli: &mut Cli) -> Result<PathBuf, String> {
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
    Ok(path)
}
