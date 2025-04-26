use onelauncher_entity::icon::{Icon, IconType};
use reqwest::Method;

use crate::{error::LauncherResult, store::Dirs};

use super::{crypto::HashAlgorithm, http, io};

/// Caches a icon on the filesystem. Name is the equivalent to the sha1 hash of the icon.
pub async fn cache_icon(icon: &Icon) -> LauncherResult<Icon> {
	Ok(match icon.get_type() {
		IconType::Cache => icon.clone(),
		IconType::Path => {
			let bytes = io::read(&**icon).await?;
			cache_icon_bytes(&bytes).await?
		},
		IconType::Url => {
			let bytes = http::fetch(Method::GET, icon).await?;
			cache_icon_bytes(&bytes).await?
		},
		IconType::Unknown => {
			return Err(anyhow::anyhow!(
				"icon type is unknown, cannot cache icon: {}",
				icon
			).into());
		},
	})
}

/// Caches a icon bytes on the filesystem. Name is the equivalent to the sha1 hash of the icon.
pub async fn cache_icon_bytes(bytes: &[u8]) -> LauncherResult<Icon> {
	let hash = HashAlgorithm::Sha1.hash(bytes).await?;

	let dir = Dirs::get_caches_dir().await?.join(&hash[0..2]);
	io::create_dir_all(&dir).await?;

	let path = dir.join(&hash);
	io::write(&path, bytes).await?;

	Ok(Icon::from_hash(hash))
}
