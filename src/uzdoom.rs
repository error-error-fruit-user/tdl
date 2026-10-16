use std::{
    ffi::OsStr,
    fs::File,
    io,
    ops::Deref,
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

use crate::{
    error::{Error, Result},
    vault::profile::Profile,
    vault::wad::Wad,
};

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
            .spawn()
    } else {
        command.spawn()
    }
    .map_err(Error::Spawn)?;
    if let Some(mut stdout) = doom.stdout.take() {
        // the log is best effort, but stdout must still be drained or the game blocks on a full pipe
        let _ = match File::create(data_subdir.join("session.log")) {
            Ok(mut log) => io::copy(&mut stdout, &mut log),
            Err(_) => io::copy(&mut stdout, &mut io::sink()),
        };
    }
    doom.wait()?;
    // keys pressed during the game would otherwise all land at once; tossing them is best effort,
    // since a terminal that can't be read (as under a test) has nothing to toss
    while let Ok(true) = ratatui::crossterm::event::poll(Duration::ZERO) {
        let _ = ratatui::crossterm::event::read();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_uzdoom_that_cannot_start_is_a_spawn_error_not_a_crash() {
        let result = execute(
            OsStr::new("/nowhere/at/all/uzdoom"),
            &Profile::default(),
            &[],
            Path::new("/nowhere"),
        );
        assert!(matches!(result, Err(Error::Spawn(_))));
    }

    /// Runs the real UZDoom (the flatpak) on exactly the arguments `uzdoom::launch` builds, with
    /// `-norun` so it loads everything, prints its startup log and exits, and with no display so
    /// no window can open. It needs flatpak and an IWAD, so it only runs when asked:
    ///
    /// ```sh
    /// TDL_REAL_IWAD=~/Downloads/freedoom-0.13.0/freedoom2.wad \
    ///     cargo test real_uzdoom -- --ignored --nocapture
    /// ```
    const HEADLESS_UZDOOM: &str = r#"#!/usr/bin/env bash
args=(-norun) files=() filesystems=()
while [ $# -gt 0 ]; do
    case "$1" in
        -iwad|-config) args+=("$1" @@ "$2" @@); shift 2 ;;
        -savedir) args+=("$1" "$2"); filesystems+=(--filesystem="$2"); shift 2 ;;
        -file) shift; files=("$@"); break ;;
        *) args+=("$1"); shift ;;
    esac
done
[ ${#files[@]} -gt 0 ] && args+=(-file @@ "${files[@]}" @@)
exec env -u DISPLAY -u WAYLAND_DISPLAY flatpak run --env=SDL_VIDEODRIVER=dummy \
    --file-forwarding "${filesystems[@]}" org.zdoom.UZDoom "${args[@]}"
"#;

    #[test]
    #[ignore = "runs the real UZDoom; see the doc comment"]
    fn real_uzdoom_loads_the_iwad_and_every_wad_in_order() {
        use std::{fs, os::unix::fs::PermissionsExt};

        use crate::vault::scribe::tests::Altar;

        let iwad = std::env::var("TDL_REAL_IWAD").expect("TDL_REAL_IWAD");
        let altar = Altar::new("real-uzdoom");

        // the smallest valid wad: a PWAD header with no lumps
        let empty_pwad = [b"PWAD".as_slice(), &0u32.to_le_bytes(), &12u32.to_le_bytes()].concat();
        let names = ["first.wad", "second.wad", "third.wad"];
        for name in names {
            fs::write(altar.file(name), &empty_pwad).unwrap();
        }
        // passes tdl's arguments into the sandbox: files are forwarded with @@ … @@ and the save
        // folder is opened up, the way a real wrapper has to
        let wrapper = altar.file("headless.sh");
        fs::write(&wrapper, HEADLESS_UZDOOM).unwrap();
        fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();

        let profile = Profile::new(
            "real".into(),
            names
                .iter()
                .map(|name| Wad::from_path(&altar.file(name)).unwrap())
                .collect(),
        );
        let data = altar.file("data");
        fs::create_dir_all(data.join("saves")).unwrap();
        launch(
            wrapper.as_os_str(),
            Some(OsStr::new(&iwad)),
            &profile,
            &data,
        )
        .unwrap();

        let log = fs::read_to_string(data.join("session.log")).unwrap();
        println!("{log}");
        let added: Vec<_> = log
            .lines()
            .filter_map(|line| line.strip_prefix("adding "))
            .collect();
        let position = |name: &str| added.iter().position(|line| line.contains(name));
        let iwad_name = Path::new(&iwad).file_name().unwrap().to_str().unwrap();
        let order: Vec<_> = [iwad_name, names[0], names[1], names[2]]
            .iter()
            .map(|name| position(name).unwrap_or_else(|| panic!("{name} was not loaded")))
            .collect();
        assert!(order.is_sorted(), "loaded out of order: {added:?}");
    }
}
