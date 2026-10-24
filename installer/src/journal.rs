//! The install's journal: everything the installer does, with the time since it began. `say!` says
//! a line on the terminal and writes it down, `warn!` says it on stderr, and `note!` only writes
//! down the details (where each file came from and went, URLs, sizes, hashes, commands and how
//! they ended). It is kept in a log file in the folder the install goes into, once that is known
//! ([`open`]); the lines from before wait for it.

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
    ($($words:tt)*) => {{
        let line = format!($($words)*);
        eprintln!("{line}");
        $crate::journal::write(&format!("problem: {line}"));
    }};
}

/// Only writes a detail down.
#[macro_export]
macro_rules! note {
    ($($words:tt)*) => {
        $crate::journal::write(&format!("  {}", format!($($words)*)))
    };
}
