use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

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
}
