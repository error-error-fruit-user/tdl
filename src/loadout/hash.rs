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

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::loadout::scribe::tests::Altar;

    #[test]
    fn the_hash_is_blake3_and_every_byte_is_counted() {
        let altar = Altar::new("hash-counted");
        let bytes = vec![3u8; CHUNK + 10];
        fs::write(altar.file("a.wad"), &bytes).unwrap();
        struct Bytes(u64);
        impl Tally for Bytes {
            fn count_file(&mut self, _: &std::ffi::OsStr, _: u64) -> Result<()> {
                Ok(())
            }
            fn count_bytes(&mut self, amount: u64) -> Result<()> {
                self.0 += amount;
                Ok(())
            }
        }
        let mut counted = Bytes(0);
        let hash = hash(&altar.file("a.wad"), &mut counted).unwrap();
        assert_eq!(hash, blake3::hash(&bytes).to_hex().to_string());
        assert_eq!(counted.0, bytes.len() as u64);
    }
}
