#![recursion_limit = "256"]

#[cfg(debug_assertions)]
pub mod dev;

pub mod api_config;
pub mod auth;
pub mod bundles;
pub mod changelog;
pub mod clusters;
pub mod constants;
pub mod crypto;
pub mod discord;
mod error;
pub mod game;
pub mod http;
pub mod images;
pub mod java;
pub mod logger;
pub mod logs;
pub mod metadata;
pub mod migration;
pub mod minecraft;
pub mod notification;
pub mod os_ext;
pub mod packages;
pub mod patch;
pub mod paths;
pub mod plus;
pub mod recovery;
pub mod reporting;
pub mod screenshots;
pub mod settings;
mod state;
pub mod status;
pub mod tos;
pub mod version;
pub mod versions;

pub use bundles::{
    ApplyBundleUpdatesResult, Bundle, BundleArchive, BundleError, BundleFile, BundleFileKind,
    BundleManifest, BundleUpdateCheckResult, BundleWithUpdateStatus, BundlesManager,
    FileUpdateStatus, apply_bundle_updates, check_bundle_updates, effective_enabled,
    get_bundles_with_update_status, install_bundle, install_cluster_bundles,
    install_package_from_bundle, is_bundle_syncing, list_cluster_bundle_overrides,
    set_bundle_package_enabled, set_bundle_package_opt_in, set_bundle_package_override,
    set_bundle_package_overrides,
};
pub use changelog::{ChangelogGroup, fetch_changelog, parse_changelog};
pub use clusters::{
    Cluster, ClusterError, ClusterManager, ClusterStage, ClusterUpdate, CreateClusterOptions,
    ensure_from_bundles, ensure_from_versions, estimate_cluster_download,
};
pub use discord::{DiscordRpc, Presence};
pub use error::{LauncherError, LauncherResult};
pub use game::{GameError, LaunchedGame, get_loader_versions, launch_cluster};
pub use images::ImageCacheStore;
pub use logs::{
    LogFileInfo, LogKind, LogLevel, LogLine, LogsError, MclogsUploadResponse, ReadOptions,
    delete_log_at, list_cluster_logs, read_log_at, upload_log_at,
};
pub use metadata::{MetadataError, MetadataStore};
pub use migration::{
    ImportTarget, MigrationDetection, MigrationSource, SourceInstance,
    detect_all as detect_migrations, import_game_dir as import_migration_game_dir,
    import_settings as import_migration_settings,
};
pub use notification::{GroupedProgressChild, GroupedProgressEvent, GroupedProgressSession};
pub use packages::LinkedArtifactInfo;
pub use patch::Patch;
pub use screenshots::{
    ScreenshotInfo, ScreenshotsError, delete_screenshot, list_cluster_screenshots, load_screenshot,
};
pub use settings::ProfileUpdate;
pub use state::LauncherServices;
pub use state::LauncherState;
pub use tos::{TermsDocument, fetch_terms};
pub use version::{ParsedMcVersion, VersionKey, format_mc_version, parse_mc_version};
pub use versions::{RemoteMigration, VersionMetadata, VersionsManager, VersionsManifest};
