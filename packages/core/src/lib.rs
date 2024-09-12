//! # `OneLauncher`
//!
//! A library used as a core for our launcher and Rust APIs.

#![warn(
	clippy::all,
	clippy::pedantic,
	clippy::correctness,
	clippy::perf,
	clippy::style,
	clippy::suspicious,
	clippy::complexity,
	clippy::nursery,
	clippy::unwrap_used,
	unused_qualifications,
	rust_2018_idioms,
	trivial_casts,
	trivial_numeric_casts,
	unused_allocation,
	clippy::unnecessary_cast,
	clippy::cast_lossless,
	clippy::cast_possible_truncation,
	clippy::cast_possible_wrap,
	clippy::cast_sign_loss,
	clippy::dbg_macro,
	unused_import_braces,
	clippy::deprecated_cfg_attr,
	clippy::separated_literal_suffix,
	deprecated
)]
#![forbid(deprecated_in_future)]
#![deny(unused_must_use)]
#![allow(
	clippy::missing_errors_doc,
	clippy::future_not_send,
	clippy::module_name_repetitions,
	clippy::struct_field_names,
	clippy::cast_precision_loss,
	clippy::significant_drop_tightening //tmp
)]

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
