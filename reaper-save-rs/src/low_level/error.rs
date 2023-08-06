use thiserror::Error;
#[derive(Debug, Error)]
pub enum Error {
    #[error("Writing value failed")]
    WriteError {
        #[from]
        source: std::fmt::Error,
    },
    #[error("Writing whitespace failed")]
    WriteWhitespaceError,
    #[error("Failed to parse:\n{report}")]
    ParseError { report: String },
}
pub type Result<T> = std::result::Result<T, Error>;
