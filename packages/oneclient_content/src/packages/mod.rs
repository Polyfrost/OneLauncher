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
    DependencyResolution, ModInstallTarget, ResolvedDependency, find_mod_for_cluster,
    pick_version, resolve_required, resolves_dependencies,
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
    skip_browser_package_update,
};
