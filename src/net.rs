use std::{
    ffi::OsString,
    io::{Read, Write},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use serde::{Deserialize, Serialize};

use crate::{
    error::{Error, Result},
    popups::progress::Progress,
    vault::profile::Profile,
    vault::wad::Wad,
};

pub const DEFAULT_PORT: u16 = 8080;
/// Upper bound on wads a host may ask a joiner to download.
pub const MAX_FILES: usize = 255;

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
                let name = path
                    .file_name()
                    .ok_or(Error::Download("Invalid wad path"))?
                    .to_os_string();
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
    pub fn fetch(ip: &str) -> Result<Self> {
        let response = new_agent()
            .get(&format!("{}/manifest", base_url(ip)))
            .call()?;
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

    /// Serves until the returned guard is dropped. `paths` must be in the same order as `entries`.
    pub fn serve(self, port: u16, paths: Vec<OsString>) -> Result<ServerGuard> {
        let manifest =
            rmp_serde::to_vec(&self).map_err(|_| Error::Download("Could not encode file list"))?;
        let server = tiny_http::Server::http(("0.0.0.0", port))
            .map_err(|_| Error::Download("Could not start the host server"))?;
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

                if url == "/manifest" {
                    let _ = request.respond(tiny_http::Response::from_data(manifest.clone()));
                } else if let Some(hash) = url.strip_prefix("/file/") {
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
#[derive(Deserialize, Serialize)]
pub struct Entry {
    pub name: OsString,
    pub size: u64,
    pub hash: String,
}

impl Entry {
    fn new(name: OsString, size: u64, hash: String) -> Self {
        Self { name, size, hash }
    }

    /// A volatile wad stands in for the host's wad of the same name, whatever its hash.
    fn is_same_name_as_volatile(&self, wad: &Wad) -> bool {
        wad.volatile && std::path::Path::new(&wad.path).file_name() == Some(&self.name)
    }

    // name and hash become path components when downloading, so a host must not choose them freely
    /// Whether `name` is a bare file name and `hash` a BLAKE3 hex digest.
    fn is_safe(&self) -> bool {
        Path::new(&self.name).file_name() == Some(&self.name)
            && self.hash.len() == 64
            && self.hash.bytes().all(|b| b.is_ascii_hexdigit())
    }
}

/// An HTTP agent with timeouts short enough that an unreachable host fails quickly.
fn new_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(3)))
        .timeout_recv_response(Some(Duration::from_secs(10)))
        .build()
        .new_agent()
}

/// Strips any `:port` the user typed; the host always serves on [`DEFAULT_PORT`].
fn base_url(ip: &str) -> String {
    let host = ip.rsplit_once(":").map_or(ip, |(h, _)| h);
    format!("http://{host}:{DEFAULT_PORT}")
}

/// Downloads every entry in order, returning the wads at their local paths.
/// `progress` is told about each file before it starts, so its size is always the current one.
pub fn download(
    ip: &str,
    entries: &[Entry],
    downloads_dir: &Path,
    progress: &mut Progress,
) -> Result<Vec<Wad>> {
    let agent = new_agent();
    let mut wads = Vec::with_capacity(entries.len());
    for entry in entries {
        progress.count_file(&entry.name, entry.size)?;
        wads.push(download_entry(&agent, ip, entry, downloads_dir, progress)?);
    }
    Ok(wads)
}

/// Reuses a previous download if present. Otherwise downloads to `<name>.part` and renames it
/// only after verification, so a partial or wrong file is never mistaken for a finished one.
fn download_entry(
    agent: &ureq::Agent,
    ip: &str,
    entry: &Entry,
    downloads_dir: &Path,
    progress: &mut Progress,
) -> Result<Wad> {
    let downloads_subdir = downloads_dir.join(&entry.hash);
    let wad_path = downloads_subdir.join(&entry.name);
    if wad_path.exists() {
        // already on disk, so the bar jumps to full instead of staying empty
        progress.count_bytes(entry.size)?;
        return Ok(Wad::new(wad_path.into(), entry.hash.clone(), entry.size));
    }
    std::fs::create_dir_all(&downloads_subdir)?;
    let part = downloads_subdir.join({
        let mut part = entry.name.clone();
        part.push(".part");
        part
    });
    if let Err(error) = download_part(agent, ip, entry, &part, progress) {
        let _ = std::fs::remove_file(&part);
        return Err(error);
    }
    std::fs::rename(&part, &wad_path)?;
    Ok(Wad::new(wad_path.into(), entry.hash.clone(), entry.size))
}

/// Streams the file to `part`, failing if it grows past the manifest's size or its hash differs.
fn download_part(
    agent: &ureq::Agent,
    ip: &str,
    entry: &Entry,
    part: &Path,
    progress: &mut Progress,
) -> Result<()> {
    let url = format!("{}/file/{}", base_url(ip), entry.hash);
    let mut reader = agent.get(&url).call()?.into_body().into_reader();
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
}
