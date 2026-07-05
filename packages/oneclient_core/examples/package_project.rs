
use std::env;

use oneclient_core::packages::domain::ProviderId;
use oneclient_core::{dev, logger, LauncherResult};

#[tokio::main]
async fn main() -> LauncherResult<()> {
	logger::init_debug()?;

	let mut args = env::args().skip(1);
	let provider_name = args.next().unwrap_or_else(|| "modrinth".into());
	let project_ref = args.next().unwrap_or_else(|| "sodium".into());
	let mc_version = args.next();

	let provider_id = parse_provider(&provider_name)?;
	let env = dev::ephemeral_services().await?;
	let provider = env.packages.get(provider_id)?;

	let project = provider.get_project(&project_ref, &env).await?;

	println!(
		"{:?} project: {} ({})",
		project.provider, project.name, project.id
	);
	println!("  {}\n", project.summary);

	let versions = provider
		.list_versions(
			&project.id,
			mc_version.as_deref(),
			None,
			0,
			5,
			&env,
		)
		.await?;

	println!("Versions (up to {}):", versions.items.len());
	for v in &versions.items {
		println!(
			"  {} - {} ({})",
			v.name, v.version_number, v.version_id
		);
	}

	if let Some(version) = versions.items.first() {
		let detail = provider
			.get_version(&project.id, &version.version_id, &env)
			.await?;
		if let Some(file) = detail.primary_file() {
			println!(
				"\nPrimary file: {} sha1={} cf_fp={:?}",
				file.file_name, file.sha1, file.fingerprint
			);
		}
	}

	Ok(())
}

fn parse_provider(name: &str) -> LauncherResult<ProviderId> {
	match name.to_lowercase().as_str() {
		"modrinth" | "mr" => Ok(ProviderId::Modrinth),
		"curseforge" | "cf" => Ok(ProviderId::CurseForge),
		other => Err(oneclient_core::LauncherError::InvalidSettingsProfile {
			reason: format!(
				"unknown provider {other:?}; use modrinth or curseforge"
			),
		}),
	}
}
