use std::io;
use std::path::Path;
use std::process::Command;

pub fn formatter(file: &Path) -> io::Result<()> {
	Command::new("pnpm")
		.arg("eslint")
		.arg("--flag")
		.arg("unstable_ts_config")
		.arg("--fix")
		.arg(file)
		.output()
		.map(|_| ())
		.map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

const _: specta_typescript::FormatterFn = formatter;
