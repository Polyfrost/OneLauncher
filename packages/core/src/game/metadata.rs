use interpulse::api::{minecraft::{Version, VersionInfo}, modded::LoaderVersion};

use crate::{api::ingress::{init_ingress, send_ingress_ref_opt}, error::LauncherResult, store::{ingress::{IngressRef, IngressType}, Dirs}, utils::{http::fetch_json_advanced, io}};

#[tracing::instrument]
pub async fn download_minecraft(version: &VersionInfo) -> LauncherResult<()> {
	let id = init_ingress(IngressType::MinecraftDownload, "Downloading Minecraft", 100.0).await?;

	// tokio::try_join! {

	// }?;

	Ok(())
}

#[tracing::instrument]
pub async fn download_version_info(
	version: &Version,
	loader: Option<&LoaderVersion>,
	force: Option<bool>,
	ingress_ref: Option<&IngressRef<'_>>,
) -> LauncherResult<VersionInfo> {
	let version_id = loader.map_or(version.id.clone(), |it| format!("{}-{}", version.id, it.id));

	let ingress_ref = ingress_ref.map(|i| i.with_increment(i.increment_by / 3.33));
	let ingress_ref = ingress_ref.as_ref();

	tracing::debug!("loading minecraft version info for minecraft version {version_id}");

	let path = Dirs::get_version_infos_dir().await?.join(format!("{version_id}.json"));

	let result = if path.exists() && !force.unwrap_or(false) {
		let data = io::read(path).await?;
		serde_json::from_slice(&data)?
	} else {
		tracing::info!(
			"downloading minecraft version info for minecraft version {}",
			&version_id
		);

		let mut info = fetch_json_advanced(
			reqwest::Method::GET,
			&version.url,
			None,
			None,
			None,
			ingress_ref
		).await?;

		if let Some(loader) = loader {
			let partial: interpulse::api::modded::PartialVersionInfo = fetch_json_advanced(
				reqwest::Method::GET,
				&loader.url,
				None,
				None,
				None,
				ingress_ref
			).await?;

			info = interpulse::api::modded::merge_partial_version(partial, info);
		}

		info.id.clone_from(&version_id);

		io::write(&path, &serde_json::to_vec(&info)?).await?;
		info
	};

	send_ingress_ref_opt(ingress_ref).await?;

	tracing::debug!("loaded minecraft version info for minecraft version {version_id}");
	Ok(result)
}