use std::path::PathBuf;

fn main() {
	let paths = vec![get_root_workspace().join(".env"), PathBuf::from(".env")];

	for path in paths {
		if path.exists() {
			load_from_path(path);
		}
	}
}

fn get_root_workspace() -> PathBuf {
	let output: Vec<u8> = std::process::Command::new(env!("CARGO"))
		.arg("locate-project")
		.arg("--workspace")
		.arg("--message-format=plain")
		.output()
		.unwrap()
		.stdout;

	let root_workspace = String::from_utf8(output).unwrap();
	let mut root_workspace = PathBuf::from(root_workspace.trim());

	if root_workspace.is_file() {
		root_workspace.pop();
	}

	root_workspace
}

fn load_from_path(path: PathBuf) {
	let vars = dotenvy::EnvLoader::with_path(path)
		.load()
		.expect("failed to find .env file");

	for (key, value) in vars {
		println!("cargo:rustc-env={key}={value}");
	}
}
