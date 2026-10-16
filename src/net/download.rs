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
    net::{Address, Entry, new_agent},
    tally::Tally,
    vault::wad::{self, Wad},
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
    let part = downloads_subdir.join({ format!("{}.part", entry.name) });
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

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::{
        net::{
            Manifest,
            tests::{CODE, host, host_wads, local},
        },
        vault::profile::Profile,
        vault::scribe::tests::Altar,
    };

    /// Counts what a download reports, instead of drawing it.
    #[derive(Default)]
    struct Ledger {
        files: Vec<(String, u64)>,
        bytes: u64,
        /// The bytes of each file, as the progress window would count them: back to 0 at every
        /// new file, and never past that file's size.
        per_file: Vec<u64>,
    }

    impl Tally for Ledger {
        fn count_file(&mut self, name: &OsStr, size: u64) -> Result<()> {
            self.files.push((name.to_string_lossy().into_owned(), size));
            self.per_file.push(0);
            Ok(())
        }

        fn count_bytes(&mut self, amount: u64) -> Result<()> {
            self.bytes += amount;
            let (_, size) = self.files.last().expect("bytes only come after a file");
            let current = self.per_file.last_mut().expect("pushed with the file");
            *current += amount;
            assert!(*current <= *size, "a file counted past its own size");
            Ok(())
        }
    }

    #[test]
    fn a_joiner_downloads_exactly_what_the_host_serves() {
        let (host_altar, joiner) = (Altar::new("net-host"), Altar::new("net-joiner"));
        let wads = host_wads(&host_altar);
        let (_server, address) = host(&wads);

        let fetched = Manifest::fetch(&address).unwrap();
        let mut tally = Ledger::default();
        let downloads = joiner.file("downloads");
        let got = download(&address, &fetched.entries, &downloads, &mut tally).unwrap();

        for (got, sent) in got.iter().zip(&wads) {
            assert_eq!(fs::read(&got.path).unwrap(), fs::read(&sent.path).unwrap());
            assert_eq!(got.hash, sent.hash);
        }
        assert_eq!(
            tally.files,
            [("small.wad".into(), 11), ("big.pk3".into(), 200_000)]
        );
        assert_eq!(tally.bytes, 200_011);
        // a profile made from the downloads is one the host's game accepts
        assert!(fetched.is_compatible_with(&Profile::new("p".into(), got.clone())));

        // joining again reuses what is already on disk
        let again = download(
            &address,
            &fetched.entries,
            &downloads,
            &mut Ledger::default(),
        )
        .unwrap();
        assert_eq!(again, got);
    }

    #[test]
    fn several_wads_download_in_any_size_order_and_each_counts_from_zero() {
        let host_altar = Altar::new("net-host-orders");
        let size = |name: &str| fs::metadata(host_altar.file(name)).unwrap().len();
        fs::write(host_altar.file("tiny.wad"), b"t").unwrap();
        fs::write(host_altar.file("mid.wad"), vec![3u8; 70_000]).unwrap();
        let mut wads = host_wads(&host_altar);
        wads.extend(
            ["tiny.wad", "mid.wad"].map(|name| Wad::from_path(&host_altar.file(name)).unwrap()),
        );
        let [small, big, tiny, mid] = [0, 1, 2, 3].map(|i| wads[i].clone());

        for (i, order) in [
            vec![small.clone(), big.clone()],
            vec![big.clone(), small.clone()],
            vec![big.clone(), tiny.clone(), mid.clone(), small.clone()],
            vec![tiny.clone(), big.clone(), tiny.clone()],
        ]
        .into_iter()
        .enumerate()
        {
            let joiner = Altar::new(&format!("net-joiner-order-{i}"));
            let (_server, address) = host(&order);
            let fetched = Manifest::fetch(&address).unwrap();
            let downloads = joiner.file("downloads");
            let mut tally = Ledger::default();
            let got = download(&address, &fetched.entries, &downloads, &mut tally).unwrap();

            let names: Vec<_> = order.iter().map(|w| w.file_name().into_owned()).collect();
            let sizes: Vec<_> = names.iter().map(|name| size(name)).collect();
            // each file came in whole and no further, though a repeated one is only fetched once
            assert_eq!(tally.per_file, sizes, "{names:?}");
            assert_eq!(
                tally.files.iter().map(|(n, _)| n).collect::<Vec<_>>(),
                names.iter().collect::<Vec<_>>()
            );
            for (got, sent) in got.iter().zip(&order) {
                assert_eq!(got.hash, sent.hash);
                assert_eq!(fs::read(&got.path).unwrap(), fs::read(&sent.path).unwrap());
            }
        }
    }

    #[test]
    fn a_file_that_changes_after_hosting_starts_is_rejected_and_leaves_nothing_behind() {
        let (host_altar, joiner) = (
            Altar::new("net-host-changed"),
            Altar::new("net-joiner-changed"),
        );
        let wads = host_wads(&host_altar);
        let (_server, address) = host(&wads);
        // same size, different bytes: only the hash can tell
        fs::write(host_altar.file("small.wad"), b"A SMALL WAD").unwrap();
        let fetched = Manifest::fetch(&address).unwrap();
        let downloads = joiner.file("downloads");

        let result = download(
            &address,
            &fetched.entries,
            &downloads,
            &mut Ledger::default(),
        );

        assert!(matches!(result, Err(Error::Download(_))));
        let folder = downloads.join(&wads[0].hash);
        assert!(
            fs::read_dir(&folder).unwrap().next().is_none(),
            "no .part or wad left"
        );
    }

    #[test]
    fn a_cancelled_background_download_stops_and_says_so() {
        let (host_altar, joiner) = (
            Altar::new("net-host-cancel"),
            Altar::new("net-joiner-cancel"),
        );
        let wads = host_wads(&host_altar);
        let (_server, address) = host(&wads);
        let fetched = Manifest::fetch(&address).unwrap();

        // cancelled before it starts, so it stops at the first report
        let cancelled = Arc::new(AtomicBool::new(true));
        let reports = download_in_background(
            address,
            fetched.entries,
            joiner.file("downloads"),
            cancelled,
        );
        let last = reports.iter().last();
        assert!(matches!(last, Some(Report::Done(Err(Error::Cancelled)))));
    }

    #[test]
    fn a_background_download_reports_everything_then_the_wads() {
        let (host_altar, joiner) = (Altar::new("net-host-bg"), Altar::new("net-joiner-bg"));
        let wads = host_wads(&host_altar);
        let (_server, address) = host(&wads);
        let fetched = Manifest::fetch(&address).unwrap();

        let reports: Vec<_> = download_in_background(
            address,
            fetched.entries,
            joiner.file("downloads"),
            Arc::new(AtomicBool::new(false)),
        )
        .iter()
        .collect();
        let files = reports
            .iter()
            .filter(|r| matches!(r, Report::File(..)))
            .count();
        let bytes: u64 = reports
            .iter()
            .map(|r| match r {
                Report::Bytes(amount) => *amount,
                _ => 0,
            })
            .sum();
        assert_eq!((files, bytes), (2, 200_011));
        match reports.last() {
            Some(Report::Done(Ok(got))) => assert_eq!(got.len(), 2),
            _ => panic!("the last report is the finished download"),
        }
    }

    #[test]
    fn big_files_get_longer_to_arrive() {
        assert_eq!(body_timeout(0), Duration::from_secs(30));
        assert_eq!(
            body_timeout(100 * MIN_BYTES_PER_SECOND),
            Duration::from_secs(130)
        );
    }

    #[test]
    fn the_wrong_code_gets_no_files_either() {
        let host_altar = Altar::new("net-host-code-files");
        let wads = host_wads(&host_altar);
        let (_server, address) = host(&wads);
        let wrong = local(address.port, "guess");
        assert_ne!(wrong.code, CODE);
        let entries = [Entry::new("small.wad".into(), 11, wads[0].hash.clone())];
        let joiner = Altar::new("net-joiner-code-files");
        assert!(download(&wrong, &entries, &joiner.file("downloads"), &mut ()).is_err());
    }
}
