use std::path::Path;

use crate::error::Result;

pub fn hash(path: &Path) -> Result<String> {
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(std::fs::File::open(path)?)?;
    Ok(hasher.finalize().to_hex().to_string())
}
