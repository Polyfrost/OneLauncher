use std::sync::LazyLock;

use onelauncher_entity::package::{PackageType, Provider};

use crate::api::packages;
use crate::api::packages::provider::ProviderExt;
use crate::error::LauncherResult;
use crate::utils::DatabaseModelExt;

macro_rules! init {
	() => {{
		crate::store::Core::initialize(crate::store::CoreOptions::default()).await?;
		if crate::store::Core::get().curseforge_api_key.is_none() {
			eprintln!("CURSEFORGE_API_KEY is not set, skipping tests that require it.");
			false
		} else {
			true
		}
	}};
}

pub(crate) use init;

struct Entry {
	slug: &'static str,
	id: &'static str,
	version_id: &'static str,
	package_type: PackageType,
}

static DATA: LazyLock<Vec<(Provider, Vec<Entry>)>> = LazyLock::new(|| {
	vec![
		(
			Provider::CurseForge,
			vec![
				Entry {
					slug: "oneconfig",
					id: "1148006",
					version_id: "5938441",
					package_type: PackageType::Mod,
				},
				Entry {
					slug: "complementary-reimagined",
					id: "627557",
					version_id: "6515577",
					package_type: PackageType::Shader,
				},
				Entry {
					slug: "tschipcrafts-dynamic-lights",
					id: "831385",
					version_id: "6725286",
					package_type: PackageType::DataPack,
				},
				Entry {
					slug: "fresh-animations",
					id: "453763",
					version_id: "6528594",
					package_type: PackageType::ResourcePack,
				},
				Entry {
					slug: "all-the-mods-10",
					id: "925200",
					version_id: "6826503",
					package_type: PackageType::ModPack,
				},
			],
		),
		(
			Provider::Modrinth,
			vec![
				Entry {
					slug: "oneconfig",
					id: "AibBIVmj",
					version_id: "YofF8Rpk",
					package_type: PackageType::Mod,
				},
				Entry {
					slug: "complementary-reimagined",
					id: "HVnmMxH1",
					version_id: "sAAjYvFB",
					package_type: PackageType::Shader,
				},
				Entry {
					slug: "veinminer",
					id: "OhduvhIc",
					version_id: "gZ4v72II",
					package_type: PackageType::Mod,
				},
				Entry {
					slug: "fresh-animations",
					id: "50dA9Sha",
					version_id: "9LtDLleW",
					package_type: PackageType::ResourcePack,
				},
				Entry {
					slug: "fabulously-optimized",
					id: "1KVo5zza",
					version_id: "U9MwqSo1",
					package_type: PackageType::ModPack,
				},
			],
		),
	]
});

#[tokio::test]
pub async fn test_get() -> LauncherResult<()> {
	let cf = init!();

	for (provider, entries) in DATA.iter() {
		if provider == &Provider::CurseForge && !cf {
			continue;
		}

		for entry in entries {
			let res = provider.get(entry.id).await?;

			assert_eq!(res.slug, entry.slug);
			assert_eq!(res.id, entry.id);
			assert_eq!(res.package_type, entry.package_type);
		}
	}

	Ok(())
}

#[tokio::test]
pub async fn test_get_multiple() -> LauncherResult<()> {
	let cf = init!();

	for (provider, entries) in DATA.iter() {
		if provider == &Provider::CurseForge && !cf {
			continue;
		}

		let ids = entries
			.iter()
			.map(|e| e.id.to_string())
			.collect::<Vec<String>>();
		let res = provider.get_multiple(&ids).await?;

		assert_eq!(res.len(), entries.len());
	}

	Ok(())
}

#[tokio::test]
pub async fn test_get_versions() -> LauncherResult<()> {
	let cf = init!();

	for (provider, entries) in DATA.iter() {
		if provider == &Provider::CurseForge && !cf {
			continue;
		}

		let slugs = entries
			.iter()
			.map(|e| e.version_id.to_string())
			.collect::<Vec<String>>();

		let res = provider
			.get_versions(&slugs.iter().map(|s| s.to_string()).collect::<Vec<String>>())
			.await?;

		assert_eq!(res.len(), slugs.len());
	}

	Ok(())
}

#[tokio::test]
pub async fn test_download_packages() -> LauncherResult<()> {
	let cf = init!();

	for (provider, entries) in DATA.iter() {
		if provider == &Provider::CurseForge && !cf {
			continue;
		}

		for entry in entries {
			if entry.package_type == PackageType::ModPack {
				continue; // TODO: Skip modpacks for now
			}

			let pkg = provider.get(entry.id).await?;
			let ver_id = entry.version_id.to_string();
			let versions = provider.get_versions(&[ver_id.clone()]).await?;
			let ver = versions.first().expect("No version found");

			let db_model = packages::download_package(&pkg, ver, None, None).await?;
			assert!(db_model.hash == ver.files.first().unwrap().sha1);

			let dest = db_model.path().await?;

			assert!(dest.exists(), "File does not exist: {dest:?}");
		}
	}

	Ok(())
}

// TODO: Add more tests
