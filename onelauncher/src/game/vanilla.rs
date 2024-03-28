use super::{
	client::{ClientTrait, Cluster, Manifest, Version},
	minecraft::MinecraftManifest,
};
use crate::{
	constants::MINECRAFT_VERSIONS_MANIFEST,
	create_game_client, impl_game_client,
	utils::{dirs, file, http},
};
use anyhow::anyhow;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

create_game_client! {
	VanillaClientProps {}

	VanillaClient {}
}

#[async_trait]
impl<'a> ClientTrait<'a> for VanillaClient<'a> {
	impl_game_client!();

	fn new(cluster: &'a Cluster, manifest: &'a Manifest) -> Self {
		VanillaClient { cluster, manifest }
	}

	async fn launch(&self) -> crate::Result<()> {
		Ok(())
	}

	async fn setup(&self) -> crate::Result<()> {
		Ok(())
	}

	async fn install_game(&self) -> crate::Result<PathBuf> {
		let manifest = &self.manifest.manifest;
		let file = dirs::clients_dir()?.join(format!("{}.jar", manifest.version));

		if !file.exists() {
			fs::create_dir_all(&file)?;
			let artifact = &manifest.downloads.client;

			http::download_file(&artifact.url, &file).await?;
			let file_hash = file::file_sha1(&file)?;

			println!("Downloaded: '{}' | '{}'", file_hash, artifact.sha1);
			if file_hash != artifact.sha1 {
				return Err(std::io::Error::new(
					std::io::ErrorKind::InvalidData,
					"Hashes do not match",
				)
				.into());
			}
		}

		Ok(file)
	}

	async fn install_libraries(&self) -> crate::Result<String> {
		Ok("".to_string())
	}

	async fn install_natives(&self) -> crate::Result<()> {
		Ok(())
	}

	async fn install_assets(&self) -> crate::Result<()> {
		Ok(())
	}
}

#[derive(Clone, Serialize, Deserialize)]
struct VersionList {
	versions: Vec<Version>,
}

#[derive(Serialize, Deserialize)]
struct CachedVersions {
	last_updated: i64,
	versions: Vec<Version>,
}

pub async fn get_versions(file: Option<&PathBuf>) -> crate::Result<Vec<Version>> {
	if let Some(file) = file {
		if file.exists() && file.is_file() {
			if let Some(versions) = get_cached_versions(file).await? {
				return Ok(versions);
			}
		}
	}

	let response = http::create_client()?
		.get(MINECRAFT_VERSIONS_MANIFEST)
		.send()
		.await?;
	let response = response.json::<VersionList>().await?;

	if let Some(file) = file {
		fs::write(
			file,
			serde_json::to_string(&CachedVersions {
				last_updated: chrono::Utc::now().timestamp(),
				versions: response.versions.clone(),
			})?,
		)?;
	}

	Ok(response.versions)
}

async fn get_cached_versions(file: &PathBuf) -> crate::Result<Option<Vec<Version>>> {
	let cached = serde_json::from_str::<CachedVersions>(&fs::read_to_string(file)?)?;
	let head_request = http::create_client()?
		.head(MINECRAFT_VERSIONS_MANIFEST)
		.send()
		.await?;

	let last_updated = head_request
		.headers()
		.get("Last-Modified")
		.ok_or(anyhow!("Last-Modified header not found"))?;
	let last_updated = last_updated
		.to_str()
		.map_err(|_| anyhow!("Failed to convert Last-Modified header to string"))?;
	let last_updated = chrono::DateTime::parse_from_rfc2822(last_updated)?;

	if cached.last_updated < last_updated.timestamp() {
		Ok(None)
	} else {
		Ok(Some(cached.versions))
	}
}

pub async fn retrieve_version_manifest(url: &str) -> crate::Result<MinecraftManifest> {
	let manifest = http::create_client()?
		.get(url)
		.send()
		.await?
		.json::<MinecraftManifest>()
		.await?;
	Ok(manifest)
}
