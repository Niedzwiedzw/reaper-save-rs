use super::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Expected [{expected}], got [{got}] ")]
    InvalidObject {
        expected: AttributeName,
        got: AttributeName,
    },
    #[error("This is an empty project and I have no idea where to put stuff in that case.")]
    EmptyProject,
    #[error("Low level error occurred: {source}")]
    LowLevel {
        #[from]
        source: crate::low_level::error::Error,
    },
    #[error("Expected attribute {attribute:?} is missing.")]
    MissingAttribute { attribute: AttributeName },
}
pub type Result<T> = std::result::Result<T, self::Error>;
