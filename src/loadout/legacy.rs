//! Files from before tdl 1.0.0, which this build doesn't read: `upgrade_legacy.py`, which comes
//! with tdl, upgrades them all (see `DATA_LAYOUT.md`). Until it has, tdl says so and changes
//! nothing.

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::loadout::{
    config::Config, edition::Edition, profile::Profile, profile::SUBDIRECTORY, scribe,
    scroll::Scroll, uac::Uac,
};

/// The script that upgrades legacy files.
pub const SCRIPT: &str = "upgrade_legacy.py";
/// What the global settings were called before they were `uac.rite`.
const OLD_GLOBAL: &str = "global.rite";
/// What every file was called before `.rite`.
const OLD_EXTENSION: &str = "doom";
/// What the profiles dir was called before it had a dot.
const OLD_PROFILES: &str = "profiles";

/// Every legacy file (or dir) in the standard config dir and the open loadout's config dir: the
/// global settings as `global.rite` (unless `uac.rite` is there already), any file saved by an
/// edition older than the type's [`Edition::FIRST`], `.doom` files with no `.rite` beside them,
/// and a `profiles` dir with anything in it.
pub fn found(standard_config_dir: &Path, config_dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let old_global = standard_config_dir.join(OLD_GLOBAL);
    if old_global.exists() && !Uac::path(standard_config_dir).exists() {
        found.push(old_global);
    }
    let uac = Uac::path(standard_config_dir);
    if Uac::is_legacy(&uac) {
        found.push(uac);
    }
    let config = Config::path(config_dir);
    let old_config = config.with_extension(OLD_EXTENSION);
    if Config::is_legacy(&config) {
        found.push(config);
    } else if old_config.exists() && !config.exists() {
        found.push(old_config);
    }
    let old_profiles = config_dir.join(OLD_PROFILES);
    if fs::read_dir(&old_profiles).is_ok_and(|mut entries| entries.next().is_some()) {
        found.push(old_profiles);
    }
    let profiles = fs::read_dir(config_dir.join(SUBDIRECTORY))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| match path.extension().and_then(|e| e.to_str()) {
            // 🪦 beside a `.rite`, a `.doom` is only a leftover: the `.rite` is what is read
            Some(OLD_EXTENSION) => !path.with_extension(scribe::EXTENSION).exists(),
            Some(scribe::EXTENSION) => Profile::is_legacy(path),
            _ => false,
        });
    let mut profiles: Vec<_> = profiles.collect();
    profiles.sort();
    found.extend(profiles);
    found
}

/// What to tell the user about the legacy files `found`: what they are, and the command that
/// upgrades them, naming the loadout passed on the command line (`loadout`), if one was.
pub fn advice(found: &[PathBuf], loadout: Option<&Path>) -> Vec<String> {
    let mut command = format!("python3 {SCRIPT}");
    if let Some(root) = loadout {
        command += &format!(" --loadout {}", quoted(root));
    }
    let mut lines = vec![
        "Some of your files are from a tdl before 1.0.0, which this one doesn't read:".to_string(),
    ];
    lines.extend(found.iter().map(|path| format!("  {}", path.display())));
    lines.push(format!(
        "Upgrade them with {SCRIPT}, which comes with tdl (your originals are kept as \
         .legacy), then start tdl again: {command}"
    ));
    lines
}

/// What to tell the user about a legacy scroll at `path`.
pub fn scroll_advice(path: &Path) -> String {
    format!(
        "The scroll is from tdl 0.6.0: upgrade it with python3 {SCRIPT} --scroll {}",
        quoted(path)
    )
}

/// `path` as it can be typed into a shell.
fn quoted(path: &Path) -> String {
    let shown = path.display().to_string();
    if shown.contains([' ', '\'', '"']) {
        format!("'{}'", shown.replace('\'', r"'\''"))
    } else {
        shown
    }
}

/// Whether the scroll at `path` is a legacy one.
pub fn is_legacy_scroll(path: &Path) -> bool {
    Scroll::is_legacy(path)
}
