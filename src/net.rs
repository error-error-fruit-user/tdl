//! Multiplayer over HTTP: the host's [`Manifest`] of wads, serving it ([`Manifest::serve`]), and
//! downloading from it ([`download_in_background`]). What they share lives here: the address and join code, and
//! the checks on names that come from someone else.

mod download;
pub mod ini;
mod manifest;
mod serve;

use std::{
    ffi::OsStr,
    hash::{BuildHasher, Hasher},
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub use self::{
    download::{Report, download, download_in_background},
    manifest::{Entry, Manifest},
};

pub const DEFAULT_PORT: u16 = 8080;
/// The letters a generated join code is made of: lowercase and digits, minus the ones that are
/// easy to misread (`0`/`o`, `1`/`l`/`i`), since the host reads it out to the other players.
const CODE_ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
/// How long a generated join code is: 31^6 is about 900 million codes, plenty against scanners.
pub const CODE_LENGTH: usize = 6;
/// The longest code a host may choose, so it stays something people can type.
pub const MAX_CODE_LENGTH: usize = 16;

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

/// Whether a wad's `name` and `hash` from someone else (a host's manifest, a shared scroll) are
/// safe to join onto a real dir: the name a bare file name, so `../../x` can't climb out of
/// the dir, and the hash a BLAKE3 hex digest, since it becomes a dir name in the downloads.
pub fn is_safe(name: &str, hash: &str) -> bool {
    is_safe_name(name) && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Whether `name` from someone else is a bare file name, the half of [`is_safe`] that needs no
/// hash: a folder on a scroll has none.
pub fn is_safe_name(name: &str) -> bool {
    Path::new(name).file_name() == Some(OsStr::new(name))
}

/// An HTTP agent with timeouts short enough that an unreachable host fails quickly.
fn new_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(3)))
        .timeout_recv_response(Some(Duration::from_secs(10)))
        .build()
        .new_agent()
}
