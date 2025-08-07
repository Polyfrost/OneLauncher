use std::sync::LazyLock;

use onelauncher_entity::package::{PackageType, Provider};

use crate::api::packages::DatabaseModelExt;
use crate::api::packages::modpack::ModpackFormat;
use crate::api::packages::provider::ProviderExt;
use crate::api::{self};
use crate::error::{LauncherError, LauncherResult};

use super::super::provider::tests::init;

const DATA: LazyLock<Vec<(ModpackFormat, Vec<Entry>)>> = LazyLock::new(|| {
	vec![(
		ModpackFormat::MrPack,
		vec![Entry {
			slug: "sop",
			id: "BYfVnHa7",
			version_id: "nciq4FNy",
			provider: Provider::Modrinth,
		}],
	)]
});

struct Entry {
	pub id: &'static str,
	pub slug: &'static str,
	pub version_id: &'static str,
	pub provider: Provider,
}

#[tokio::test]
pub async fn test_install_modpack() -> LauncherResult<()> {
	let cf = init!();

	for (format_kind, entries) in DATA.iter() {
		if format_kind == &ModpackFormat::CurseForge && !cf {
			continue;
		}

		for entry in entries {
			let provider = &entry.provider;

			let modpack_pkg = provider.get(entry.id).await?;

			assert_eq!(modpack_pkg.slug, entry.slug);
			assert_eq!(modpack_pkg.id, entry.id);
			assert_eq!(modpack_pkg.package_type, PackageType::ModPack);

			let modpack_version = provider
				.get_versions(&[entry.version_id.to_string()])
				.await?;
			let modpack_version = modpack_version.first().ok_or_else(|| {
				LauncherError::from(anyhow::anyhow!("no version found for {}", entry.version_id))
			})?;

			assert_eq!(modpack_version.version_id, entry.version_id);

			let modpack_model =
				api::packages::download_package(&modpack_pkg, &modpack_version, None, None).await?;
			let mut cluster = api::cluster::create_cluster(
				"Modpack Format Test",
				modpack_version.mc_versions.first().unwrap(),
				*modpack_version.loaders.first().unwrap(),
				None,
				None,
			)
			.await?;

			let modpack_format = ModpackFormat::from_file(modpack_model.path().await?).await?;
			api::packages::modpack::install_managed_modpack(
				&modpack_model,
				&modpack_format,
				&mut cluster,
				None,
				None,
			)
			.await?;

			assert_eq!(
				cluster.linked_modpack_hash,
				Some(modpack_model.hash.clone())
			);
		}
	}

	Ok(())
}
