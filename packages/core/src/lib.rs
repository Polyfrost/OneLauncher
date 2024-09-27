//! # `OneLauncher`
//!
//! A library used as a core for our launcher and Rust APIs.

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
