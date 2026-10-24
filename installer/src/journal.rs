//! The install's journal: everything the installer does, with the time since it began. `say!` says
//! a line on the terminal and writes it down, `warn!` says it on stderr, and `note!` only writes
//! down the details (where each file came from and went, URLs, sizes, hashes, commands and how
//! they ended). It is kept in a log file in the folder the install goes into, once that is known
//! ([`open`]); the lines from before wait for it.
//!
//! What is said can be in a voice from installation-groans ([`speak_in`]): [`tell`] says an event
//! (begin, download, verified, copy, done...) as one of the voice's phrases for it, picked at
//! random, and `warn!` says a problem as one of its `problem` phrases. The journal keeps the plain
//! line beside whatever the voice said.

use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

struct Journal {
    file: Option<File>,
    /// Lines written before the log file was opened.
    waiting: Vec<String>,
    began: Instant,
}

fn journal() -> &'static Mutex<Journal> {
    static JOURNAL: OnceLock<Mutex<Journal>> = OnceLock::new();
    JOURNAL.get_or_init(|| {
        Mutex::new(Journal {
            file: None,
            waiting: Vec::new(),
            began: Instant::now(),
        })
    })
}

/// Writes `line` down, with the time since the install began.
pub fn write(line: &str) {
    let Ok(mut journal) = journal().lock() else {
        return;
    };
    let line = format!("[{:8.3}s] {line}", journal.began.elapsed().as_secs_f64());
    match &mut journal.file {
        Some(file) => {
            let _ = writeln!(file, "{line}");
        }
        None => journal.waiting.push(line),
    }
}

/// Starts keeping the journal in `dir/name`, added to if it is there already, with what was written
/// so far. Returns where it is, or `None` if it can't be written (the install goes on without it).
pub fn open(dir: &Path, name: &str) -> Option<PathBuf> {
    let path = dir.join(name);
    fs::create_dir_all(dir).ok()?;
    let mut file = File::options().create(true).append(true).open(&path).ok()?;
    let since = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_secs())
        .unwrap_or_default();
    let mut journal = journal().lock().ok()?;
    let _ = writeln!(
        file,
        "\n=== {} {}, started at {since} (seconds since 1970) ===",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );
    for line in journal.waiting.drain(..) {
        let _ = writeln!(file, "{line}");
    }
    journal.file = Some(file);
    Some(path)
}

/// A voice: an installation-groans TOML (its `[phrases]` for each event, and its `[interjection]`).
static VOICE: OnceLock<toml::Table> = OnceLock::new();

/// Speaks in the voice `source` (an installation-groans TOML) from now on. One that can't be read
/// (or is empty, when the build found none) leaves the plain lines.
pub fn speak_in(source: &str) {
    if let Ok(table) = source.parse::<toml::Table>()
        && table.get("phrases").is_some_and(toml::Value::is_table)
    {
        let _ = VOICE.set(table);
    }
}

/// A number from the time and how many have been drawn: enough to pick phrases with.
fn draw() -> usize {
    use std::sync::atomic::{AtomicU64, Ordering};
    static DRAWN: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.subsec_nanos() as u64)
        .unwrap_or_default();
    let mut x = nanos ^ DRAWN.fetch_add(0x9e37_79b9_7f4a_7c15, Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x as usize
}

/// One of the voice's phrases for `event`, picked at random, its `{placeholders}` filled in from
/// `fills`; `None` without a voice or a phrase for it.
pub fn phrase(event: &str, fills: &[(&str, &str)]) -> Option<String> {
    let phrases = VOICE.get()?.get("phrases")?.get(event)?.as_array()?;
    let phrases: Vec<&str> = phrases.iter().filter_map(toml::Value::as_str).collect();
    if phrases.is_empty() {
        return None;
    }
    let mut said = phrases[draw() % phrases.len()].to_string();
    for (name, value) in fills {
        said = said.replace(&format!("{{{name}}}"), value);
    }
    // 🔠 a phrase that opens with a placeholder still opens a sentence
    let mut letters = said.chars();
    Some(match letters.next() {
        Some(first) => first.to_uppercase().chain(letters).collect(),
        None => said,
    })
}

/// Says `event` in the voice (filled from `fills`), or `plain` without one, and writes it down.
pub fn tell(event: &str, fills: &[(&str, &str)], plain: &str) {
    match phrase(event, fills) {
        Some(said) => {
            println!("{said}");
            write(&said);
            write(&format!("  ({plain})"));
            interject();
        }
        None => {
            println!("{plain}");
            write(plain);
        }
    }
}

/// Now and then (the voice's `[interjection]` `chance`), something else speaks through the machine:
/// one of its phrases, in its color, bold, on a terminal.
fn interject() {
    use std::io::IsTerminal;
    let Some(interjection) = VOICE.get().and_then(|voice| voice.get("interjection")) else {
        return;
    };
    let chance = interjection
        .get("chance")
        .and_then(toml::Value::as_float)
        .unwrap_or(0.0);
    if (draw() % 10_000) as f64 >= chance.clamp(0.0, 1.0) * 10_000.0 {
        return;
    }
    let phrases: Vec<&str> = interjection
        .get("phrases")
        .and_then(toml::Value::as_array)
        .map(|phrases| phrases.iter().filter_map(toml::Value::as_str).collect())
        .unwrap_or_default();
    if phrases.is_empty() {
        return;
    }
    let said = phrases[draw() % phrases.len()];
    let code = match interjection
        .get("color")
        .and_then(toml::Value::as_str)
        .unwrap_or("red")
    {
        "yellow" => 33,
        "green" => 32,
        "blue" => 34,
        "magenta" => 35,
        "cyan" => 36,
        "white" => 37,
        _ => 31,
    };
    if std::io::stdout().is_terminal() {
        println!("\x1b[1;{code}m{said}\x1b[0m");
    } else {
        println!("{said}");
    }
    write(said);
}

/// Says a problem in the voice's `problem` phrases (or as it is), on stderr, and writes it down.
pub fn wail(problem: &str) {
    let said = phrase("problem", &[("problem", problem)]).unwrap_or_else(|| problem.to_string());
    eprintln!("{said}");
    write(&format!("problem: {problem}"));
}

/// A size, as a person reads it.
pub fn size(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        format!("{} MB", bytes / 1_000_000)
    } else {
        format!("{} KB", bytes.div_ceil(1000))
    }
}

/// Says a line on the terminal and writes it down.
#[macro_export]
macro_rules! say {
    ($($words:tt)*) => {{
        let line = format!($($words)*);
        println!("{line}");
        $crate::journal::write(&line);
    }};
}

/// Says a problem on stderr and writes it down.
#[macro_export]
macro_rules! warn {
    ($($words:tt)*) => {
        $crate::journal::wail(&format!($($words)*))
    };
}

/// Only writes a detail down.
#[macro_export]
macro_rules! note {
    ($($words:tt)*) => {
        $crate::journal::write(&format!("  {}", format!($($words)*)))
    };
}
