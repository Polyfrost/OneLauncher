use serde::Serialize;

use crate::store::processes::Process;

#[onelauncher_macro::specta]
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ProcessPayload {
	Starting { command: String },
	Started { process: Process },
	Stopped { pid: u32, exit_code: i32 },
	Output { pid: u32, output: String },
}
