
use onelauncher_entity::package::Provider;

use crate::{api::packages::{self, provider::ProviderExt}, error::LauncherResult, store::{Core, CoreOptions, Dirs}};

#[tokio::test]
pub async fn test_get_provider() -> LauncherResult<()> {
	Core::initialize(CoreOptions::default()).await?;

	let provider = Provider::Modrinth;

	let res = provider.get("oneconfig").await?;

	assert_eq!(res.id, "AibBIVmj");

	Ok(())
}

#[tokio::test]
pub async fn test_get_multiple() -> LauncherResult<()> {
	Core::initialize(CoreOptions::default()).await?;

	let provider = Provider::Modrinth;

	let res = provider.get_multiple(&["oneconfig".to_string(), "chatting".to_string()]).await?;

	assert_eq!(res.len(), 2);

	Ok(())
}

#[tokio::test]
pub async fn test_get_versions() -> LauncherResult<()> {
	Core::initialize(CoreOptions::default()).await?;

	let provider = Provider::Modrinth;

	let res = provider.get_versions(&["ZvlCAdEF".to_string(), "GQANlg7p".to_string()]).await?;

	assert_eq!(res.len(), 2);

	Ok(())
}

#[tokio::test]
pub async fn test_download_chatting() -> LauncherResult<()> {
	Core::initialize(CoreOptions::default()).await?;

	let provider = Provider::Modrinth;

	let pkg = provider.get("chatting").await?;
	let ver_id = pkg.version_ids.first().expect("No version found");
	let versions = provider.get_versions(&[ver_id.clone()]).await?;
 	let ver = versions.first().expect("No version found");

	let db_model = packages::download_package(&pkg, ver, None).await;
	assert!(db_model.is_ok_and(|m| m.display_name == pkg.name));

	let dir = Dirs::get_package_dir(&pkg.package_type, &provider, &pkg.id).await?;
	let dest = dir.join(&ver.files.first().unwrap().file_name);

	assert!(dest.exists(), "File does not exist: {dest:?}");

	Ok(())
}

// TODO: Add more tests
