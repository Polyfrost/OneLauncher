fn main() {
	tauri_build::try_build(
		tauri_build::Attributes::new()
			.codegen(tauri_build::CodegenContext::new())
			.plugin(
				"onelauncher",
				tauri_build::InlinedPlugin::new().commands(&[
					// User
					"auth_login",
					"get_users",
					"get_user",
					"remove_user",
					"get_default_user",
					"set_default_user",
					// Cluster
					"create_cluster",
					"remove_cluster",
					"get_cluster",
					"get_clusters",
					"run_cluster",
					// Processor
					"get_running_clusters",
					"get_processes_by_path",
					"kill_process",
					// Settings
					"get_settings",
					"set_settings",
					// Metadata
					"get_minecraft_versions",
					// Package
					"random_mods",
					"get_mod",
					"download_mod",
					// Other
					"get_program_info",
				]),
			),
	)
	.expect("failed to run tauri-build")
}
