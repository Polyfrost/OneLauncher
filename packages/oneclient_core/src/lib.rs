#![recursion_limit = "256"]

#[cfg(debug_assertions)]
pub mod dev;

pub mod changelog;
pub mod clusters;
mod error;
pub mod game;
pub mod images;
mod java_store;
pub mod logger;
pub mod migration;
pub mod recovery;
pub mod reporting;
pub mod settings;
pub mod storage;
pub mod tos;
mod state;
pub mod simulate;
pub mod verify;
pub mod versions;

pub use oneclient_content::bundles::{
    apply_bundle_updates, check_bundle_updates, effective_enabled, install_bundle,
    install_cluster_bundles,
    install_package_from_bundle, is_bundle_syncing, list_cluster_bundle_overrides,
    pending_optional_mods, resolve_optional_mods, skip_optional_mods, PendingOptionalMod,
    set_bundle_package_enabled, set_bundle_package_opt_in, set_bundle_package_override,
    set_bundle_package_overrides,
    ApplyBundleUpdatesResult, Bundle, BundleArchive, BundleError, BundleFile,
    BundleFileKind, BundleManifest, BundleOptionalPackage, BundlesManager, BundleUpdateCheckResult,
    BundleWithUpdateStatus, FileUpdateStatus, get_bundles_with_update_status,
    remove_artifact_from_cluster, toggle_artifact_enabled,
};
pub use changelog::{fetch_changelog, parse_changelog, ChangelogGroup};
pub use tos::{fetch_terms, TermsDocument};
pub use oneclient_discord::{DiscordRpc, Presence};
pub use clusters::{
    Cluster, ClusterError, ClusterManager, ClusterStage, ClusterUpdate, CreateClusterOptions,
    ensure_from_bundles, ensure_from_versions, estimate_cluster_download,
};
pub use error::{LauncherError, LauncherResult, SentryExclusion};
pub use game::{GameError, LaunchedGame, get_loader_versions, launch_cluster};
pub use images::ImageCacheStore;
pub use oneclient_cluster::logs::{
    LogFileInfo, LogKind, LogLevel, LogLine, LogsError, MclogsUploadResponse, ReadOptions,
    delete_log_at, list_cluster_logs, read_log_at, upload_log_at,
};
pub use oneclient_mc::{McError as MetadataError, MetadataStore};
pub use migration::{
    detect as detect_migration, import_game_dir as import_migration_game_dir, ImportTarget,
    MigrationDetection, MigrationSource, SourceInstance,
};
pub use oneclient_cluster::screenshots::{
    ScreenshotInfo, ScreenshotsError, delete_screenshot, list_cluster_screenshots, load_screenshot,
};
pub use oneclient_content::packages::LinkedArtifactInfo;
pub use oneclient_content::packages::updates::{
    BrowserPackageUpdate, BrowserUpdateCheck, apply_browser_package_update,
    cached_browser_package_updates, check_browser_package_updates,
    refresh_browser_package_updates, skip_browser_package_update,
};
pub use settings::ProfileUpdate;
pub use verify::{ClusterVerifyReport, verify_cluster_files};
pub use state::LauncherServices;
pub use state::LauncherState;
pub use state::run_startup_tasks;
pub use versions::{
    RemoteMigration, VersionMetadata, VersionsManager, VersionsManifest, resolve_migration_chain,
};
