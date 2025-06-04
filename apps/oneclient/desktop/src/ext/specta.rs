use std::io;
use std::path::Path;
use std::process::Command;

pub fn formatter(file: &Path) -> io::Result<()> {
	let result = Command::new("pnpm")
		.arg("eslint")
		.arg("--flag")
		.arg("unstable_ts_config")
		.arg("--fix")
		.arg(file)
		.output();

	match result {
		Ok(_) => Ok(()),
		Err(e) => {
			eprintln!("error running formatter on {}: {}", file.display(), e);
			Ok(())
		}
	}
}

const _: specta_typescript::FormatterFn = formatter;
