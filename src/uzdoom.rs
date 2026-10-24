use std::{
    collections::VecDeque,
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
    loadout::host_settings::HostSettings,
    loadout::profile::Profile,
    loadout::wad::Wad,
};

/// How long a stopped UZDoom gets to go quietly before it is killed outright.
const GRACE: Duration = Duration::from_secs(2);
/// How many of UZDoom's lines are kept to show in its log window; `uzdoom.log` has them all.
const KEPT_LINES: usize = 10_000;

/// UZDoom while it runs. Its output is copied to `uzdoom.log` on a thread of its own, so whoever
/// started it is free to keep the screen moving, and its lines are kept to show how far its
/// startup has got and, in the log window, everything it said.
pub struct Game {
    child: Child,
    output: Arc<Mutex<Output>>,
    logger: Option<JoinHandle<()>>,
}

/// What UZDoom has printed so far: its last [`KEPT_LINES`] non-blank lines, the latest last.
#[derive(Default)]
struct Output {
    lines: VecDeque<String>,
}

impl Output {
    fn push(&mut self, line: String) {
        if self.lines.len() == KEPT_LINES {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }
}

impl Game {
    /// The last line UZDoom printed, or empty if it hasn't printed one yet.
    pub fn latest_line(&self) -> String {
        self.output
            .lock()
            .ok()
            .and_then(|output| output.lines.back().cloned())
            .unwrap_or_default()
    }

    /// Everything UZDoom has printed that is still kept, oldest first.
    pub fn lines(&self) -> Vec<String> {
        self.output
            .lock()
            .map(|output| output.lines.iter().cloned().collect())
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
    /// Keys (and controller presses) made during the game would otherwise all land at once
    /// afterwards, so they are tossed. That is best effort, since a terminal that can't be read (as under a test) has
    /// nothing to toss.
    pub fn wait(mut self) -> Result<()> {
        self.child.wait()?;
        if let Some(logger) = self.logger.take() {
            let _ = logger.join();
        }
        while !crate::ritual::possessed()
            && let Ok(true) = ratatui::crossterm::event::poll(Duration::ZERO)
        {
            let _ = ratatui::crossterm::event::read();
        }
        crate::gamepad::forget();
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

/// Starts UZDoom, with its output going to `uzdoom.log` in `data_subdir`. `extra` (the player's
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
    let output = Arc::new(Mutex::new(Output::default()));
    let logger = child.stdout.take().map(|stdout| {
        // the log is best effort, but stdout must still be drained or the game blocks on a full pipe
        let log = File::create(data_subdir.join("uzdoom.log")).ok();
        // 📜 and into `tdl headless --log`'s uzdoom.log, if there is one
        let logs = log
            .into_iter()
            .chain(crate::ritual::prayer_from_beyond())
            .collect();
        let output = output.clone();
        thread::spawn(move || copy_output(stdout, logs, &output))
    });
    Ok(Game {
        child,
        output,
        logger,
    })
}

/// Copies everything from `output` into `logs` until UZDoom closes it, keeping its non-blank
/// lines in `kept` as it goes.
fn copy_output(mut output: impl Read, mut logs: Vec<File>, kept: &Mutex<Output>) {
    let mut buffer = [0; 8 * 1024];
    let mut line = Vec::new();
    loop {
        let read = match output.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        // a log that can't be written is dropped; the others carry on
        logs.retain_mut(|file| file.write_all(&buffer[..read]).is_ok());
        for &byte in &buffer[..read] {
            if byte == b'\n' || byte == b'\r' {
                let text = String::from_utf8_lossy(&line).trim().to_string();
                if !text.is_empty()
                    && let Ok(mut kept) = kept.lock()
                {
                    kept.push(text);
                }
                line.clear();
            } else {
                line.push(byte);
            }
        }
    }
}

/// Plays offline, with saves kept in the profile's own dir. `base_wad` and `ini` are the ones in
/// effect (see [`Profile::base_wad`] and [`Profile::ini`]), as for hosting and joining.
pub fn launch(
    uzdoom: &OsStr,
    base_wad: Option<&OsStr>,
    ini: Option<&OsStr>,
    profile: &Profile,
    extra: &str,
    data_subdir: &Path,
) -> Result<Game> {
    let saves_path = data_subdir.join("saves");
    let mut args = vec![OsStr::new("-savedir"), saves_path.as_os_str()];
    if let Some(base_wad) = base_wad {
        args.extend([OsStr::new("-iwad"), base_wad]);
    }
    if let Some(ini) = ini {
        args.extend([OsStr::new("-config"), ini])
    }
    execute(uzdoom, profile, &args, extra, data_subdir)
}

/// Starts a multiplayer game as host, with the player count, mode and map from `settings`. An
/// empty map lets UZDoom pick the starting map.
pub fn host(
    uzdoom: &OsStr,
    base_wad: Option<&OsStr>,
    ini: Option<&OsStr>,
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
    if let Some(ini) = ini {
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
    ini: Option<&OsStr>,
    profile: &Profile,
    ip: &str,
    extra: &str,
    data_subdir: &Path,
) -> Result<Game> {
    let mut args = vec![OsStr::new("-join"), OsStr::new(ip)];
    if let Some(base_wad) = base_wad {
        args.extend([OsStr::new("-iwad"), base_wad]);
    }
    if let Some(ini) = ini {
        args.extend([OsStr::new("-config"), ini]);
    }
    execute(uzdoom, profile, &args, extra, data_subdir)
}
