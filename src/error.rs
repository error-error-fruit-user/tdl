use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

/// `Exit` is not a failure: it is how Ctrl+C unwinds out of the nested rituals.
#[derive(Debug, Error)]
pub enum Error {
    #[error("Ghosted! 👻")]
    Exit,

    #[error("IO Error: {0}!")]
    Io(#[from] std::io::Error),

    #[error("HTTP Error: {0}!")]
    Http(#[from] ureq::Error),

    #[error("Decode Error: {0}!")]
    Decode(#[from] rmp_serde::decode::Error),

    #[error("Download Error: {0}!")]
    Download(&'static str),

    /// The user pressed Esc during a download. Like `Exit` it is not a failure, so nothing is shown.
    #[error("Cancelled")]
    Cancelled,

    /// The host turned down the join code.
    #[error("The host refused the join code!")]
    Refused,

    /// UZDoom itself could not be started. Unlike the others this is shown, not fatal.
    #[error("Could not start UZDoom: {0}!")]
    Spawn(std::io::Error),

    #[error("Upgrade Error: {0}!")]
    Upgrade(&'static str),
}
