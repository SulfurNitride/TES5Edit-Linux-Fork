use thiserror::Error;

use crate::Signature;

#[derive(Error, Debug)]
pub enum DomError {
    #[error("Invalid plugin magic: expected TES3 or TES4, got {0:?}")]
    InvalidMagic([u8; 4]),

    #[error("Unexpected end of file at offset {offset}")]
    UnexpectedEof { offset: u64 },

    #[error("Invalid record signature: {0:?}")]
    InvalidSignature(Signature),

    #[error("Record decompression failed for {sig} at offset {offset}: {source}")]
    DecompressionFailed {
        sig: Signature,
        offset: u64,
        source: std::io::Error,
    },

    #[error("Group type {group_type} is invalid at offset {offset}")]
    InvalidGroupType { group_type: u32, offset: u64 },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
