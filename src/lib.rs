//! Public library surface for integration tests.
//!
//! Only the modules needed by `tests/` are re-exported here. The
//! application entry point lives in `src/main.rs` and is compiled as a
//! separate binary target.

pub mod config;
pub mod error;
pub mod transition;
