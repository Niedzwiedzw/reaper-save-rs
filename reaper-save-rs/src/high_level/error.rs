use super::*;
use low_level::AttributeKind;
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
    #[error("Invalid attribute kind for [{field}]: expected [{expected:?}], found [{found:?}]")]
    InvalidAttributeType {
        field: &'static str,
        expected: AttributeKind,
        found: AttributeKind,
    },
    #[error("Ttem has source wave")]
    NoSourceFile,
}
pub type Result<T> = std::result::Result<T, self::Error>;
