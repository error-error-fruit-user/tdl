use std::{
    ffi::{OsStr, OsString},
    hash::{BuildHasher, Hasher},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    error::{Error, Result},
    tally::Tally,
    vault::profile::Profile,
    vault::wad::{self, Wad},
};

pub const DEFAULT_PORT: u16 = 8080;
/// The letters a generated join code is made of: lowercase and digits, minus the ones that are
/// easy to misread (`0`/`o`, `1`/`l`/`i`), since the host reads it out to the other players.
const CODE_ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
/// How long a generated join code is: 31^6 is about 900 million codes, plenty against scanners.
pub const CODE_LENGTH: usize = 6;
/// The longest code a host may choose, so it stays something people can type.
pub const MAX_CODE_LENGTH: usize = 16;
/// How slowly a wad may arrive before its download is given up on, so a dead connection
/// doesn't hold a download thread forever. Generous, so only a stalled transfer trips it.
const MIN_BYTES_PER_SECOND: u64 = 16 * 1024;

/// Upper bound on wads a host may ask a joiner to download.
pub const MAX_FILES: usize = 255;

/// Where a host's wads are served, and the code that unlocks them.
///
/// Everything is requested under `/<code>/`, so someone who finds the port open still can't fetch
/// the manifest or the wads. The `host` alone is what UZDoom joins; the `port` is only tdl's.
#[derive(Clone, Debug, PartialEq)]
pub struct Address {
    pub host: String,
    pub port: u16,
    pub code: String,
}

impl Address {
    /// The URL of `path` (no leading slash) on the host.
    fn url(&self, path: &str) -> String {
        format!("http://{}:{}/{}/{path}", self.host, self.port, self.code)
    }
}

/// A fresh join code, for a host that didn't choose one.
///
/// The randomness comes from the standard library's per-process hash keys, mixed with the time,
/// so no random-number crate is needed. It only has to be unguessable to someone scanning ports.
pub fn random_code() -> String {
    let mut hasher = std::hash::RandomState::new().build_hasher();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| time.as_nanos());
    hasher.write_u128(nanos);
    let mut bits = hasher.finish();
    (0..CODE_LENGTH)
        .map(|_| {
            let letter = CODE_ALPHABET[(bits % CODE_ALPHABET.len() as u64) as usize];
            bits /= CODE_ALPHABET.len() as u64;
            letter as char
        })
        .collect()
}

/// Whether `code` can be a join code: lowercase letters and digits, so it is always safe in a URL.
pub fn is_valid_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= MAX_CODE_LENGTH
        && code
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// Compares two codes in constant time, by comparing their hashes (`blake3::Hash` equality is
/// constant time), so the server's answer time says nothing about how close a guess was.
fn codes_match(given: &str, expected: &str) -> bool {
    blake3::hash(given.as_bytes()) == blake3::hash(expected.as_bytes())
}

/// What a host serves at `/manifest`: its wads in load order, identified by hash.
///
/// Files are then requested by hash at `/file/<hash>`. The manifest comes from an untrusted peer,
/// so [`Manifest::fetch`] validates it and downloads are verified against it.
#[derive(Deserialize, Serialize)]
pub struct Manifest {
    pub entries: Vec<Entry>,
    pub base_wad_hash: String,
}

impl Manifest {
    /// Describes `wads` for joiners. Fails if a wad can't be read, since the host couldn't serve it.
    pub fn build(wads: &[Wad], base_wad_hash: String) -> Result<Self> {
        let entries = wads
            .iter()
            .map(|w| {
                let path = Path::new(&w.path);
                // 🌍 text, so a joiner on another OS can read it; a name that isn't valid UTF-8
                // (rare) goes out with its odd bytes replaced, which only costs matching it by name
                let name = path
                    .file_name()
                    .ok_or(Error::Download("Invalid wad path"))?
                    .to_string_lossy()
                    .into_owned();
                let size = std::fs::metadata(path)?.len();
                let hash = w.hash.clone();
                Ok(Entry::new(name, size, hash))
            })
            .collect::<Result<_>>()?;
        Ok(Self {
            entries,
            base_wad_hash,
        })
    }

    /// Downloads and validates the host's manifest. Rejects too many files and unsafe names or hashes.
    /// A host that turns the code down is [`Error::Refused`], so the user can be told why.
    pub fn fetch(address: &Address) -> Result<Self> {
        let response = match new_agent().get(&address.url("manifest")).call() {
            Err(ureq::Error::StatusCode(403)) => return Err(Error::Refused),
            other => other?,
        };
        let mut body = Vec::new();
        response.into_body().into_reader().read_to_end(&mut body)?;
        let manifest: Manifest = rmp_serde::from_slice(&body)?;
        if manifest.entries.len() > MAX_FILES {
            return Err(Error::Download("Too many files trying to be downloaded"));
        }
        if !manifest.entries.iter().all(Entry::is_safe) {
            return Err(Error::Download("Host sent an invalid file list"));
        }
        Ok(manifest)
    }

    /// Serves until the returned guard is dropped, only to requests that carry `code`.
    /// `paths` must be in the same order as `entries`.
    pub fn serve(self, port: u16, code: String, paths: Vec<OsString>) -> Result<ServerGuard> {
        let manifest =
            rmp_serde::to_vec(&self).map_err(|_| Error::Download("Could not encode file list"))?;
        let server = tiny_http::Server::http(("0.0.0.0", port))
            .map_err(|_| Error::Download("Could not start the host server"))?;
        // 🔌 the port actually bound, which differs from `port` only when that is 0 (any free one)
        let port = server
            .server_addr()
            .to_ip()
            .map_or(port, |address| address.port());
        let should_quit = Arc::new(AtomicBool::new(false));
        let flag = should_quit.clone();
        let thread = thread::spawn(move || {
            while !flag.load(Ordering::SeqCst) {
                let request = match server.recv_timeout(Duration::from_millis(250)) {
                    Ok(Some(request)) => request,
                    Ok(None) => continue,
                    Err(_) => break,
                };
                let url = request.url().to_string();
                // 🔐 /<code>/<what>: anything without the right code is turned away at the door
                let Some((given, url)) = url.trim_start_matches('/').split_once('/') else {
                    let _ = request.respond(
                        tiny_http::Response::from_string("not found").with_status_code(404),
                    );
                    continue;
                };
                if !codes_match(given, &code) {
                    let _ = request.respond(
                        tiny_http::Response::from_string("wrong code").with_status_code(403),
                    );
                    continue;
                }

                if url == "manifest" {
                    let _ = request.respond(tiny_http::Response::from_data(manifest.clone()));
                } else if let Some(hash) = url.strip_prefix("file/") {
                    let path = self
                        .entries
                        .iter()
                        .position(|e| e.hash == hash)
                        .map(|i| paths[i].clone());

                    thread::spawn(move || {
                        let _ = match path.and_then(|p| std::fs::File::open(p).ok()) {
                            Some(file) => request.respond(tiny_http::Response::from_file(file)),
                            None => request.respond(
                                tiny_http::Response::from_string("not found").with_status_code(404),
                            ),
                        };
                    });
                } else {
                    let _ = request.respond(
                        tiny_http::Response::from_string("not found").with_status_code(404),
                    );
                }
            }
        });
        Ok(ServerGuard {
            port,
            should_quit,
            thread: Some(thread),
        })
    }

    /// An empty hash means the host has no base wad selected, so anything matches.
    pub fn base_wad_matches(&self, base_wad: Option<&Wad>) -> bool {
        self.base_wad_hash.is_empty() || base_wad.is_none_or(|w| w.hash == self.base_wad_hash)
    }

    /// Compatible means identical wads in identical order; paths don't matter. A volatile wad
    /// matches by file name instead of hash.
    pub fn is_compatible_with(&self, profile: &Profile) -> bool {
        self.entries.len() == profile.wads.len()
            && self
                .entries
                .iter()
                .zip(&profile.wads)
                .all(|(entry, wad)| entry.hash == wad.hash || entry.is_same_name_as_volatile(wad))
    }

    /// The names of the volatile wads whose hash differs from the host's. A profile can be
    /// compatible and still have these, so the user is asked before joining with them.
    pub fn differing_wads(&self, profile: &Profile) -> Vec<String> {
        self.entries
            .iter()
            .zip(&profile.wads)
            .filter(|(entry, wad)| entry.hash != wad.hash)
            .map(|(_, wad)| wad.file_name().into_owned())
            .collect()
    }
}

/// One wad in a [`Manifest`].
#[derive(Clone, Deserialize, Serialize)]
pub struct Entry {
    pub name: String,
    pub size: u64,
    pub hash: String,
}

impl Entry {
    fn new(name: String, size: u64, hash: String) -> Self {
        Self { name, size, hash }
    }

    /// A volatile wad stands in for the host's wad of the same name, whatever its hash.
    fn is_same_name_as_volatile(&self, wad: &Wad) -> bool {
        wad.volatile && wad.file_name() == self.name
    }

    // name and hash become path components when downloading, so a host must not choose them freely
    /// Whether `name` is a bare file name and `hash` a BLAKE3 hex digest.
    fn is_safe(&self) -> bool {
        is_safe(&self.name, &self.hash)
    }
}

/// Whether a wad's `name` and `hash` from someone else (a host's manifest, a shared scroll) are
/// safe to join onto a real folder: the name a bare file name, so `../../x` can't climb out of
/// the folder, and the hash a BLAKE3 hex digest, since it becomes a folder name in the downloads.
pub fn is_safe(name: &str, hash: &str) -> bool {
    Path::new(name).file_name() == Some(OsStr::new(name))
        && hash.len() == 64
        && hash.bytes().all(|b| b.is_ascii_hexdigit())
}

/// An HTTP agent with timeouts short enough that an unreachable host fails quickly.
fn new_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(3)))
        .timeout_recv_response(Some(Duration::from_secs(10)))
        .build()
        .new_agent()
}

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

/// Stops and joins the server thread on drop, so the host is served for exactly as long as it's held.
pub struct ServerGuard {
    /// The port being served on.
    pub port: u16,
    should_quit: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        self.should_quit.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, hash: &str) -> Entry {
        Entry::new(name.into(), 1, hash.into())
    }

    fn manifest(entries: Vec<Entry>) -> Manifest {
        Manifest {
            entries,
            base_wad_hash: String::new(),
        }
    }

    fn wad(path: &str, hash: &str, volatile: bool) -> Wad {
        Wad {
            volatile,
            ..Wad::new(path.into(), hash.into(), 1)
        }
    }

    fn profile(wads: Vec<Wad>) -> Profile {
        Profile::new("p".into(), wads)
    }

    #[test]
    fn identical_wads_in_order_are_compatible() {
        let host = manifest(vec![entry("a.pk3", "h1"), entry("b.wad", "h2")]);
        let mine = profile(vec![
            wad("/x/a.pk3", "h1", false),
            wad("/y/b.wad", "h2", false),
        ]);
        assert!(host.is_compatible_with(&mine));
        assert!(host.differing_wads(&mine).is_empty());
    }

    #[test]
    fn a_different_hash_is_incompatible_unless_the_wad_is_volatile() {
        let host = manifest(vec![entry("a.pk3", "h1")]);
        assert!(!host.is_compatible_with(&profile(vec![wad("/x/a.pk3", "other", false)])));

        let volatile = profile(vec![wad("/x/a.pk3", "other", true)]);
        assert!(host.is_compatible_with(&volatile));
        // ... but the user still gets told
        assert_eq!(host.differing_wads(&volatile), ["a.pk3"]);
    }

    #[test]
    fn a_volatile_wad_only_stands_in_for_a_wad_of_the_same_name() {
        let host = manifest(vec![entry("a.pk3", "h1")]);
        let imposter = profile(vec![wad("/x/b.pk3", "other", true)]);
        assert!(!host.is_compatible_with(&imposter));
    }

    #[test]
    fn the_number_of_wads_must_still_match() {
        let host = manifest(vec![entry("a.pk3", "h1"), entry("b.wad", "h2")]);
        assert!(!host.is_compatible_with(&profile(vec![wad("/x/a.pk3", "h1", true)])));
    }

    #[test]
    fn a_matching_volatile_wad_is_not_reported_as_differing() {
        let host = manifest(vec![entry("a.pk3", "h1")]);
        assert!(
            host.differing_wads(&profile(vec![wad("/x/a.pk3", "h1", true)]))
                .is_empty()
        );
    }

    // 🛰️ the whole trip over a real socket: a host serves, a joiner fetches and downloads

    use std::{fs, net::TcpListener};

    use crate::vault::scribe::tests::Altar;

    /// Counts what a download reports, instead of drawing it.
    #[derive(Default)]
    struct Ledger {
        files: Vec<(String, u64)>,
        bytes: u64,
    }

    impl Tally for Ledger {
        fn count_file(&mut self, name: &OsStr, size: u64) -> Result<()> {
            self.files.push((name.to_string_lossy().into_owned(), size));
            Ok(())
        }

        fn count_bytes(&mut self, amount: u64) -> Result<()> {
            self.bytes += amount;
            Ok(())
        }
    }

    const CODE: &str = "letmein";

    /// The address of a host on this machine.
    fn local(port: u16, code: &str) -> Address {
        Address {
            host: "127.0.0.1".into(),
            port,
            code: code.into(),
        }
    }

    /// A port nobody is listening on right now.
    fn free_port() -> u16 {
        TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    /// Two wads in `altar`, one big enough to arrive in several chunks.
    fn host_wads(altar: &Altar) -> Vec<Wad> {
        fs::write(altar.file("small.wad"), b"a small wad").unwrap();
        fs::write(altar.file("big.pk3"), vec![7u8; 200_000]).unwrap();
        ["small.wad", "big.pk3"]
            .map(|name| Wad::from_path(&altar.file(name)).unwrap())
            .to_vec()
    }

    #[test]
    fn a_joiner_downloads_exactly_what_the_host_serves() {
        let (host, joiner) = (Altar::new("net-host"), Altar::new("net-joiner"));
        let wads = host_wads(&host);
        let manifest = Manifest::build(&wads, String::new()).unwrap();
        // port 0 lets the OS pick a free one, so tests running at once never collide
        let server = manifest
            .serve(
                0,
                CODE.into(),
                wads.iter().map(|w| w.path.clone()).collect(),
            )
            .unwrap();
        let port = server.port;
        let address = local(port, CODE);

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
    fn a_file_that_changes_after_hosting_starts_is_rejected_and_leaves_nothing_behind() {
        let (host, joiner) = (
            Altar::new("net-host-changed"),
            Altar::new("net-joiner-changed"),
        );
        let wads = host_wads(&host);
        let manifest = Manifest::build(&wads, String::new()).unwrap();
        // port 0 lets the OS pick a free one, so tests running at once never collide
        let server = manifest
            .serve(
                0,
                CODE.into(),
                wads.iter().map(|w| w.path.clone()).collect(),
            )
            .unwrap();
        let port = server.port;
        // same size, different bytes: only the hash can tell
        fs::write(host.file("small.wad"), b"A SMALL WAD").unwrap();
        let address = local(port, CODE);
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
    fn an_unreachable_host_is_an_error_not_a_hang() {
        let started = std::time::Instant::now();
        assert!(Manifest::fetch(&local(free_port(), CODE)).is_err());
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn the_wrong_code_is_refused_and_gets_nothing() {
        let host = Altar::new("net-host-code");
        let wads = host_wads(&host);
        let manifest = Manifest::build(&wads, String::new()).unwrap();
        // port 0 lets the OS pick a free one, so tests running at once never collide
        let server = manifest
            .serve(
                0,
                CODE.into(),
                wads.iter().map(|w| w.path.clone()).collect(),
            )
            .unwrap();
        let port = server.port;

        assert!(matches!(
            Manifest::fetch(&local(port, "guess")),
            Err(Error::Refused)
        ));
        // the files are behind the code too, not just the list of them
        let file = format!("http://127.0.0.1:{port}/guess/file/{}", wads[0].hash);
        assert!(matches!(
            new_agent().get(&file).call(),
            Err(ureq::Error::StatusCode(403))
        ));
        // and with no code at all there is nothing to find
        let bare = format!("http://127.0.0.1:{port}/manifest");
        assert!(new_agent().get(&bare).call().is_err());
    }

    #[test]
    fn a_cancelled_background_download_stops_and_says_so() {
        let (host, joiner) = (
            Altar::new("net-host-cancel"),
            Altar::new("net-joiner-cancel"),
        );
        let wads = host_wads(&host);
        let manifest = Manifest::build(&wads, String::new()).unwrap();
        // port 0 lets the OS pick a free one, so tests running at once never collide
        let server = manifest
            .serve(
                0,
                CODE.into(),
                wads.iter().map(|w| w.path.clone()).collect(),
            )
            .unwrap();
        let port = server.port;
        let address = local(port, CODE);
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
        let (host, joiner) = (Altar::new("net-host-bg"), Altar::new("net-joiner-bg"));
        let wads = host_wads(&host);
        let manifest = Manifest::build(&wads, String::new()).unwrap();
        // port 0 lets the OS pick a free one, so tests running at once never collide
        let server = manifest
            .serve(
                0,
                CODE.into(),
                wads.iter().map(|w| w.path.clone()).collect(),
            )
            .unwrap();
        let port = server.port;
        let address = local(port, CODE);
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
    fn urls_carry_the_port_and_the_code() {
        let address = Address {
            host: "10.0.0.2".into(),
            port: 9000,
            code: "abc234".into(),
        };
        assert_eq!(
            address.url("manifest"),
            "http://10.0.0.2:9000/abc234/manifest"
        );
    }

    #[test]
    fn generated_codes_are_valid_and_differ() {
        let code = random_code();
        assert_eq!(code.len(), CODE_LENGTH);
        assert!(is_valid_code(&code));
        assert!(code.bytes().all(|b| CODE_ALPHABET.contains(&b)));
        assert_ne!(random_code(), random_code());
    }

    #[test]
    fn only_lowercase_letters_and_digits_make_a_code() {
        assert!(is_valid_code("doom2"));
        assert!(!is_valid_code(""));
        assert!(!is_valid_code("Doom"));
        assert!(!is_valid_code("../x"));
        assert!(!is_valid_code(&"a".repeat(MAX_CODE_LENGTH + 1)));
    }

    #[test]
    fn codes_only_match_themselves() {
        assert!(codes_match("abc234", "abc234"));
        assert!(!codes_match("abc23", "abc234"));
    }

    #[test]
    fn big_files_get_longer_to_arrive() {
        assert_eq!(body_timeout(0), Duration::from_secs(30));
        assert_eq!(
            body_timeout(100 * MIN_BYTES_PER_SECOND),
            Duration::from_secs(130)
        );
    }
}
