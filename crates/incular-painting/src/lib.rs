//! Compatibility imports for Incular's renderer-neutral presentation API.
//!
//! New code should depend on [`incular_rendering`]. This crate remains as a
//! small, source-compatible transition layer for applications that adopted the
//! original `incular_painting` import path.

pub use incular_rendering::*;
