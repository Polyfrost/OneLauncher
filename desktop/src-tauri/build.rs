fn main() {
	tauri_build::try_build(
		tauri_build::Attributes::new()
			.codegen(tauri_build::CodegenContext::new())
			.plugin(
				"onelauncher",
				tauri_build::InlinedPlugin::new().commands(&[
					"login_msa",
					"refresh_client_manager",
					"launch_cluster",
					"create_cluster",
					"get_clusters",
					"get_cluster",
					"get_manifest",
                    "get_settings",
                    "set_settings"
				]),
			),
	)
	.expect("failed to run tauri-build")
}
