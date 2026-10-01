use std::{
    io::{BufRead, BufReader, BufWriter, Write},
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
            .stderr(Stdio::piped())
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

pub fn launch(uzdoom: &str, profile: &Profile) -> Result<bool> {
    execute(uzdoom, profile, &[])
}

pub fn host(
    uzdoom: &str,
    profile: &Profile,
    player_count: &str,
    is_deathmatch: bool,
    map: &str,
) -> Result<bool> {
    let mut args = vec!["-host", player_count];
    if is_deathmatch {
        args.push("-deathmatch");
    }
    if !map.is_empty() {
        args.extend(["-warp", map]);
    }
    execute(uzdoom, profile, &args)
}

pub fn join(uzdoom: &str, profile: &Profile, ip: &str) -> Result<bool> {
    execute(uzdoom, profile, &["-join", ip])
}
