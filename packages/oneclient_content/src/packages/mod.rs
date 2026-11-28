pub mod activity;
pub mod bad_modpacks;
pub mod bad_mods;
pub mod dependencies;
pub mod dependents;
pub mod error;
pub mod fabric_version;
pub mod local_manifest;
pub mod metadata_cache;
pub mod provider;
pub mod release_migration;
pub mod store;
pub mod types;
pub mod updates;

mod file_identity;
pub mod markdown;

// Re-exported so `oneclient_core::packages::ContentType` keeps working
pub use activity::reconcile_duplicate_activity;
pub use bad_modpacks::{
    fetch_bad_modpacks, fetch_modpack_explanation, load_bad_modpacks, refresh_bad_modpacks,
};
pub use bad_mods::{
    BadMod, BadModAlternative, BadModList, ProjectIds, ResolvedAlternative, fetch_bad_mods,
    fetch_explanation, fetch_improvements, load_bad_mods, refresh_bad_mods, resolve_alternatives,
};
pub use dependencies::{
    DependencyResolution, ModInstallTarget, ResolvedDependency, artifact_mod_id,
    find_mod_for_cluster, pick_version, resolve_required, resolves_dependencies,
};
pub use error::{PackageError, PackageResult};
pub use file_identity::{FileIdentity, curseforge_fingerprint};
pub use local_manifest::{
    JarDependencies, JarManifest, read_jar_bundled_mod_ids, read_jar_dependencies, read_jar_icon,
    read_jar_loader, read_jar_manifest,
};
pub use metadata_cache::{
    CachedPackageMeta, cached_project_detail, fetch_package_meta, get_version_cached,
    read_cached_package_meta,
};
pub use dependents::{
    DependentPackage, DisableImpact, DisableRoot, disable_impact, ensure_cluster_dependencies,
};
pub use oneclient_common::domain::{ContentType, GameLoader, HashAlgorithm, ProviderId};
pub use provider::{PackageProvider, PackageProviderRegistry};
pub use store::{LiveSync, LocalImportReport, PackageStore};
pub use types::*;
pub use updates::{
    BrowserPackageUpdate, BrowserUpdateCheck, apply_browser_package_update,
    cached_browser_package_updates, check_browser_package_updates, refresh_browser_package_updates,
    refresh_browser_package_updates_for, skip_browser_package_update,
};


// Dependency resolution and content writes from any launcher entry point can
// overlap. Undo needs both to settle before it checks and removes links.
static PACKAGE_MUTATIONS: tokio::sync::RwLock<()> = tokio::sync::RwLock::const_new(());

pub async fn package_mutation_guard() -> tokio::sync::RwLockReadGuard<'static, ()> {
    PACKAGE_MUTATIONS.read().await
}

static INSTANCES: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<i64, std::sync::Weak<tokio::sync::Mutex<()>>>>,
> = std::sync::LazyLock::new(Default::default);

/// Serializes whatever resolves packages against an instance and then links or removes
/// them, so nothing is planned around a package that is gone by the time it is applied.
/// Always taken before a package mutation guard, never while holding one.
pub async fn instance_package_guard(cluster_id: i64) -> tokio::sync::OwnedMutexGuard<()> {
    let instance = {
        let mut locks = INSTANCES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(&cluster_id).and_then(std::sync::Weak::upgrade) {
            lock
        } else {
            let lock = std::sync::Arc::new(tokio::sync::Mutex::new(()));
            locks.insert(cluster_id, std::sync::Arc::downgrade(&lock));
            lock
        }
    };
    instance.lock_owned().await
}

pub async fn exclusive_package_mutation_guard() -> tokio::sync::RwLockWriteGuard<'static, ()> {
    PACKAGE_MUTATIONS.write().await
}
