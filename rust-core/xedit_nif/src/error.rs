use thiserror::Error;

#[derive(Error, Debug)]
pub enum NifError {
    #[error("nifly library not found: searched {searched_paths:?}")]
    LibraryNotFound { searched_paths: Vec<String> },

    #[error("Failed to load nifly library: {0}")]
    LoadFailed(String),

    #[error("Missing symbol in nifly library: {0}")]
    MissingSymbol(String),

    #[error("NIF operation failed: {0}")]
    OperationFailed(String),

    #[error("Invalid NIF file: {0}")]
    InvalidFile(String),
}
