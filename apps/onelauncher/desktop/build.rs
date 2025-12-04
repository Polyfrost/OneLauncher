use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
	let output = Command::new("git")
		.args(&["rev-parse", "HEAD"])
		.output()
		.unwrap();
	let git_hash = String::from_utf8(output.stdout).unwrap();
	println!("cargo:rustc-env=GIT_HASH={}", git_hash);

	let start = SystemTime::now();
	let timestamp = start
		.duration_since(UNIX_EPOCH)
		.expect("Time went backwards")
		.as_secs();
	println!("cargo:rustc-env=BUILD_TIMESTAMP={}", timestamp);

	tauri_build::build();
}
