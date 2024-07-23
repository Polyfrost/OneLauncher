fn main() {
	tauri_build::try_build(
		tauri_build::Attributes::new()
			.codegen(tauri_build::CodegenContext::new())
			.plugin(
				"onelauncher",
				tauri_build::InlinedPlugin::new().commands(&[
					// User
					"begin_msa",
					"finish_msa",
					"get_users",
					"get_user",
					"remove_user",
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
					// Other
					"get_program_info",
				]),
			),
	)
	.expect("failed to run tauri-build")
}
