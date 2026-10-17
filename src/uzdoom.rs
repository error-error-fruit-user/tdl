use std::{
    ffi::OsStr,
    fs::File,
    io::{self, Read, Write},
    ops::Deref,
    path::Path,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::{
    error::{Error, Result},
    vault::host_settings::HostSettings,
    vault::profile::Profile,
    vault::wad::Wad,
};

/// How long a stopped UZDoom gets to go quietly before it is killed outright.
const GRACE: Duration = Duration::from_secs(2);

/// UZDoom while it runs. Its output is copied to `session.log` on a thread of its own, so whoever
/// started it is free to keep the screen moving, and the latest line is kept to show how far its
/// startup has got.
pub struct Game {
    child: Child,
    latest: Arc<Mutex<String>>,
    logger: Option<JoinHandle<()>>,
}

impl Game {
    /// The last line UZDoom printed, or empty if it hasn't printed one yet.
    pub fn latest_line(&self) -> String {
        self.latest
            .lock()
            .map(|line| line.clone())
            .unwrap_or_default()
    }

    /// Whether UZDoom has quit, without waiting for it.
    pub fn has_quit(&mut self) -> Result<bool> {
        Ok(self.child.try_wait()?.is_some())
    }

    /// Stops UZDoom now, for Ctrl+C. What tdl started may be a wrapper script (or flatpak) that
    /// runs the real game as a child of its own, so everything in the process group it was started
    /// in is asked to stop, and killed if it is still going after [`GRACE`]. Its log is finished
    /// once all of it is gone; tdl doesn't wait forever on anything that escaped the group.
    pub fn stop(mut self) -> Result<()> {
        let logger = self.logger.take();
        let gone = |game: &mut Self| -> Result<bool> {
            Ok(game.has_quit()? && logger.as_ref().is_none_or(JoinHandle::is_finished))
        };
        signal_group(&mut self.child, false);
        let deadline = Instant::now() + GRACE;
        while !gone(&mut self)? && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        if !gone(&mut self)? {
            signal_group(&mut self.child, true);
        }
        self.child.wait()?;
        // 📜 only joined if the output really closed: a straggler holding it would block tdl
        if let Some(logger) = logger
            && logger.is_finished()
        {
            let _ = logger.join();
        }
        Ok(())
    }

    /// Waits for UZDoom to quit and for the last of its output to reach the log.
    ///
    /// Keys pressed during the game would otherwise all land at once afterwards, so they are
    /// tossed. That is best effort, since a terminal that can't be read (as under a test) has
    /// nothing to toss.
    pub fn wait(mut self) -> Result<()> {
        self.child.wait()?;
        if let Some(logger) = self.logger.take() {
            let _ = logger.join();
        }
        while let Ok(true) = ratatui::crossterm::event::poll(Duration::ZERO) {
            let _ = ratatui::crossterm::event::read();
        }
        Ok(())
    }
}

/// Asks UZDoom's whole process group to stop, or with `kill` kills it outright. It may have quit
/// already, which is just as good.
#[cfg(unix)]
fn signal_group(child: &mut Child, kill: bool) {
    let signal = if kill { "KILL" } else { "TERM" };
    // 👥 the group stays while any of it is left, even once the one tdl started has gone
    let _ = Command::new("kill")
        .args(["-s", signal, "--", &format!("-{}", child.id())])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Stops UZDoom. Windows has no process groups to ask, and UZDoom there is the program itself.
#[cfg(not(unix))]
fn signal_group(child: &mut Child, _kill: bool) {
    let _ = child.kill();
}

/// The player's own extra arguments, from the settings: split at whitespace, with no quoting.
pub fn extra_args(text: &str) -> impl Iterator<Item = &str> {
    text.split_whitespace()
}

/// Starts UZDoom, with its output going to `session.log` in `data_subdir`. `extra` (the player's
/// own arguments) goes after tdl's `args` and before the wads, so `-file` stays last.
fn execute(
    uzdoom: &OsStr,
    profile: &Profile,
    args: &[&OsStr],
    extra: &str,
    data_subdir: &Path,
) -> Result<Game> {
    let mut command = Command::new(uzdoom);
    command
        .args(args)
        .args(extra_args(extra))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // 👥 a group of its own, so Ctrl+C can stop whatever it starts too (see [`Game::stop`])
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = if !profile.wads.is_empty() {
        command
            .arg("-file")
            .args(profile.wads.iter().map(Wad::deref))
            .spawn()
    } else {
        command.spawn()
    }
    .map_err(Error::Spawn)?;
    let latest = Arc::new(Mutex::new(String::new()));
    let logger = child.stdout.take().map(|stdout| {
        // the log is best effort, but stdout must still be drained or the game blocks on a full pipe
        let log = File::create(data_subdir.join("session.log")).ok();
        let latest = latest.clone();
        thread::spawn(move || copy_output(stdout, log, &latest))
    });
    Ok(Game {
        child,
        latest,
        logger,
    })
}

/// Copies everything from `output` into `log` (if there is one) until UZDoom closes it, keeping
/// the last non-blank line in `latest` as it goes.
fn copy_output(mut output: impl Read, mut log: Option<File>, latest: &Mutex<String>) {
    let mut buffer = [0; 8 * 1024];
    let mut line = Vec::new();
    loop {
        let read = match output.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        if let Some(file) = &mut log
            && file.write_all(&buffer[..read]).is_err()
        {
            log = None;
        }
        for &byte in &buffer[..read] {
            if byte == b'\n' || byte == b'\r' {
                let text = String::from_utf8_lossy(&line).trim().to_string();
                if !text.is_empty()
                    && let Ok(mut latest) = latest.lock()
                {
                    *latest = text;
                }
                line.clear();
            } else {
                line.push(byte);
            }
        }
    }
}

/// Plays offline, with saves kept in the profile's own folder.
pub fn launch(
    uzdoom: &OsStr,
    base_wad: Option<&OsStr>,
    profile: &Profile,
    extra: &str,
    data_subdir: &Path,
) -> Result<Game> {
    let saves_path = data_subdir.join("saves");
    let mut args = vec![OsStr::new("-savedir"), saves_path.as_os_str()];
    if let Some(base_wad) = base_wad {
        args.extend([OsStr::new("-iwad"), base_wad]);
    }
    if let Some(ini) = profile.ini.as_ref() {
        args.extend([OsStr::new("-config"), ini])
    }
    execute(uzdoom, profile, &args, extra, data_subdir)
}

/// Starts a multiplayer game as host, with the player count, mode and map from `settings`. An
/// empty map lets UZDoom pick the starting map.
pub fn host(
    uzdoom: &OsStr,
    base_wad: Option<&OsStr>,
    profile: &Profile,
    settings: &HostSettings,
    extra: &str,
    data_subdir: &Path,
) -> Result<Game> {
    let saves_path = data_subdir.join("saves");
    let mut args = vec![
        OsStr::new("-host"),
        OsStr::new(&settings.player_count),
        OsStr::new("-savedir"),
        saves_path.as_os_str(),
    ];
    if let Some(base_wad) = base_wad {
        args.extend([OsStr::new("-iwad"), base_wad]);
    }
    if let Some(ini) = profile.ini.as_ref() {
        args.extend([OsStr::new("-config"), ini]);
    }
    if settings.is_deathmatch {
        args.push(OsStr::new("-deathmatch"));
    }
    if !settings.map.is_empty() {
        args.extend(["+map", &settings.map].map(OsStr::new));
    }
    execute(uzdoom, profile, &args, extra, data_subdir)
}

/// Joins the game hosted at `ip`.
pub fn join(
    uzdoom: &OsStr,
    base_wad: Option<&OsStr>,
    profile: &Profile,
    ip: &str,
    extra: &str,
    data_subdir: &Path,
) -> Result<Game> {
    let mut args = vec![OsStr::new("-join"), OsStr::new(ip)];
    if let Some(base_wad) = base_wad {
        args.extend([OsStr::new("-iwad"), base_wad]);
    }
    if let Some(ini) = profile.ini.as_ref() {
        args.extend([OsStr::new("-config"), ini]);
    }
    execute(uzdoom, profile, &args, extra, data_subdir)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn a_uzdoom_that_cannot_start_is_a_spawn_error_not_a_crash() {
        let result = execute(
            OsStr::new("/nowhere/at/all/uzdoom"),
            &Profile::default(),
            &[],
            "",
            Path::new("/nowhere"),
        );
        assert!(matches!(result, Err(Error::Spawn(_))));
    }

    /// Writes `body` to `path` as a script that can be run.
    #[cfg(unix)]
    fn script(path: &Path, body: &str) {
        use std::{fs, os::unix::fs::PermissionsExt};

        fs::write(path, body).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// Runs `start` until it gets past "text file busy" (ETXTBSY). Linux won't run a file that is
    /// open for writing anywhere, and a script that was just written still is for a moment if
    /// another test thread forked meanwhile: its child holds a copy of the handle until it execs.
    /// That clears up in moments, so retrying is enough.
    #[cfg(unix)]
    fn past_busy<T>(mut start: impl FnMut() -> Result<T>) -> T {
        const TEXT_FILE_BUSY: i32 = 26;
        for _ in 0..100 {
            match start() {
                Err(Error::Spawn(error)) if error.raw_os_error() == Some(TEXT_FILE_BUSY) => {
                    thread::sleep(Duration::from_millis(10));
                }
                other => return other.unwrap_or_else(|error| panic!("{error}")),
            }
        }
        panic!("the script stayed busy");
    }

    /// Runs the real UZDoom (the flatpak) on exactly the arguments `uzdoom::launch` builds, with
    /// `-norun` so it loads everything, prints its startup log and exits, and with no display so
    /// no window can open. It needs flatpak and an IWAD, so it only runs when asked:
    ///
    /// ```sh
    /// TDL_REAL_IWAD=~/Downloads/freedoom-0.13.0/freedoom2.wad \
    ///     cargo test real_uzdoom -- --ignored --nocapture
    /// ```
    ///
    /// The same check runs on every `cargo test` against [`DUMMY_UZDOOM`] instead.
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

    /// Stands in for UZDoom without needing it installed. It reads arguments the way UZDoom does,
    /// checks every file is really a wad, and prints the `adding <file>, <n> lumps` lines of
    /// UZDoom's startup log in load order (engine files, the IWAD, then `-file`), so the test
    /// below works the same against either one. It fails like UZDoom would on a missing or broken
    /// file, a missing save folder, or a `-file` with nothing after it.
    const DUMMY_UZDOOM: &str = r#"#!/usr/bin/env bash
echo "UZDoom version 0.0.0 (tdl's dummy)"
iwad= config= savedir= files=() seen_file=
while [ $# -gt 0 ]; do
    case "$1" in
        -iwad) iwad="$2"; shift 2 ;;
        -config) config="$2"; shift 2 ;;
        -savedir) savedir="$2"; shift 2 ;;
        -file) seen_file=1; shift; while [ $# -gt 0 ] && [[ "$1" != -* && "$1" != +* ]]; do
            files+=("$1"); shift; done ;;
        -host|-join|+map) shift 2 ;;
        *) shift ;;
    esac
done
fail() { echo "$1" >&2; exit 1; }
[ -n "$seen_file" ] && [ ${#files[@]} -eq 0 ] && fail "-file with no files"
[ -n "$savedir" ] && [ ! -d "$savedir" ] && fail "save folder $savedir does not exist"
[ -n "$config" ] && [ ! -f "$config" ] && fail "config $config does not exist"
add() { # a wad's file, and the header it must have
    [ -f "$1" ] || fail "Could not find $1"
    local magic; magic=$(head -c 4 "$1")
    [[ " $2 " == *" $magic "* ]] || fail "$1 is not a wad"
    echo "adding $1, $(od -An -tu4 -j4 -N4 "$1" | tr -d ' ') lumps"
}
echo "W_Init: Init WADfiles."
echo "adding /dummy/uzdoom.pk3, 734 lumps"
[ -n "$iwad" ] && add "$iwad" IWAD
for file in "${files[@]}"; do add "$file" "PWAD IWAD"; done
"#;

    /// Runs the profile's wads through `uzdoom` exactly as [`launch`] passes them, then checks
    /// from its log that the IWAD and every wad were loaded, in order.
    #[cfg(unix)]
    fn loads_the_iwad_and_every_wad_in_order(name: &str, uzdoom: &str, iwad: PathBuf) {
        use std::fs;

        use crate::vault::scribe::tests::Altar;

        let altar = Altar::new(name);

        // the smallest valid wad: a PWAD header with no lumps
        let empty_pwad = [
            b"PWAD".as_slice(),
            &0u32.to_le_bytes(),
            &12u32.to_le_bytes(),
        ]
        .concat();
        let names = ["first.wad", "second.wad", "third.wad"];
        for name in names {
            fs::write(altar.file(name), &empty_pwad).unwrap();
        }
        let wrapper = altar.file("uzdoom.sh");
        script(&wrapper, uzdoom);

        let profile = Profile::new(
            "real".into(),
            names
                .iter()
                .map(|name| Wad::from_path(&altar.file(name)).unwrap())
                .collect(),
        );
        let data = altar.file("data");
        fs::create_dir_all(data.join("saves")).unwrap();
        past_busy(|| {
            launch(
                wrapper.as_os_str(),
                Some(iwad.as_os_str()),
                &profile,
                "",
                &data,
            )
        })
        .wait()
        .unwrap();

        let log = fs::read_to_string(data.join("session.log")).unwrap();
        println!("{log}");
        let added: Vec<_> = log
            .lines()
            .filter_map(|line| line.strip_prefix("adding "))
            .collect();
        let position = |name: &str| added.iter().position(|line| line.contains(name));
        let iwad_name = iwad.file_name().unwrap().to_str().unwrap();
        let order: Vec<_> = [iwad_name, names[0], names[1], names[2]]
            .iter()
            .map(|name| position(name).unwrap_or_else(|| panic!("{name} was not loaded")))
            .collect();
        assert!(order.is_sorted(), "loaded out of order: {added:?}");
    }

    #[test]
    #[cfg(unix)]
    fn the_dummy_uzdoom_loads_the_iwad_and_every_wad_in_order() {
        use crate::vault::scribe::tests::Altar;

        // an IWAD of two empty lumps' worth of header, which is all the dummy looks at
        let altar = Altar::new("dummy-iwad");
        let iwad = altar.file("dummy2.wad");
        let header = [
            b"IWAD".as_slice(),
            &2u32.to_le_bytes(),
            &12u32.to_le_bytes(),
        ]
        .concat();
        std::fs::write(&iwad, header).unwrap();
        loads_the_iwad_and_every_wad_in_order("dummy-uzdoom", DUMMY_UZDOOM, iwad);
    }

    #[test]
    #[cfg(unix)]
    fn the_dummy_uzdoom_fails_like_uzdoom_on_a_file_that_is_not_a_wad() {
        use std::fs;

        use crate::vault::scribe::tests::Altar;

        let altar = Altar::new("dummy-uzdoom-bad");
        let wrapper = altar.file("uzdoom.sh");
        script(&wrapper, DUMMY_UZDOOM);
        fs::write(altar.file("notes.wad"), b"not a wad at all").unwrap();
        let status = past_busy(|| {
            Command::new(&wrapper)
                .arg("-file")
                .arg(altar.file("notes.wad"))
                .stderr(Stdio::null())
                .status()
                .map_err(Error::Spawn)
        });
        assert!(!status.success());
    }

    #[test]
    #[cfg(unix)]
    #[ignore = "runs the real UZDoom; see the doc comment"]
    fn real_uzdoom_loads_the_iwad_and_every_wad_in_order() {
        let iwad = std::env::var("TDL_REAL_IWAD").expect("TDL_REAL_IWAD");
        // passes tdl's arguments into the sandbox: files are forwarded with @@ … @@ and the save
        // folder is opened up, the way a real wrapper has to
        loads_the_iwad_and_every_wad_in_order("real-uzdoom", HEADLESS_UZDOOM, iwad.into());
    }

    #[test]
    #[cfg(unix)]
    fn extra_arguments_go_after_tdls_own_and_before_the_wads() {
        use std::fs;

        use crate::vault::scribe::tests::Altar;

        let altar = Altar::new("uzdoom-extra-args");
        let echo = altar.file("uzdoom.sh");
        script(
            &echo,
            "#!/bin/sh\nfor arg in \"$@\"; do echo \"$arg\"; done\n",
        );
        let profile = Profile::new(
            "p".into(),
            vec![Wad::new("/wads/a.wad".into(), String::new(), 0)],
        );
        let data = altar.file("data");
        fs::create_dir_all(&data).unwrap();
        past_busy(|| {
            launch(
                echo.as_os_str(),
                None,
                &profile,
                "  -nomusic   +sv_cheats 1 ",
                &data,
            )
        })
        .wait()
        .unwrap();
        let log = fs::read_to_string(data.join("session.log")).unwrap();
        let args: Vec<_> = log.lines().collect();
        let saves = data.join("saves");
        assert_eq!(
            args,
            [
                "-savedir",
                saves.to_str().unwrap(),
                "-nomusic",
                "+sv_cheats",
                "1",
                "-file",
                "/wads/a.wad"
            ]
        );
    }

    #[test]
    fn the_latest_line_is_the_last_one_with_something_on_it() {
        let latest = Mutex::new(String::new());
        let output = b"W_Init: Init WADfiles.\nadding a.wad, 2 lumps\r\n\n   \npartial";
        copy_output(&output[..], None, &latest);
        assert_eq!(*latest.lock().unwrap(), "adding a.wad, 2 lumps");
    }
}
