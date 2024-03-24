use tauri_build::{Attributes, InlinedPlugin};

fn main() {
	let _ = tauri_build::try_build(
		Attributes::new()
			.plugin(
				"launcher-core",
				InlinedPlugin::new().commands(&[
                    "login_msa",
                    "create_instance",
                    "get_instances",
                    "get_instance",
                ]),
			),
	);
}
