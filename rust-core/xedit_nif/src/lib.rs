//! NIF mesh file handling via nifly C wrapper.
//!
//! nifly is loaded as a mandatory dependency at startup.
//! If the nifly shared library cannot be found, initialization fails fast.

pub mod api;
pub mod error;
pub mod loader;
pub mod metadata;
pub mod scanner;
pub mod validator;

pub use api::NifFile;
pub use error::NifError;
pub use loader::NiflyLibrary;
pub use metadata::{extract_metadata, NifMetadata};
pub use scanner::{normalize_texture_path, scan_directory_nifs, scan_nif_textures};
pub use validator::{validate_nif, ValidationIssue};
