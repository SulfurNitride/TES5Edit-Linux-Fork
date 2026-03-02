//! Workspace-level integration test support crate.
//!
//! This thin crate re-exports the workspace members so that integration tests
//! under `tests/` can use them without being placed inside a specific subcrate.

pub use xedit_core;
pub use xedit_dom;
pub use xedit_games;
pub use xedit_io;
