//! Lossless binary-preserving DOM for Bethesda plugin files.
//!
//! The DOM preserves exact binary layout for unmodified records,
//! enabling byte-identical roundtrip when no edits are applied.

pub mod plugin;
pub mod group;
pub mod record;
pub mod subrecord;
pub mod element;
pub mod dialect;
pub mod signature;
pub mod formid;
pub mod error;

pub use plugin::Plugin;
pub use group::Group;
pub use record::Record;
pub use subrecord::Subrecord;
pub use element::Element;
pub use dialect::{GameDialect, GameId};
pub use signature::Signature;
pub use formid::FormId;
pub use error::DomError;
