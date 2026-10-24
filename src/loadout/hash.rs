use std::{
    fs::File,
    io::{ErrorKind, Read},
    path::Path,
};

use crate::{error::Result, tally::Tally};

/// How much of the file is read between reports.
const CHUNK: usize = 1024 * 1024;

/// The hex-encoded BLAKE3 hash of a file's contents, read in a stream. `tally` hears about every
/// chunk, so a big file can show its progress, and can stop the hashing by failing.
pub fn hash(path: &Path, tally: &mut impl Tally) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0; CHUNK];
    loop {
        let read = match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        };
        hasher.update(&buffer[..read]);
        tally.count_bytes(read as u64)?;
    }
    Ok(hasher.finalize().to_hex().to_string())
}
