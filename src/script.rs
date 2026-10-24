//! `tdl headless`: tdl played by a script instead of a person, with no splash and no terminal.
//!
//! The script comes from a file (`--script FILE`) or, without one, from stdin, one line at a
//! time, so it can be typed as it goes. Its lines are of two kinds:
//!
//! - keys for the screens, which are drawn off-screen at [`WIDTH`] x [`HEIGHT`]: `press KEY...`
//!   (`press j j enter`, `press shift+b`, `press ctrl+c`), `type TEXT`, `wait 2s`,
//!   `expect TEXT` (on the screen now), `wait-for TEXT [10s]` (on the screen within that long,
//!   10 seconds unless given; quote a TEXT that ends in a time), and `pick PATH...` (the answer
//!   to the file dialog the last key opened; `pick` alone cancels it), and
//! - commands, done the plain way, as `--headless` does them: `launch PROFILE`,
//!   `host PROFILE [options]` and `join IP --code CODE [options]`, with the options of
//!   `tdl host` and `tdl join` (quote a name with spaces).
//!
//! The screens close before a command and open again for the next key line. `quit`, or the end of
//! the script, ends tdl; a line that fails (an `expect` that isn't on the screen, a command that
//! fails) ends it with an error. `#` starts a comment.
//!
//! With `--log [DIR]` the run is recorded in DIR (the working directory if none is given):
//! `tdl.log` has every line of the script as it is played, every screen it led to and every
//! command's outcome, and `uzdoom.log` everything UZDoom printed.

use std::{
    cell::RefCell,
    collections::VecDeque,
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::{
        OnceLock,
        mpsc::{self, Receiver, RecvTimeoutError},
    },
    thread,
    time::{Duration, Instant},
};

use ratatui::{
    buffer::Buffer,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
};

use crate::{
    cli::{Cli, Command},
    error::{Error, Result},
};

/// The size the screens are drawn at.
pub const WIDTH: u16 = 100;
pub const HEIGHT: u16 = 30;
/// How long `wait-for` waits unless it is told.
const WAIT_FOR: Duration = Duration::from_secs(10);
/// How often a waiting line looks at the screen again.
const GLANCE: Duration = Duration::from_millis(20);

/// One line of a script, read.
enum Step {
    Keys(Vec<KeyEvent>),
    Wait(Duration),
    Expect(String),
    WaitFor(String, Duration),
    Pick(Option<Vec<PathBuf>>),
    Command(Command),
    Quit,
}

/// What the script asks for next, for the loop in [`play`](crate::screens::launcher::Launcher::play).
pub enum Turn {
    /// A command, to be done with the screens closed.
    Command(Command),
    /// A key line: the screens are wanted.
    Screens,
    /// The script is over.
    End,
}

/// The script being played, and the record of it.
struct Player {
    /// The script's lines, numbered from 1, as they are read.
    lines: Receiver<(usize, String)>,
    /// Lines read from `lines` to see what is coming, not played yet.
    ahead: VecDeque<(usize, String)>,
    /// Whether `lines` has no more to give.
    over: bool,
    /// The line being played, and its number.
    step: Option<(usize, Step)>,
    /// Keys from `press` and `type` not given out yet.
    keys: VecDeque<KeyEvent>,
    /// When a `wait` is over, or a `wait-for` gives up, and when it started.
    until: Option<Instant>,
    since: Instant,
    /// How many lines have been played, and when the script started.
    played: usize,
    started: Instant,
    /// What the screens drew last, as text.
    screen: String,
    log: Option<Log>,
}

/// `tdl.log`: what happened, with the time since the start.
struct Log {
    file: File,
    started: Instant,
    /// The screen as it was last written down, so an unchanged one isn't written again.
    screen: String,
}

impl Log {
    fn line(&mut self, text: &str) {
        let seconds = self.started.elapsed().as_secs_f64();
        let _ = writeln!(self.file, "[{seconds:8.3}s] {text}");
    }

    /// The screen, if it changed since it was last written down.
    fn screen(&mut self, screen: &str) {
        if screen.is_empty() || screen == self.screen {
            return;
        }
        self.line("screen:");
        for row in screen.lines() {
            let _ = writeln!(self.file, "    │{row}");
        }
        self.screen = screen.to_string();
    }
}

thread_local! {
    static PLAYER: RefCell<Option<Player>> = const { RefCell::new(None) };
}

/// Where `--log` writes, once a script is playing with it.
static LOG_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Starts playing the script in `file`, or on stdin without one, recording it in `log_dir` if
/// given. From here on the keys come from the script.
pub fn start(file: Option<&Path>, log_dir: Option<&Path>) -> io::Result<()> {
    let source: Box<dyn Read + Send> = match file {
        Some(file) => Box::new(File::open(file).map_err(|error| {
            io::Error::new(error.kind(), format!("{}: {error}", file.display()))
        })?),
        None => Box::new(io::stdin()),
    };
    // 📜 read on a thread of its own, so a script typed line by line doesn't stop the screens
    let (sender, lines) = mpsc::channel();
    thread::spawn(move || {
        for (index, line) in BufReader::new(source).lines().enumerate() {
            let Ok(line) = line else { break };
            if sender.send((index + 1, line)).is_err() {
                break;
            }
        }
    });
    let log = match log_dir {
        Some(dir) => {
            std::fs::create_dir_all(dir)?;
            File::create(dir.join("uzdoom.log"))?;
            let _ = LOG_DIR.set(dir.to_path_buf());
            let mut log = Log {
                file: File::create(dir.join("tdl.log"))?,
                started: Instant::now(),
                screen: String::new(),
            };
            log.line(&format!(
                "tdl {} headless, playing {}",
                env!("CARGO_PKG_VERSION"),
                file.map_or("stdin".into(), |file| file.display().to_string())
            ));
            Some(log)
        }
        None => None,
    };
    PLAYER.with_borrow_mut(|player| {
        *player = Some(Player {
            lines,
            ahead: VecDeque::new(),
            over: false,
            step: None,
            keys: VecDeque::new(),
            until: None,
            since: Instant::now(),
            played: 0,
            started: Instant::now(),
            screen: String::new(),
            log,
        })
    });
    Ok(())
}

/// Whether a script is playing.
pub fn playing() -> bool {
    PLAYER.with_borrow(Option::is_some)
}

/// `uzdoom.log` in the `--log` dir, opened to add to, if there is one.
pub fn uzdoom_log() -> Option<File> {
    let dir = LOG_DIR.get()?;
    OpenOptions::new()
        .append(true)
        .create(true)
        .open(dir.join("uzdoom.log"))
        .ok()
}

/// Says `text`, and writes it in `tdl.log` if there is one.
pub fn note(text: &str) {
    crate::ritual::murmur(format!("     {text}"));
    PLAYER.with_borrow_mut(|player| {
        if let Some(log) = player.as_mut().and_then(|player| player.log.as_mut()) {
            log.line(text);
        }
    });
}

/// Whether the page about to be drawn will be read: always without a script, and with one only
/// if it records screens (`--log`), is looking at the screen (`expect`, `wait-for`), or its next
/// line is one that will, or hasn't come yet. Every other frame is skipped (see
/// [`Haunted::bewitch`](crate::ritual::Haunted::bewitch)).
pub fn wants_page() -> bool {
    PLAYER.with_borrow_mut(|player| player.as_mut().is_none_or(Player::wants_page))
}

/// Keeps what the screens just drew, for `expect` and `wait-for` (and `tdl.log`).
pub fn saw(buffer: &Buffer) {
    PLAYER.with_borrow_mut(|player| {
        let Some(player) = player.as_mut() else {
            return;
        };
        let area = buffer.area;
        let mut rows: Vec<String> = (area.top()..area.bottom())
            .map(|y| {
                (area.left()..area.right())
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect();
        while rows.last().is_some_and(String::is_empty) {
            rows.pop();
        }
        player.screen = rows.join("\n");
    });
}

/// Like [`event::poll`]: whether a key is ready within `timeout`. While a script plays, the keys
/// are its own, and its other lines are played on the way to the next key.
pub fn poll(timeout: Duration) -> Result<bool> {
    if !playing() {
        return Ok(event::poll(timeout)?);
    }
    PLAYER.with_borrow_mut(|player| player.as_mut().unwrap().poll(timeout))
}

/// Like [`event::read`]: the next key, from the script while one plays.
pub fn read() -> Result<Event> {
    if !playing() {
        return Ok(event::read()?);
    }
    loop {
        if poll(Duration::from_secs(1))? {
            let key = PLAYER.with_borrow_mut(|player| player.as_mut().unwrap().keys.pop_front());
            if let Some(key) = key {
                return Ok(Event::Key(key));
            }
        }
    }
}

/// The answer to a file dialog: the script's next line, which must be `pick`.
pub fn pick() -> Result<Option<Vec<PathBuf>>> {
    PLAYER.with_borrow_mut(|player| {
        let player = player.as_mut().unwrap();
        let Some((number, step)) = player.next(None)? else {
            return Err(Error::Script(
                "a file dialog opened, but the script ended".into(),
            ));
        };
        match step {
            Step::Pick(paths) => Ok(paths),
            step => {
                player.step = Some((number, step));
                Err(Error::Script(format!(
                    "line {number}: a file dialog opened, so this line should have been pick"
                )))
            }
        }
    })
}

/// What the script asks for next, waiting for its next line if need be.
pub fn turn() -> Result<Turn> {
    PLAYER.with_borrow_mut(|player| {
        let player = player.as_mut().unwrap();
        if !player.keys.is_empty() {
            return Ok(Turn::Screens);
        }
        if player.step.is_none() {
            player.step = player.next(None)?;
        }
        Ok(match player.step.take() {
            None | Some((_, Step::Quit)) => Turn::End,
            Some((_, Step::Command(command))) => Turn::Command(command),
            step => {
                player.step = step;
                Turn::Screens
            }
        })
    })
}

impl Player {
    fn wants_page(&mut self) -> bool {
        if self.log.is_some() {
            return true;
        }
        if let Some((_, Step::Expect(_) | Step::WaitFor(..))) = &self.step {
            return true;
        }
        if self.step.is_some() || !self.keys.is_empty() {
            return false;
        }
        // 🔭 a look at what is coming, without playing it
        while let Ok(line) = self.lines.try_recv() {
            self.ahead.push_back(line);
        }
        if self.lines.try_recv() == Err(mpsc::TryRecvError::Disconnected) {
            self.over = true;
        }
        let coming = self
            .ahead
            .iter()
            .map(|(_, text)| text.trim())
            .find(|text| !text.is_empty() && !text.starts_with('#'));
        match coming {
            Some(text) => text.starts_with("expect") || text.starts_with("wait-for"),
            None => !self.over,
        }
    }

    /// The next line that is a step, read and logged, waiting `timeout` for it (forever without
    /// one). `None` at the end of the script; `Err(ScriptWaiting)` if it didn't come in time.
    fn next(&mut self, timeout: Option<Duration>) -> Result<Option<(usize, Step)>> {
        loop {
            let (number, text) = if let Some(line) = self.ahead.pop_front() {
                line
            } else if self.over {
                return Ok(None);
            } else {
                match timeout {
                    Some(timeout) => match self.lines.recv_timeout(timeout) {
                        Ok(line) => line,
                        Err(RecvTimeoutError::Timeout) => return Err(Error::ScriptWaiting),
                        Err(RecvTimeoutError::Disconnected) => return Ok(None),
                    },
                    None => match self.lines.recv() {
                        Ok(line) => line,
                        Err(_) => return Ok(None),
                    },
                }
            };
            let text = text.trim();
            if text.is_empty() || text.starts_with('#') {
                continue;
            }
            if let Some(log) = &mut self.log {
                log.screen(&self.screen);
                log.line(&format!("line {number}: {text}"));
            }
            crate::ritual::invoke_line(number, text);
            self.played += 1;
            let step = parse(text).map_err(|problem| {
                let problem = format!("line {number}: {problem}");
                if let Some(log) = &mut self.log {
                    log.line(&format!("failed: {problem}"));
                }
                Error::Script(problem)
            })?;
            return Ok(Some((number, step)));
        }
    }

    fn poll(&mut self, timeout: Duration) -> Result<bool> {
        loop {
            if !self.keys.is_empty() {
                return Ok(true);
            }
            if self.step.is_none() {
                self.step = match self.next(Some(timeout)) {
                    Ok(Some(step)) => Some(step),
                    Ok(None) => return Err(Error::ScriptEnd),
                    Err(Error::ScriptWaiting) => return Ok(false),
                    Err(error) => return Err(error),
                };
            }
            let (number, step) = self.step.take().unwrap();
            match step {
                Step::Keys(keys) => self.keys.extend(keys),
                Step::Wait(how_long) => {
                    let until = *self.until.get_or_insert_with(|| {
                        self.since = Instant::now();
                        Instant::now() + how_long
                    });
                    let now = Instant::now();
                    if now >= until {
                        self.until = None;
                    } else {
                        crate::ritual::planchette(
                            &format!("waiting {}s", how_long.as_secs_f64()),
                            self.since.elapsed(),
                        );
                        self.step = Some((number, Step::Wait(how_long)));
                        thread::sleep(timeout.min(until - now));
                        return Ok(false);
                    }
                }
                Step::Expect(text) => {
                    if !self.screen.contains(&text) {
                        return Err(self.failed(number, &format!("\"{text}\" isn't on the screen")));
                    }
                }
                Step::WaitFor(text, within) => {
                    if self.screen.contains(&text) {
                        self.until = None;
                    } else {
                        let until = *self.until.get_or_insert_with(|| {
                            self.since = Instant::now();
                            Instant::now() + within
                        });
                        crate::ritual::planchette(
                            &format!("waiting for \"{text}\""),
                            self.since.elapsed(),
                        );
                        if Instant::now() >= until {
                            self.until = None;
                            let problem = format!(
                                "\"{text}\" didn't come on the screen within {}s",
                                within.as_secs_f64()
                            );
                            return Err(self.failed(number, &problem));
                        }
                        self.step = Some((number, Step::WaitFor(text, within)));
                        thread::sleep(timeout.min(GLANCE));
                        return Ok(false);
                    }
                }
                Step::Pick(_) => {
                    return Err(self.failed(number, "pick, but no file dialog is open"));
                }
                // 🚪 the screens close for a command, which the loop then does
                Step::Command(command) => {
                    self.step = Some((number, Step::Command(command)));
                    return Err(Error::ScriptCommand);
                }
                Step::Quit => {
                    self.step = Some((number, Step::Quit));
                    return Err(Error::ScriptEnd);
                }
            }
        }
    }

    /// The error for line `number`, written in `tdl.log` with the screen it was looking at.
    fn failed(&mut self, number: usize, problem: &str) -> Error {
        let problem = format!("line {number}: {problem}");
        if let Some(log) = &mut self.log {
            log.screen(&self.screen);
            log.line(&format!("failed: {problem}"));
        }
        Error::Script(problem)
    }
}

/// Writes how the run ended in `tdl.log`, and the last screen.
pub fn finish(outcome: &std::result::Result<(), String>) {
    PLAYER.with_borrow_mut(|player| {
        let Some(player) = player.as_mut() else {
            // 🚪 the séance never began (no script to read): say why all the same
            if let Err(problem) = outcome {
                crate::ritual::farewell(false, problem);
            }
            return;
        };
        let screen = player.screen.clone();
        if let Some(log) = &mut player.log {
            log.screen(&screen);
            match outcome {
                Ok(()) => log.line("done"),
                Err(problem) => log.line(&format!("ended: {problem}")),
            }
        }
        let lines = if player.played == 1 { "line" } else { "lines" };
        let took = player.started.elapsed().as_secs_f64();
        match outcome {
            Ok(()) => crate::ritual::farewell(
                true,
                &format!(
                    "the script is done · {} {lines} · {took:.1}s",
                    player.played
                ),
            ),
            Err(problem) => {
                // 👻 what was on the table when the circle broke
                if !screen.is_empty() {
                    crate::ritual::apparition(&screen);
                }
                crate::ritual::farewell(false, problem);
            }
        }
    });
}

/// One line of a script, as a step.
fn parse(text: &str) -> std::result::Result<Step, String> {
    let (word, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    let rest = rest.trim();
    match word {
        "press" => {
            let keys = rest
                .split_whitespace()
                .map(key)
                .collect::<std::result::Result<Vec<_>, _>>()?;
            if keys.is_empty() {
                return Err("press needs a key, like press enter".into());
            }
            Ok(Step::Keys(keys))
        }
        "type" => Ok(Step::Keys(
            rest.chars()
                .map(|c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
                .collect(),
        )),
        "wait" => Ok(Step::Wait(duration(rest)?)),
        "expect" => Ok(Step::Expect(unquoted(rest)?)),
        "wait-for" => {
            let (text, within) = match rest.rsplit_once(char::is_whitespace) {
                Some((text, last)) if !rest.ends_with('"') && duration(last).is_ok() => {
                    (text.trim(), duration(last)?)
                }
                _ => (rest, WAIT_FOR),
            };
            Ok(Step::WaitFor(unquoted(text)?, within))
        }
        "pick" => {
            let paths = words(rest)?;
            Ok(Step::Pick(
                (!paths.is_empty()).then(|| paths.into_iter().map(PathBuf::from).collect()),
            ))
        }
        "launch" | "host" | "join" => {
            let mut args = vec![word.to_string()];
            args.extend(words(rest)?);
            args.push("--headless".into());
            let cli = Cli::parse(args).map_err(|error| error.to_string().trim().to_string())?;
            cli.command
                .map(Step::Command)
                .ok_or_else(|| format!("{word} isn't a command"))
        }
        "quit" => Ok(Step::Quit),
        _ => Err(format!(
            "{word} isn't something a script can do (press, type, wait, expect, wait-for, pick, \
             launch, host, join, quit)"
        )),
    }
}

/// A key like `enter`, `j`, `shift+b` or `ctrl+c`.
fn key(name: &str) -> std::result::Result<KeyEvent, String> {
    let mut parts: Vec<&str> = name.split('+').collect();
    // ➕ `+` and `shift++` are the plus key itself
    if name.ends_with("++") || name == "+" {
        parts.retain(|part| !part.is_empty());
        parts.push("+");
    }
    let last = parts.pop().unwrap_or_default();
    let mut modifiers = KeyModifiers::NONE;
    for part in parts {
        modifiers |= match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => KeyModifiers::CONTROL,
            "shift" => KeyModifiers::SHIFT,
            "alt" => KeyModifiers::ALT,
            _ => return Err(format!("{part} in {name} isn't ctrl, shift or alt")),
        };
    }
    let code = match last.to_ascii_lowercase().as_str() {
        "enter" | "return" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "tab" if modifiers.contains(KeyModifiers::SHIFT) => KeyCode::BackTab,
        "tab" => KeyCode::Tab,
        "backtab" => KeyCode::BackTab,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" | "pgup" => KeyCode::PageUp,
        "pagedown" | "pgdn" => KeyCode::PageDown,
        "delete" | "del" => KeyCode::Delete,
        "backspace" => KeyCode::Backspace,
        "insert" => KeyCode::Insert,
        "space" => KeyCode::Char(' '),
        f if f.len() > 1
            && f.starts_with('f')
            && let Ok(number) = f[1..].parse::<u8>() =>
        {
            KeyCode::F(number)
        }
        _ => {
            let mut chars = last.chars();
            match (chars.next(), chars.next()) {
                // 🔠 a shifted letter comes as the capital, as terminals send it
                (Some(c), None) if modifiers.contains(KeyModifiers::SHIFT) => {
                    KeyCode::Char(c.to_ascii_uppercase())
                }
                (Some(c), None) => KeyCode::Char(c),
                _ => return Err(format!("{name} isn't a key")),
            }
        }
    };
    Ok(KeyEvent::new(code, modifiers))
}

/// A time like `2s`, `500ms` or `2` (seconds).
fn duration(text: &str) -> std::result::Result<Duration, String> {
    let text = text.trim();
    let (number, scale) = if let Some(number) = text.strip_suffix("ms") {
        (number, 0.001)
    } else if let Some(number) = text.strip_suffix('s') {
        (number, 1.0)
    } else {
        (text, 1.0)
    };
    number
        .parse::<f64>()
        .ok()
        .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
        .map(|seconds| Duration::from_secs_f64(seconds * scale))
        .ok_or_else(|| format!("{text} isn't a time, like 2s or 500ms"))
}

/// `text` without the quotes around it, if it has them; it must not be empty.
fn unquoted(text: &str) -> std::result::Result<String, String> {
    let text = text.trim();
    let text = text
        .strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
        .unwrap_or(text);
    if text.is_empty() {
        return Err("there is no text to look for".into());
    }
    Ok(text.to_string())
}

/// `text` split at whitespace, keeping what is in double quotes together.
fn words(text: &str) -> std::result::Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut started = false;
    for c in text.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            c => {
                word.push(c);
                started = true;
            }
        }
    }
    if quoted {
        return Err("a quote isn't closed".into());
    }
    if started {
        words.push(word);
    }
    Ok(words)
}
