use std::{
    ffi::{OsStr, OsString},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::Duration,
};

use crate::{
    error::{Error, Result},
    loadout::wad::{self, Wad},
    net::{Address, Entry, new_agent},
    tally::Tally,
};

/// How slowly a wad may arrive before its download is given up on, so a dead connection
/// doesn't hold a download thread forever. Generous, so only a stalled transfer trips it.
const MIN_BYTES_PER_SECOND: u64 = 16 * 1024;

/// How long the body of a `size`-byte wad may take to arrive: half a minute to get going, plus
/// the time it takes at [`MIN_BYTES_PER_SECOND`]. ureq's body timeout covers the whole body
/// rather than each read, so it has to grow with the file.
fn body_timeout(size: u64) -> Duration {
    Duration::from_secs(30 + size / MIN_BYTES_PER_SECOND)
}

/// Downloads every entry in order, returning the wads at their local paths.
/// `progress` is told about each file before it starts, so its size is always the current one.
pub fn download(
    address: &Address,
    entries: &[Entry],
    downloads_dir: &Path,
    progress: &mut impl Tally,
) -> Result<Vec<Wad>> {
    let agent = new_agent();
    let mut wads = Vec::with_capacity(entries.len());
    for entry in entries {
        progress.count_file(OsStr::new(&entry.name), entry.size)?;
        wads.push(download_entry(
            &agent,
            address,
            entry,
            downloads_dir,
            progress,
        )?);
    }
    Ok(wads)
}

/// What a download running in the background tells the thread that draws it.
pub enum Report {
    /// A file is starting, with its full size.
    File(OsString, u64),
    /// More bytes of the current file have arrived.
    Bytes(u64),
    /// The download is over: the wads, or why it stopped.
    Done(Result<Vec<Wad>>),
}

/// Runs [`download`] on its own thread, so the screen keeps drawing and reading keys while it
/// waits on the network. Setting `cancelled` stops it after the chunk it is on (its partial file is
/// cleaned up as for any failure); the last report is always [`Report::Done`].
pub fn download_in_background(
    address: Address,
    entries: Vec<Entry>,
    downloads_dir: PathBuf,
    cancelled: Arc<AtomicBool>,
) -> Receiver<Report> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut reporter = Reporter {
            sender: sender.clone(),
            cancelled,
        };
        let result = download(&address, &entries, &downloads_dir, &mut reporter);
        let _ = sender.send(Report::Done(result));
    });
    receiver
}

/// Passes a background download's progress over the channel, and stops it once it is cancelled
/// or nobody is listening anymore.
struct Reporter {
    sender: Sender<Report>,
    cancelled: Arc<AtomicBool>,
}

impl Reporter {
    fn send(&self, report: Report) -> Result<()> {
        if self.cancelled.load(Ordering::SeqCst) {
            return Err(Error::Cancelled);
        }
        self.sender.send(report).map_err(|_| Error::Cancelled)
    }
}

impl Tally for Reporter {
    fn count_file(&mut self, name: &OsStr, size: u64) -> Result<()> {
        self.send(Report::File(name.to_os_string(), size))
    }

    fn count_bytes(&mut self, amount: u64) -> Result<()> {
        self.send(Report::Bytes(amount))
    }
}

/// Reuses a previous download if present. Otherwise downloads to `<name>.part` and renames it
/// only after verification, so a partial or wrong file is never mistaken for a finished one.
fn download_entry(
    agent: &ureq::Agent,
    address: &Address,
    entry: &Entry,
    downloads_dir: &Path,
    progress: &mut impl Tally,
) -> Result<Wad> {
    let downloads_subdir = downloads_dir.join(&entry.hash);
    let wad_path = downloads_subdir.join(&entry.name);
    if wad_path.exists() {
        // already on disk, so the bar jumps to full instead of staying empty
        progress.count_bytes(entry.size)?;
        return Ok(downloaded(wad_path, entry));
    }
    std::fs::create_dir_all(&downloads_subdir)?;
    let part = downloads_subdir.join(format!("{}.part", entry.name));
    if let Err(error) = download_part(agent, address, entry, &part, progress) {
        let _ = std::fs::remove_file(&part);
        return Err(error);
    }
    std::fs::rename(&part, &wad_path)?;
    Ok(downloaded(wad_path, entry))
}

/// The wad for `entry`, downloaded to `path`. Its modified time is recorded, so the lookup can
/// trust the file later without reading it.
fn downloaded(path: PathBuf, entry: &Entry) -> Wad {
    let modified = std::fs::metadata(&path).map_or(0, |metadata| wad::modified(&metadata));
    Wad {
        modified,
        ..Wad::new(path.into(), entry.hash.clone(), entry.size)
    }
}

/// Streams the file to `part`, failing if it grows past the manifest's size or its hash differs.
fn download_part(
    agent: &ureq::Agent,
    address: &Address,
    entry: &Entry,
    part: &Path,
    progress: &mut impl Tally,
) -> Result<()> {
    let url = address.url(&format!("file/{}", entry.hash));
    let mut reader = agent
        .get(&url)
        .config()
        .timeout_recv_body(Some(body_timeout(entry.size)))
        .build()
        .call()?
        .into_body()
        .into_reader();
    let mut file = std::fs::File::create(part)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0; 64 * 1024];
    let mut total = 0;

    loop {
        let size = reader.read(&mut buf)?;
        progress.count_bytes(size as u64)?;
        if size == 0 {
            break;
        }
        total += size;
        if total as u64 > entry.size {
            return Err(Error::Download(
                "File downloading is bigger than it should be",
            ));
        }
        hasher.update(&buf[..size]);
        file.write_all(&buf[..size])?;
    }
    file.flush()?;
    if hasher.finalize().to_hex().to_string() != entry.hash {
        return Err(Error::Download(
            "Downloaded file does not have matching hash",
        ));
    }
    Ok(())
}
