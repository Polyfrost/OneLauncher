use tauri_build::{Attributes, InlinedPlugin};

fn main() {
	let _ = tauri_build::try_build(
		Attributes::new()
			.plugin("auth", InlinedPlugin::new().commands(&["login_msa"]))
			.plugin(
				"game",
				InlinedPlugin::new().commands(&["launch_game", "set_selected_client"]),
			),
	);
}
