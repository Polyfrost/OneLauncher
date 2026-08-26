pub fn no_window(command: &mut std::process::Command) {
	#[cfg(windows)]
	{
		use std::os::windows::process::CommandExt;

		const CREATE_NO_WINDOW: u32 = 0x0800_0000;
		command.creation_flags(CREATE_NO_WINDOW);
	}

	#[cfg(not(windows))]
	let _ = command;
}