use std::{
    ffi::OsStr,
    io::{BufRead, BufReader, BufWriter, Write},
    path::Path,
    process::{Command, Stdio},
};

use crate::{error::Result, profile::Profile};

fn execute(uzdoom: &OsStr, profile: &Profile, args: &[&OsStr]) -> Result<()> {
    let mut doom = if !profile.mods.is_empty() {
        Command::new(uzdoom)
            .args(args)
            .arg("-file")
            .args(profile.mods.iter().map(|m| &m.path))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?
    } else {
        Command::new(uzdoom)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?
    };
    if let Some(stdout) = doom.stdout.take() {
        let reader = BufReader::new(stdout);
        let log = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open("session.log")?;
        let mut writer = BufWriter::new(log);
        for line in reader.lines() {
            writeln!(&mut writer, "{}", line.unwrap())?;
        }
        writer.flush()?;
    }
    Ok(())
}

pub fn launch(
    uzdoom: &OsStr,
    base_wad: Option<&OsStr>,
    profile: &Profile,
    data_subdir: &Path,
) -> Result<()> {
    let ini_path = data_subdir.join("uzdoom.ini");
    let saves_path = data_subdir.join("saves");
    let mut args = vec![
        OsStr::new("-config"),
        ini_path.as_os_str(),
        OsStr::new("-savedir"),
        saves_path.as_os_str(),
    ];
    if let Some(base_wad) = base_wad {
        args.extend([OsStr::new("-iwad"), base_wad]);
    }
    execute(uzdoom, profile, &args)
}

pub fn host(
    uzdoom: &OsStr,
    base_wad: Option<&OsStr>,
    profile: &Profile,
    player_count: &str,
    is_deathmatch: bool,
    map: &str,
    data_subdir: &Path,
) -> Result<()> {
    let ini_path = data_subdir.join("uzdoom.ini");
    let saves_path = data_subdir.join("saves");
    let mut args = vec![
        OsStr::new("-host"),
        OsStr::new(player_count),
        OsStr::new("-config"),
        ini_path.as_os_str(),
        OsStr::new("-savedir"),
        saves_path.as_os_str(),
    ];
    if let Some(base_wad) = base_wad {
        args.extend([OsStr::new("-iwad"), base_wad]);
    }
    if is_deathmatch {
        args.push(OsStr::new("-deathmatch"));
    }
    if !map.is_empty() {
        args.extend(["+map", map].map(OsStr::new));
    }
    execute(uzdoom, profile, &args)
}

pub fn join(
    uzdoom: &OsStr,
    base_wad: Option<&OsStr>,
    profile: &Profile,
    ip: &str,
    data_subdir: &Path,
) -> Result<()> {
    let ini_path = data_subdir.join("uzdoom.ini");
    let mut args = vec![
        OsStr::new("-join"),
        OsStr::new(ip),
        OsStr::new("-config"),
        ini_path.as_os_str(),
    ];
    if let Some(base_wad) = base_wad {
        args.extend([OsStr::new("-iwad"), base_wad]);
    }
    execute(uzdoom, profile, &args)
}
