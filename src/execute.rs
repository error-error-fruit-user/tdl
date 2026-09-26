use std::{
    io::{BufRead, BufReader, BufWriter, Write},
    path::Path,
    process::{Command, Stdio},
};

use crate::{error::Result, profile::Profile};

fn execute(uzdoom: &Path, profile: &Profile, args: &[&str]) -> Result<()> {
    let mut doom = if profile.mods.is_empty() {
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
    Ok(())
}

pub fn launch(uzdoom: &Path, profile: &Profile) -> Result<()> {
    execute(uzdoom, profile, &[])
}

pub fn host(
    uzdoom: &Path,
    profile: &Profile,
    player_count: &str,
    is_deathmatch: bool,
    map: &str,
) -> Result<()> {
    let mut args = vec!["-host", player_count];
    if is_deathmatch {
        args.push("-deathmatch");
    }
    if !map.is_empty() {
        args.extend(["-warp", map]);
    }
    execute(uzdoom, profile, &args)
}

pub fn join(uzdoom: &Path, profile: &Profile, ip: &str) -> Result<()> {
    execute(uzdoom, profile, &["-join", ip])
}
