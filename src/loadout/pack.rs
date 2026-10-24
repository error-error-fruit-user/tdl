//! Packing a mod folder into a pk3, so a profile that plays a folder can be hosted: a joiner can
//! only download files. The packing is `pack_pk3.py`'s, which tdl carries inside it and writes
//! next to the pk3s it makes, so there is one set of packing rules and nothing to install but
//! Python.

use std::{
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

use crate::error::{Error, Result};

/// The packer, as this tdl knows it.
const SCRIPT: &str = include_str!("../../pack_pk3.py");
const SCRIPT_NAME: &str = "pack_pk3.py";

/// The Pythons to try, in order, until one can be started.
#[cfg(not(windows))]
const PYTHONS: &[&str] = &["python3", "python"];
#[cfg(windows)]
const PYTHONS: &[&str] = &["py", "python3", "python"];

/// The pk3 of the folder at `folder`, packed into `packed_dir` the first time and again whenever
/// it has gone stale: when anything in the folder is newer than the pk3, or the packer is (a newer
/// tdl packs differently, so its pk3s are made again). A fresh pk3 is used as it is.
///
/// Each folder gets a dir of its own, named from its path, so two folders of the same name never
/// share a pk3; the pk3 is named after the folder, which is the name joiners see.
pub fn packed(folder: &Path, packed_dir: &Path) -> Result<PathBuf> {
    let folder = fs::canonicalize(folder)?;
    let name = folder
        .file_name()
        .ok_or_else(|| Error::Pack(format!("{} has no name to pack it as", folder.display())))?;
    let key = blake3::hash(folder.as_os_str().as_encoded_bytes()).to_hex();
    let target = packed_dir.join(&key[..16]).join(pk3_name(name));
    let script = written_script(packed_dir)?;
    if !is_fresh(&folder, &target, &script) {
        run(&script, &folder, &target)?;
    }
    Ok(target)
}

/// `name` with `.pk3` on the end, unless it already has it, as `pack_pk3.py` names it.
fn pk3_name(name: &OsStr) -> String {
    let name = name.to_string_lossy();
    if name.to_lowercase().ends_with(".pk3") {
        name.into_owned()
    } else {
        format!("{name}.pk3")
    }
}

/// Where the packer is in `packed_dir`, written there if it isn't this tdl's. Writing it makes it
/// newer than every pk3 an older one made, which makes them stale.
fn written_script(packed_dir: &Path) -> io::Result<PathBuf> {
    let path = packed_dir.join(SCRIPT_NAME);
    if fs::read(&path).ok().as_deref() != Some(SCRIPT.as_bytes()) {
        fs::create_dir_all(packed_dir)?;
        fs::write(&path, SCRIPT)?;
    }
    Ok(path)
}

/// Whether `target` is there and newer than `script` and everything in `folder`. Anything that
/// can't be looked at counts as stale, so the packer gets to say what is wrong.
fn is_fresh(folder: &Path, target: &Path, script: &Path) -> bool {
    let packed = |path: &Path| fs::metadata(path).and_then(|metadata| metadata.modified());
    let Ok(target) = packed(target) else {
        return false;
    };
    packed(script).is_ok_and(|script| script <= target)
        && newest(folder).is_ok_and(|newest| newest <= target)
}

/// When anything in `dir` was last modified, the dirs included: removing a file changes its dir.
/// Dot files and dot dirs are skipped, as the packer skips them. Links are followed for their
/// times but never walked into, so a link can't lead the walk in circles.
fn newest(dir: &Path) -> io::Result<SystemTime> {
    let mut newest = fs::metadata(dir)?.modified()?;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let modified = if entry.file_type()?.is_dir() {
            self::newest(&entry.path())?
        } else {
            fs::metadata(entry.path())?.modified()?
        };
        newest = newest.max(modified);
    }
    Ok(newest)
}

/// Packs `folder` into `target` with the packer at `script`, using the first Python there is.
fn run(script: &Path, folder: &Path, target: &Path) -> Result<()> {
    let shown = folder
        .file_name()
        .unwrap_or(folder.as_os_str())
        .to_string_lossy();
    for python in PYTHONS {
        let output = match Command::new(python)
            .arg("-I")
            .arg(script)
            .arg(folder)
            .arg("--out")
            .arg(target)
            .output()
        {
            Ok(output) => output,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(Error::Pack(format!("Could not pack {shown}: {error}"))),
        };
        if output.status.success() {
            return Ok(());
        }
        // 🐍 the last line is the reason (or the end of a traceback)
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr.lines().rev().find(|line| !line.trim().is_empty());
        return Err(Error::Pack(match reason {
            Some(reason) => format!("Could not pack {shown}: {}", reason.trim()),
            None => format!("Could not pack {shown}"),
        }));
    }
    Err(Error::Pack(format!(
        "Hosting the folder {shown} packs it with Python 3, which wasn't found"
    )))
}
