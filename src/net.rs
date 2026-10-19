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

#[cfg(test)]
pub(in crate::net) mod tests {
    use std::{fs, net::TcpListener};

    use super::{serve::ServerGuard, *};
    use crate::{loadout::scribe::tests::Altar, loadout::wad::Wad};

    pub(in crate::net) const CODE: &str = "letmein";

    /// The address of a host on this machine.
    pub(in crate::net) fn local(port: u16, code: &str) -> Address {
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
    pub(in crate::net) fn host_wads(altar: &Altar) -> Vec<Wad> {
        fs::write(altar.file("small.wad"), b"a small wad").unwrap();
        fs::write(altar.file("big.pk3"), vec![7u8; 200_000]).unwrap();
        ["small.wad", "big.pk3"]
            .map(|name| Wad::from_path(&altar.file(name)).unwrap())
            .to_vec()
    }

    /// Serves `wads` behind [`CODE`] until the guard is dropped, and says where.
    pub(in crate::net) fn host(wads: &[Wad]) -> (ServerGuard, Address) {
        let manifest = Manifest::build(wads, String::new()).unwrap();
        // port 0 lets the OS pick a free one, so tests running at once never collide
        let server = manifest
            .serve(
                0,
                CODE.into(),
                wads.iter().map(|w| w.path.clone()).collect(),
                None,
            )
            .unwrap();
        let address = local(server.port, CODE);
        (server, address)
    }

    #[test]
    fn an_unreachable_host_is_an_error_not_a_hang() {
        let started = std::time::Instant::now();
        assert!(Manifest::fetch(&local(free_port(), CODE)).is_err());
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn the_wrong_code_is_refused_and_gets_nothing() {
        let altar = Altar::new("net-host-code");
        let wads = host_wads(&altar);
        let (_server, address) = host(&wads);
        let port = address.port;

        assert!(matches!(
            Manifest::fetch(&local(port, "guess")),
            Err(crate::error::Error::Refused)
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
    fn a_shared_ini_is_served_behind_the_code_and_an_unshared_one_is_not_there() {
        let read = |address: &Address| -> Option<Vec<u8>> {
            let mut body = Vec::new();
            let response = new_agent().get(&address.url("ini")).call().ok()?;
            std::io::Read::read_to_end(&mut response.into_body().into_reader(), &mut body).unwrap();
            Some(body)
        };
        let manifest = Manifest::build(&[], String::new()).unwrap();
        let server = manifest
            .serve(0, CODE.into(), Vec::new(), Some(b"[Doom.Player]".to_vec()))
            .unwrap();
        assert_eq!(
            read(&local(server.port, CODE)).as_deref(),
            Some(&b"[Doom.Player]"[..])
        );
        assert_eq!(read(&local(server.port, "guess")), None);

        let altar = Altar::new("net-host-no-ini");
        let (_server, address) = host(&host_wads(&altar));
        assert!(matches!(
            new_agent().get(&address.url("ini")).call(),
            Err(ureq::Error::StatusCode(404))
        ));
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
}
