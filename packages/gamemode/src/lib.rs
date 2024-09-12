#![cfg(target_os = "linux")]
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

use libc::pid_t;
use std::ffi::c_int;

#[link(name = "stub", kind = "static")]
extern "C" {
	fn gamemode_start_for_wrapper(pid: pid_t) -> c_int;
}

pub fn request_start_for_wrapper(pid: u32) -> Result<(), String> {
	match pid.try_into() {
		Ok(signed) => unsafe {
			let result = gamemode_start_for_wrapper(signed);
			if result == 0 {
				Ok(())
			} else {
				Err(format!(
					"failed to request gamemode for pid {}: {}",
					pid, result
				))
			}
		},
		Err(e) => Err(format!("failed to request gamemode for pid {}: {}", pid, e)),
	}
}
