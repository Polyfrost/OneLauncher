use tauri_build::{Attributes, InlinedPlugin};

fn main() {
	let _ = tauri_build::try_build(
		Attributes::new()
			.plugin(
				"launcher-core",
				InlinedPlugin::new().commands(&[
                    "login_msa",
                    "refresh_client_manager",
                    "create_instance",
                    "get_instances",
                    "get_instance",
                    "get_manifest"
                ]),
			),
	);
}
