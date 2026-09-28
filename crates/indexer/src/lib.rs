//! Index construction and reading APIs.
//!
//! The command-line binary is intentionally kept thin. The indexing
//! implementation and the reader used by the query crate belong here.

pub mod config;

pub use config::Config;
