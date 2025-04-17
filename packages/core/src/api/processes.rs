use serde::Serialize;

use crate::store::processes::Process;

#[derive(Debug, Serialize)]
pub enum ProcessPayload {
	Started {
		process: Process,
	},
	Stopped {
		pid: u32,
		exit_code: i32,
	},
	Output {
		pid: u32,
		output: String,
	},
}