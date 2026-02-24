#[cfg(test)]
mod tests {
	use onelauncher_core::entity::loader::GameLoader;
	use onelauncher_core::store::{Core, CoreOptions};
	use std::path::PathBuf;
	use tokio::fs;

	use crate::oneclient::bundle_updates::{apply_bundle_updates, check_bundle_updates};
	use crate::oneclient::bundles::BundlesManager;
	use onelauncher_core::api::cluster::{create_cluster, dao};
	use onelauncher_core::utils::DatabaseModelExt;

	// We only run this if we ignore it by default, since it hits the network and modifies DB
	// but we can run it explicitly.
	#[tokio::test]
	#[ignore]
	async fn test_bundle_updates_e2e() {
		// 1. Initialize DB and paths
		Core::initialize(CoreOptions::default()).await.unwrap();

		// 2. Create a fresh cluster
		let cluster_name = format!(
			"Bundle Test {}",
			std::time::SystemTime::now()
				.duration_since(std::time::UNIX_EPOCH)
				.unwrap()
				.as_nanos()
		);
		let cluster = create_cluster(&cluster_name, "1.21.1", GameLoader::Fabric, None, None)
			.await
			.unwrap();

		let cluster_dir = cluster.path().await.unwrap();

		// Ensure BundlesManager is initialized and fetches remote bundles
		println!("Loaded BundlesManager: {:#?}", BundlesManager::get().await);
		let retrieved_bundles = BundlesManager::get()
			.await
			.get_bundles_for(&cluster.mc_version, cluster.mc_loader)
			.await
			.unwrap();
		println!(
			"get_bundles_for manually returned: {} bundles",
			retrieved_bundles.len()
		);
		for b in &retrieved_bundles {
			println!(
				"Bundle '{}' has {} files",
				b.manifest.name,
				b.manifest.files.len()
			);
		}

		// Subscribe the cluster to one of the bundles manually to trigger additions
		use sea_orm::{ActiveModelTrait, Set};

		let dummy_hash = uuid::Uuid::new_v4().to_string();
		let dummy_base_pkg = onelauncher_core::entity::packages::ActiveModel {
			hash: Set(dummy_hash.clone()),
			file_name: Set("dummy.jar".to_string()),
			version_id: Set("dummy_version".to_string()),
			display_name: Set("Dummy".to_string()),
			display_version: Set("1.0.0".to_string()),
			package_type: Set(onelauncher_core::entity::package::PackageType::Mod),
			provider: Set(onelauncher_core::entity::package::Provider::Modrinth),
			package_id: Set("dummy_id".to_string()),
			mc_versions: Set(onelauncher_core::entity::utility::DbVec(vec![
				"1.21.1".to_string(),
			])),
			mc_loader: Set(onelauncher_core::entity::utility::DbVec(vec![
				onelauncher_core::entity::loader::GameLoader::Fabric,
			])),
			published_at: Set(chrono::Utc::now()),
			..Default::default()
		};
		dummy_base_pkg
			.insert(&onelauncher_core::store::State::get().await.unwrap().db)
			.await
			.unwrap();

		let dummy_pkg = onelauncher_core::entity::cluster_packages::ActiveModel {
			cluster_id: Set(cluster.id),
			package_hash: Set(dummy_hash.clone()),
			bundle_name: Set(Some("OneClient 1.21.1 Fabric [Performance]".to_string())),
			..Default::default()
		};
		dummy_pkg
			.insert(&onelauncher_core::store::State::get().await.unwrap().db)
			.await
			.unwrap();

		// 3. First apply: should detect additions for all bundle packages
		let check1 = check_bundle_updates(cluster.id).await.unwrap();
		assert!(
			!check1.additions_available.is_empty(),
			"Expected fresh cluster to have bundle additions available"
		);
		assert!(check1.updates_available.is_empty(), "Expected 0 updates");
		assert!(check1.removals_available.is_empty(), "Expected 0 removals");

		let applied1 = apply_bundle_updates(cluster.id).await.unwrap();
		assert_eq!(
			applied1.additions_applied.len(),
			check1.additions_available.len()
		);

		// 4. Verify overrides were extracted
		// The 1.21.1 Fabric bundles usually include some config overrides (e.g. options.txt or config/...)
		// We'll write a custom file in the cluster to simulate a user creating a config
		let user_config = cluster_dir.join("config").join("user_custom.txt");
		fs::create_dir_all(user_config.parent().unwrap())
			.await
			.unwrap();
		fs::write(&user_config, b"user content").await.unwrap();

		// We will modify an *existing* extracted override (if one exists), or we just let it be.
		// For the sake of the test, let's artificially remove one package from the DB so the updater runs again
		let installed_pkgs =
			onelauncher_core::api::packages::bundle_dao::get_bundle_packages_for_cluster(
				cluster.id,
			)
			.await
			.unwrap();
		let hash_to_remove = installed_pkgs
			.into_iter()
			.find(|p| {
				p.package_hash != dummy_hash
					&& p.bundle_name.as_deref() == Some("OneClient 1.21.1 Fabric [Performance]")
			})
			.unwrap()
			.package_hash;

		// Remove it from the database and filesystem so the updater thinks it's missing.
		// record_override=false: this simulates a missing package, not a deliberate user removal.
		println!("Removing package hash: {}", hash_to_remove);
		onelauncher_core::api::packages::remove_package(cluster.id, hash_to_remove, false)
			.await
			.unwrap();

		// 5. Second apply: should detect 1 addition (the one we removed from DB)
		let check2 = check_bundle_updates(cluster.id).await.unwrap();
		println!(
			"check2 additions count: {}",
			check2.additions_available.len()
		);
		if check2.additions_available.len() != 1 {
			println!(
				"check2 additions: {:#?}",
				check2
					.additions_available
					.iter()
					.map(|a| &a.new_file.kind)
					.collect::<Vec<_>>()
			);
		}
		assert_eq!(
			check2.additions_available.len(),
			1,
			"Expected exactly 1 addition available after DB deletion"
		);
		assert_eq!(check2.updates_available.len(), 0);
		assert_eq!(check2.removals_available.len(), 0);

		// Record modification time of the user_config
		let mdata_before = fs::metadata(&user_config).await.unwrap();
		let mtime_before = mdata_before.modified().unwrap();

		let applied2 = apply_bundle_updates(cluster.id).await.unwrap();
		assert_eq!(applied2.additions_applied.len(), 1);

		// 6. Verify user config was NOT overwritten or wiped (no_overwrite preserved it)
		let mdata_after = fs::metadata(&user_config).await.unwrap();
		let mtime_after = mdata_after.modified().unwrap();

		assert_eq!(
			mtime_before, mtime_after,
			"User config was modified during bundle update!"
		);
		let content = fs::read_to_string(&user_config).await.unwrap();
		assert_eq!(content, "user content");

		// Clean up
		let _ = onelauncher_core::api::packages::dao::delete_package_by_id(dummy_hash).await;
		let _ = dao::delete_cluster_by_id(cluster.id).await;
		let _ = fs::remove_dir_all(cluster_dir).await;
		println!("Bundle updates E2E test passed successfully");
	}

	#[tokio::test]
	#[ignore]
	async fn test_bundle_user_overrides() {
		// 1. Initialize DB and paths
		Core::initialize(CoreOptions::default()).await.unwrap();

		// 2. Create a fresh cluster
		let cluster_name = format!(
			"Bundle Overrides Test {}",
			std::time::SystemTime::now()
				.duration_since(std::time::UNIX_EPOCH)
				.unwrap()
				.as_nanos()
		);
		let cluster = create_cluster(&cluster_name, "1.21.1", GameLoader::Fabric, None, None)
			.await
			.unwrap();

		let bundles = BundlesManager::get()
			.await
			.get_bundles_for(&cluster.mc_version, cluster.mc_loader)
			.await
			.unwrap();
		let target_bundle = bundles
			.into_iter()
			.find(|b| b.manifest.name.contains("Performance"))
			.expect("Expected to find a Performance bundle");

		// Subscribe cluster to bundle to trigger initial fetch
		use sea_orm::{ActiveModelTrait, Set};

		let dummy_hash = uuid::Uuid::new_v4().to_string();
		let dummy_base_pkg = onelauncher_core::entity::packages::ActiveModel {
			hash: Set(dummy_hash.clone()),
			file_name: Set("dummy.jar".to_string()),
			version_id: Set("dummy_version".to_string()),
			display_name: Set("Dummy".to_string()),
			display_version: Set("1.0.0".to_string()),
			package_type: Set(onelauncher_core::entity::package::PackageType::Mod),
			provider: Set(onelauncher_core::entity::package::Provider::Modrinth),
			package_id: Set("dummy_id".to_string()),
			mc_versions: Set(onelauncher_core::entity::utility::DbVec(vec![
				"1.21.1".to_string(),
			])),
			mc_loader: Set(onelauncher_core::entity::utility::DbVec(vec![
				onelauncher_core::entity::loader::GameLoader::Fabric,
			])),
			published_at: Set(chrono::Utc::now()),
			..Default::default()
		};
		dummy_base_pkg
			.insert(&onelauncher_core::store::State::get().await.unwrap().db)
			.await
			.unwrap();

		let dummy_pkg = onelauncher_core::entity::cluster_packages::ActiveModel {
			cluster_id: Set(cluster.id),
			package_hash: Set(dummy_hash.clone()),
			bundle_name: Set(Some(target_bundle.manifest.name.clone())),
			..Default::default()
		};
		dummy_pkg
			.insert(&onelauncher_core::store::State::get().await.unwrap().db)
			.await
			.unwrap();

		// 3. First apply: installs real bundle packages
		let applied1 = apply_bundle_updates(cluster.id).await.unwrap();
		assert!(
			!applied1.additions_applied.is_empty(),
			"Expected initial additions from bundle"
		);

		// Get all installed packages mapping to the bundle
		let installed_pkgs =
			onelauncher_core::api::packages::bundle_dao::get_bundle_packages_for_cluster(
				cluster.id,
			)
			.await
			.unwrap();

		// Find two packages to test the two override types (Disabled, Removed)
		let mut real_packages = installed_pkgs
			.into_iter()
			.filter(|p| p.package_hash != dummy_hash && p.package_id.is_some());

		let disabled_target = real_packages
			.next()
			.expect("Expected at least 1 real package installed");
		let removed_target = real_packages
			.next()
			.expect("Expected at least 2 real packages installed");

		// 4. Test "Removed" override behavior
		println!(
			"Testing Removed override on package: {}",
			removed_target.package_id.as_ref().unwrap()
		);

		// Unlink/remove package, which should internally trigger the Overrides logic.
		// record_override=true: this is an explicit user-initiated removal.
		onelauncher_core::api::packages::remove_package(
			cluster.id,
			removed_target.package_hash.clone(),
			true,
		)
		.await
		.unwrap();

		// Check updates. The package should NOT appear as an addition since the user explicitly removed it.
		let check_removed = check_bundle_updates(cluster.id).await.unwrap();

		let is_offered_as_addition = check_removed.additions_available.iter().any(|a| {
			let file_id = match &a.new_file.kind {
				onelauncher_core::api::packages::modpack::data::ModpackFileKind::Managed((
					pkg,
					_,
				)) => pkg.id.clone(),
				onelauncher_core::api::packages::modpack::data::ModpackFileKind::External(ext) => {
					ext.sha1.clone()
				}
			};
			&file_id == removed_target.package_id.as_ref().unwrap()
		});
		assert!(
			!is_offered_as_addition,
			"Package was removed by user but offered as an addition during bundle check!"
		);

		// 5. Test "Disabled" override behavior
		println!(
			"Testing Disabled override on package: {}",
			disabled_target.package_id.as_ref().unwrap()
		);

		// Toggle package to disabled, should trigger Overrides logic
		let is_now_enabled = onelauncher_core::api::packages::toggle_package(
			cluster.id,
			disabled_target.package_hash.clone(),
		)
		.await
		.unwrap();
		assert!(!is_now_enabled, "Package was not disabled successfully");

		// Let's fake an update for the disabled package by altering its installed version ID
		let dummy_pkg = onelauncher_core::entity::cluster_packages::ActiveModel {
			cluster_id: Set(disabled_target.cluster_id),
			package_hash: Set(disabled_target.package_hash.clone()),
			bundle_version_id: Set(Some("old_fake_version_trigger_update".to_string())),
			..disabled_target.clone().into()
		};
		// Ignore warnings about updating Primary Key fields
		dummy_pkg
			.update(&onelauncher_core::store::State::get().await.unwrap().db)
			.await
			.unwrap();

		// Apply updates. The disabled package should be updated, BUT should remain disabled.
		let _pre_update_path = onelauncher_core::api::packages::dao::get_package_by_hash(
			disabled_target.package_hash.clone(),
		)
		.await
		.unwrap()
		.unwrap()
		.path()
		.await
		.unwrap();

		let check_disabled = check_bundle_updates(cluster.id).await.unwrap();
		assert!(
			check_disabled
				.updates_available
				.iter()
				.any(|u| u.installed_package_hash == disabled_target.package_hash),
			"Disabled package should still be marked for update"
		);

		let applied2 = apply_bundle_updates(cluster.id).await.unwrap();

		let updated_pkg_hash = applied2
			.updates_applied
			.into_iter()
			.find(|u| u.installed_package_hash == disabled_target.package_hash)
			.expect("Expected the disabled package to be updated")
			.new_file;

		let file_id = match &updated_pkg_hash.kind {
			onelauncher_core::api::packages::modpack::data::ModpackFileKind::Managed((pkg, _)) => {
				pkg.id.clone()
			}
			onelauncher_core::api::packages::modpack::data::ModpackFileKind::External(ext) => {
				ext.sha1.clone()
			}
		};

		// Check the new package database entry and filesystem to ensure it's still disabled
		let new_bundle_mapping =
			onelauncher_core::api::packages::bundle_dao::get_bundle_package_by_package_id(
				cluster.id, &file_id,
			)
			.await
			.unwrap()
			.unwrap();
		let new_pkg_data = onelauncher_core::api::packages::dao::get_package_by_hash(
			new_bundle_mapping.package_hash,
		)
		.await
		.unwrap()
		.unwrap();

		assert!(
			new_pkg_data.file_name.ends_with(".disabled"),
			"Updated bundle package was re-enabled! It should have kept the .disabled extension."
		);

		// Clean up
		let _ = dao::delete_cluster_by_id(cluster.id).await;
		let _ = fs::remove_dir_all(cluster.path().await.unwrap()).await;
	}
}
