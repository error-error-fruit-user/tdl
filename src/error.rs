use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Program ended abruptly!")]
    Exit,

    #[error("IO Error: {0}!")]
    Io(#[from] std::io::Error),
}
