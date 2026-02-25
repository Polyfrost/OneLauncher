#![cfg(target_os = "linux")]

use libc::pid_t;
use std::ffi::c_int;

#[link(name = "stub", kind = "static")]
unsafe extern "C" {
	unsafe fn gamemode_start_for_wrapper(pid: pid_t) -> c_int;
}

pub fn request_start_for_wrapper(pid: u32) -> Result<(), String> {
	match pid.try_into() {
		Ok(signed) => unsafe {
			let result = gamemode_start_for_wrapper(signed);
			if result == 0 {
				Ok(())
			} else {
				Err(format!(
					"failed to request gamemode for pid {pid}: {result}"
				))
			}
		},
		Err(e) => Err(format!("failed to request gamemode for pid {pid}: {e}")),
	}
}
