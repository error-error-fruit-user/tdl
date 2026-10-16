//! Counting long work as it goes (downloading, hashing) for whatever shows it: the progress
//! window, or a test that counts.

use std::ffi::OsStr;

use crate::error::Result;

/// Whatever long work reports to as it goes. Either call can fail to stop the work, which is how
/// a cancelled progress window stops what it is showing.
pub trait Tally {
    /// A file is starting; `size` is its full size in bytes.
    fn count_file(&mut self, name: &OsStr, size: u64) -> Result<()>;
    /// `amount` more bytes of the current file are done.
    fn count_bytes(&mut self, amount: u64) -> Result<()>;
}

/// Counts nothing, for work nobody is watching.
impl Tally for () {
    fn count_file(&mut self, _: &OsStr, _: u64) -> Result<()> {
        Ok(())
    }

    fn count_bytes(&mut self, _: u64) -> Result<()> {
        Ok(())
    }
}
