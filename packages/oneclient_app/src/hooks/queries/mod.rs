use crate::hooks::use_settings_snapshot;

/// Not the url itself just enough to notice when the meta host setting
/// changes so a different host refetches instead of serving the old answer
fn use_meta_url_key() -> String {
    use_settings_snapshot()
        .settings
        .custom_meta_url_base
        .unwrap_or_default()
        .trim()
        .to_string()
}

mod analytics;
mod auth;
mod bundles;
mod changelog;
mod cluster_content;
mod clusters;
mod image;
mod java;
mod logs;
mod migration;
mod mutations;
mod package_updates;
mod packages;
mod player_profile;
mod screenshots;
mod settings_profiles;
mod skin;
mod state;
mod storage;
mod tos;
mod version_metadata;
mod versions;

pub use analytics::{
    try_cluster_analytics, try_global_analytics, use_cluster_analytics, use_global_analytics,
};
pub use auth::{
    AddOfflineAccountKeys, BeginMicrosoftLoginMutation, CancelMicrosoftLoginKeys,
    CancelMicrosoftLoginMutation, FinishMicrosoftLoginMutation, RefreshAccountKeys,
    RemoveAccountKeys, SetDefaultAccountKeys, UseRefreshAccount, UseRemoveAccount,
    UseSetDefaultAccount, accounts_have_microsoft, login_code_already_handled, mutation_error,
    mutation_is_pending, mutation_is_running, reset_login_code_dedup, try_account, try_accounts, try_default_account,
    use_account, use_accounts, use_add_microsoft_account, use_add_offline_account,
    use_begin_microsoft_login, use_cancel_microsoft_login, use_current_account,
    use_default_account, use_finish_microsoft_login, use_refresh_account, use_refresh_all_accounts,
    use_remove_account, use_set_default_account,
};
pub use bundles::{
    ClusterBundles, OnboardingBundlesQuery, bundle_overrides_map, bundles_with_status_items,
    onboarding_bundles_items, use_bundle_overrides, use_bundle_updates, use_bundles_with_status,
    use_onboarding_bundles,
};
pub use changelog::{
    changelog_error, changelog_groups, changelog_is_loading, latest_changelog_version, use_changelog,
};
pub use cluster_content::{cluster_content_items, use_cluster_content};
pub use clusters::{use_cluster, use_clusters};
pub use image::{CachedImageQuery, loaded_image, use_cached_image};
pub use java::{
    invalidate_java_queries, java_runtimes, provider_versions, use_java_runtimes,
    use_provider_versions,
};
pub use logs::{
    ClusterLogsQuery, LogAction, LogContentQuery, UploadLogKeys, UploadLogMutation, UseLogAction,
    UseUploadLog, invalidate_logs_queries, try_cluster_logs, try_log_content, use_cluster_logs,
    use_log_action, use_log_content, use_upload_log,
};
pub use migration::{MigrationQuery, has_migration_data, migration_detection, use_migration};
pub use storage::{
    StorageAction, StorageActionMutation, StorageReportQuery, UseStorageAction,
    invalidate_storage_queries, try_storage_report, use_storage_action, use_storage_report,
};
pub use mutations::{
    ClusterAction, invalidate_cluster_content_queries, invalidate_cluster_queries,
    invalidate_profile_queries, use_cluster_mutation,
};
pub use package_updates::{package_updates, stale_hashes, use_package_updates};
pub use packages::{
    BROWSE_PAGE_SIZE, VERSIONS_PAGE_SIZE, category_list, content_type_for_slug, package_meta_batch,
    project_detail, search_items, search_pending, search_total, use_package_categories,
    use_package_meta_batch, use_package_project, use_package_search, use_package_versions,
    use_package_versions_when,
    version_list, versions_total,
};
pub use player_profile::use_player_profile;
pub use screenshots::{
    ScreenshotAction, UseScreenshotAction, invalidate_screenshots_queries, try_cluster_screenshots,
    use_cluster_screenshots, use_local_image, use_screenshot_action,
};
pub use settings_profiles::{
    try_game_profile, use_cluster_profile, use_cluster_settings, use_game_profile,
    use_named_profiles,
};
pub use skin::use_player_skin;
pub use state::{query_error, query_is_busy, query_is_loading, settled_or_loading};
pub use tos::{TermsQuery, terms_document, terms_error, terms_is_loading, use_terms};
pub use version_metadata::{pick_version_metadata, use_version_metadata};
pub use versions::{loader_versions, use_loader_versions, use_versions, versions_metadata};
