use std::{
    ffi::OsStr,
    fs::File,
    io,
    ops::Deref,
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

use crate::{error::Result, profile::Profile, wad::Wad};

/// Runs UZDoom to completion, blocking the launcher. Output goes to `session.log` in `data_subdir`.
fn execute(uzdoom: &OsStr, profile: &Profile, args: &[&OsStr], data_subdir: &Path) -> Result<()> {
    let mut command = Command::new(uzdoom);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut doom = if !profile.wads.is_empty() {
        command
            .arg("-file")
            .args(profile.wads.iter().map(Wad::deref))
            .spawn()?
    } else {
        command.spawn()?
    };
    if let Some(mut stdout) = doom.stdout.take() {
        // the log is best effort, but stdout must still be drained or the game blocks on a full pipe
        let _ = match File::create(data_subdir.join("session.log")) {
            Ok(mut log) => io::copy(&mut stdout, &mut log),
            Err(_) => io::copy(&mut stdout, &mut io::sink()),
        };
    }
    doom.wait()?;
    while ratatui::crossterm::event::poll(Duration::ZERO)? {
        ratatui::crossterm::event::read()?;
    }
    Ok(())
}

/// Plays offline, with saves kept in the profile's own folder.
pub fn launch(
    uzdoom: &OsStr,
    base_wad: Option<&OsStr>,
    profile: &Profile,
    data_subdir: &Path,
) -> Result<()> {
    let saves_path = data_subdir.join("saves");
    let mut args = vec![OsStr::new("-savedir"), saves_path.as_os_str()];
    if let Some(base_wad) = base_wad {
        args.extend([OsStr::new("-iwad"), base_wad]);
    }
    if let Some(ini) = profile.ini.as_ref() {
        args.extend([OsStr::new("-config"), ini])
    }
    execute(uzdoom, profile, &args, data_subdir)
}

/// Starts a multiplayer game as host. An empty `map` lets UZDoom pick the starting map.
pub fn host(
    uzdoom: &OsStr,
    base_wad: Option<&OsStr>,
    profile: &Profile,
    player_count: &str,
    is_deathmatch: bool,
    map: &str,
    data_subdir: &Path,
) -> Result<()> {
    let saves_path = data_subdir.join("saves");
    let mut args = vec![
        OsStr::new("-host"),
        OsStr::new(player_count),
        OsStr::new("-savedir"),
        saves_path.as_os_str(),
    ];
    if let Some(base_wad) = base_wad {
        args.extend([OsStr::new("-iwad"), base_wad]);
    }
    if let Some(ini) = profile.ini.as_ref() {
        args.extend([OsStr::new("-config"), ini]);
    }
    if is_deathmatch {
        args.push(OsStr::new("-deathmatch"));
    }
    if !map.is_empty() {
        args.extend(["+map", map].map(OsStr::new));
    }
    execute(uzdoom, profile, &args, data_subdir)
}

/// Joins the game hosted at `ip`.
pub fn join(
    uzdoom: &OsStr,
    base_wad: Option<&OsStr>,
    profile: &Profile,
    ip: &str,
    data_subdir: &Path,
) -> Result<()> {
    let mut args = vec![OsStr::new("-join"), OsStr::new(ip)];
    if let Some(base_wad) = base_wad {
        args.extend([OsStr::new("-iwad"), base_wad]);
    }
    if let Some(ini) = profile.ini.as_ref() {
        args.extend([OsStr::new("-config"), ini]);
    }
    execute(uzdoom, profile, &args, data_subdir)
}
