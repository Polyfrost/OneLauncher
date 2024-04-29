//! # OneLauncher
//!
//! A library used as a core for our launcher and Rust APIs.

#![warn(unused_import_braces, missing_debug_implementations)]
#![deny(unused_must_use)]

#[macro_use]
pub mod utils;

pub mod api;
pub mod constants;
pub mod error;
pub mod game;
pub mod logger;
pub mod store;

pub use api::proxy::{Ingress, IngressType, ProxyState};
pub use api::*;
pub use error::*;
pub use logger::start_logger;
pub use store::{InnerPathLinux, State};
