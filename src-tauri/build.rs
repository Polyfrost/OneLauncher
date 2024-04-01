use tauri_build::{Attributes, InlinedPlugin};

fn main() {
	let _ = tauri_build::try_build(Attributes::new().plugin(
		"onelauncher",
		InlinedPlugin::new().commands(&[
			"login_msa",
			"refresh_client_manager",
            "launch_cluster",
			"create_cluster",
			"get_clusters",
			"get_cluster",
			"get_manifest",
		]),
	));
}
