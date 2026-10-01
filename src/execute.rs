use std::{
    io::{BufRead, BufReader, BufWriter, Write},
    path::Path,
    process::{Command, Stdio},
};

use crate::{error::Result, profile::Profile};

fn execute(uzdoom: &str, profile: &Profile, args: &[&str]) -> Result<bool> {
    if uzdoom.is_empty() {
        return Ok(false);
    }
    let mut doom = if !profile.mods.is_empty() {
        Command::new(uzdoom)
            .args(args)
            .arg("-file")
            .args(profile.mods.iter())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
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
    Ok(true)
}

pub fn launch(uzdoom: &str, profile: &Profile, data_subdir: &Path) -> Result<bool> {
    execute(
        uzdoom,
        profile,
        &[
            "-config",
            data_subdir.join("uzdoom.ini").to_str().unwrap(),
            "-savedir",
            data_subdir.join("saves").to_str().unwrap(),
        ],
    )
}

pub fn host(
    uzdoom: &str,
    profile: &Profile,
    player_count: &str,
    is_deathmatch: bool,
    map: &str,
    data_subdir: &Path,
) -> Result<bool> {
    let ini_path = data_subdir.join("uzdoom.ini");
    let saves_path = data_subdir.join("saves");
    let mut args = vec![
        "-host",
        player_count,
        "-config",
        ini_path.to_str().unwrap(),
        "-savedir",
        saves_path.to_str().unwrap(),
    ];
    if is_deathmatch {
        args.push("-deathmatch");
    }
    if !map.is_empty() {
        args.extend(["-warp", map]);
    }
    execute(uzdoom, profile, &args)
}

pub fn join(uzdoom: &str, profile: &Profile, ip: &str, data_subdir: &Path) -> Result<bool> {
    execute(
        uzdoom,
        profile,
        &[
            "-join",
            ip,
            "-config",
            data_subdir.join("uzdoom.ini").to_str().unwrap(),
        ],
    )
}
