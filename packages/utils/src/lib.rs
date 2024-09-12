//! **`OneLauncher` Utilities**
//! Standard utilities for use within all Rust subprojects.
//!
//! - [`logging`]: Async utilities for log4j parsing with [`nom`].
//! - [`platform`]: Async utilities for managing OS-specific [`interpulse`] operations and rules.
//! - [`io`]: Async wrapper around [`tokio::fs`] and [`std::io::Error`] for our error system.

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
	clippy::module_name_repetitions
)]

pub mod io;
pub mod logging;
pub mod platform;
pub mod prisma;

/// Simple macro that takes a mutable reference and inserts it into a codeblock closure
/// as an owned reference.
///
/// mutable reference gets epically owned by free thinking macro!!!!! (not clickbait)
/// im going insane insane insane insane insane insane insane insane insane
#[macro_export]
macro_rules! ref_owned {
	($id:ident = $init:expr => $transform:block) => {{
		let mut it = $init;
		{
			let $id = &mut it;
			$transform;
		}
		it
	}};
}

/// Combines an iterator of `T` and an iterator of [`Option<T>`],
/// removing and ensuring the safety of any [`None`] values in the process.
pub fn chain_iterator<T>(
	required: impl IntoIterator<Item = T>,
	optional: impl IntoIterator<Item = Option<T>>,
) -> Vec<T> {
	required
		.into_iter()
		.map(Some)
		.chain(optional)
		.flatten()
		.collect()
}

#[inline]
#[must_use]
pub fn test_type_of<T>(_: T) -> &'static str {
	std::any::type_name::<T>()
}
