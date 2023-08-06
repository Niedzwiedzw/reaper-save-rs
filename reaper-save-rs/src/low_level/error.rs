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
    #[error("Param {param} not found in object")]
    ObjectNoSuchParam { param: String },
    #[error("Expected for object parameter to have {expected} attributes, but it has {found}")]
    BadParamCount { expected: usize, found: usize },
}
pub type Result<T> = std::result::Result<T, Error>;
