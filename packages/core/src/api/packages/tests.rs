use onelauncher_entity::package::Provider;

use crate::{api::packages::provider::ProviderExt, error::LauncherResult, store::{Core, CoreOptions}};

#[tokio::test]
pub async fn test_get_provider() -> LauncherResult<()> {
	Core::initialize(CoreOptions::default()).await?;

	let provider = Provider::Modrinth;

	let res = provider.get("oneconfig").await?;

	assert_eq!(res.provider_id, "AibBIVmj");

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

// TODO: Add more tests
