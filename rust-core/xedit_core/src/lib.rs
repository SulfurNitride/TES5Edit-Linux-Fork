//! Core xEdit engine - plugin loading, conflict detection, editing.
//!
//! This crate ties together the DOM, I/O, game definitions, NIF handling,
//! and tools into a unified API that the FFI layer exposes to the Pascal GUI.

pub mod conflicts;
pub mod engine;
pub mod load_order;
pub mod mo2;

pub use engine::XEditEngine;
