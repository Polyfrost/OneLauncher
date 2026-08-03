pub mod activity;
pub mod dependencies;
pub mod error;
pub mod installed;
pub mod metadata_cache;
pub mod modpack;
pub mod provider;
pub mod store;
pub mod types;
pub mod updates;

mod file_identity;

// Re-exported so `oneclient_core::packages::ContentType` keeps working; the
// types themselves now live in the leaf vocabulary crate.
pub use oneclient_common::domain::{ContentType, GameLoader, HashAlgorithm, ProviderId};
pub use metadata_cache::{
    CachedPackageMeta, cached_project_detail, fetch_package_meta, get_version_cached,
    read_cached_package_meta,
};
pub use activity::reconcile_duplicate_activity;
pub use dependencies::{
    DependencyResolution, ResolvedDependency, resolve_required, resolves_dependencies,
};
pub use file_identity::{curseforge_fingerprint, FileIdentity};
pub use installed::{InstalledCopy, installed_copies, installed_local_copy, local_copy_among};
pub use error::{PackageError, PackageResult};
pub use provider::{PackageProvider, PackageProviderRegistry};
pub use store::PackageStore;
pub use types::*;
pub use updates::{
    BrowserPackageUpdate, BrowserUpdateCheck, apply_browser_package_update,
    cached_browser_package_updates, check_browser_package_updates,
    refresh_browser_package_updates, skip_browser_package_update,
};
