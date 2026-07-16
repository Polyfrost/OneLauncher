mod active_cluster;
mod dispatch;
mod queries;
mod view_state;

pub use view_state::{PersistedView, use_view_state};

pub use active_cluster::{
    ActiveClusterState, BrowserCompatState, BrowserStateStore, BrowserUiState, ImportSelection,
    LinkConfirmState, OnboardingSelectionState, use_active_cluster_id, use_browser_compat,
    use_browser_state_store, use_link_confirm, use_onboarding_selection,
    use_provide_active_cluster, use_provide_browser_compat, use_provide_browser_state,
    use_provide_link_confirm, use_provide_onboarding_selection,
};

pub use dispatch::BridgeDispatch;
pub use queries::{
    AddOfflineAccountKeys, BROWSE_PAGE_SIZE, BeginMicrosoftLoginMutation, CachedImageQuery,
    CancelMicrosoftLoginKeys, CancelMicrosoftLoginMutation, ClusterAction, ClusterBundles,
    ClusterLogsQuery, FinishMicrosoftLoginMutation, LogAction, LogContentQuery, MigrationQuery,
    OnboardingBundlesQuery, RefreshAccountKeys, RemoveAccountKeys, ScreenshotAction,
    SetDefaultAccountKeys, TermsQuery, UploadLogKeys, UploadLogMutation, UseLogAction,
    UseRefreshAccount, UseRemoveAccount, UseScreenshotAction, UseSetDefaultAccount, UseUploadLog,
    VERSIONS_PAGE_SIZE, accounts_have_microsoft, bundle_overrides_map, bundles_with_status_items,
    category_list, changelog_error, changelog_groups, changelog_is_loading, cluster_content_items,
    content_type_for_slug, has_migration_data, invalidate_cluster_queries, invalidate_java_queries,
    invalidate_logs_queries, invalidate_profile_queries, invalidate_screenshots_queries,
    java_runtimes, loader_versions, login_code_already_handled, migration_detections,
    mutation_error, mutation_is_pending, onboarding_bundles_items, package_meta_batch,
    pick_version_metadata, project_detail, provider_versions, reset_login_code_dedup, search_items,
    search_pending, search_total, terms_document, terms_error, terms_is_loading, try_account,
    try_accounts, try_cluster_analytics, try_cluster_logs, try_cluster_screenshots,
    try_default_account, try_game_profile, try_global_analytics, try_log_content, use_account,
    use_accounts, use_add_microsoft_account, use_add_offline_account, use_begin_microsoft_login,
    use_bundle_overrides, use_bundle_updates, use_bundles_with_status, use_cached_image,
    use_cancel_microsoft_login, use_changelog, use_cluster_analytics, use_cluster_content,
    use_cluster_logs, use_cluster_mutation, use_cluster_profile, use_cluster_screenshots,
    use_cluster_settings, use_clusters, use_current_account, use_default_account,
    use_finish_microsoft_login, use_game_profile, use_global_analytics, use_java_runtimes,
    use_loader_versions, use_local_image, use_log_action, use_log_content, use_migration,
    use_named_profiles, use_onboarding_bundles, use_package_categories, use_package_meta_batch,
    use_package_project, use_package_search, use_package_versions, use_player_profile,
    use_player_skin, use_provider_versions, use_refresh_account, use_refresh_all_accounts,
    use_remove_account, use_screenshot_action, use_set_default_account, use_terms, use_upload_log,
    use_version_metadata, use_versions, version_list, versions_metadata, versions_total,
};

use crate::{
    bridge::{
        BridgeSnapshot, ClustersSnapshot, GameSnapshot, JavaSnapshot, LauncherInit,
        OneClientBridge, ProfilesSnapshot, SettingsSnapshot, use_bridge_snapshot,
    },
    notifications::NotificationSnapshot,
};
use freya::prelude::*;

pub fn use_provide_bridge(bridge: &OneClientBridge) {
    let bridge = bridge.clone();
    use_provide_root_context(move || bridge.clone());
}

pub fn use_bridge() -> OneClientBridge {
    consume_root_context::<OneClientBridge>()
}

pub fn use_snapshots() -> BridgeSnapshot {
    let bridge = use_bridge();
    use_bridge_snapshot(&bridge)
}

pub fn use_dispatch() -> BridgeDispatch {
    let bridge = use_bridge();
    use_hook(move || BridgeDispatch::new(bridge.clone()))
}

pub fn use_launcher() -> LauncherInit {
    use_snapshots().launcher
}

pub fn use_settings_snapshot() -> SettingsSnapshot {
    use_snapshots().settings
}

pub fn use_profiles_snapshot() -> ProfilesSnapshot {
    use_snapshots().profiles
}

pub fn use_clusters_snapshot() -> ClustersSnapshot {
    use_snapshots().clusters
}

pub fn use_notifications_snapshot() -> NotificationSnapshot {
    use_snapshots().notifications
}

pub fn use_account_switcher_open() -> bool {
    use_snapshots().account_switcher_open
}

pub fn use_game_snapshot() -> GameSnapshot {
    use_snapshots().game
}

pub fn use_microsoft_login_status() -> Option<oneclient_core::notification::MicrosoftLoginStatus> {
    use_snapshots().microsoft_login
}

pub fn use_java_snapshot() -> JavaSnapshot {
    use_snapshots().java
}

#[derive(PartialEq)]
pub struct DataSync;

impl Component for DataSync {
    fn render(&self) -> impl IntoElement {
        let clusters_pulse = use_clusters_snapshot().generation;
        let profiles_pulse = use_profiles_snapshot().generation;
        let java_pulse = use_java_snapshot().generation;
        let mut last_clusters = use_state(|| clusters_pulse);
        let mut last_profiles = use_state(|| profiles_pulse);
        let mut last_java = use_state(|| java_pulse);

        if *last_clusters.peek() != clusters_pulse {
            last_clusters.set(clusters_pulse);
            spawn(async move { invalidate_cluster_queries().await });
        }

        if *last_profiles.peek() != profiles_pulse {
            last_profiles.set(profiles_pulse);
            spawn(async move { invalidate_profile_queries().await });
        }

        if *last_java.peek() != java_pulse {
            last_java.set(java_pulse);
            spawn(async move { invalidate_java_queries().await });
        }

        rect().width(Size::px(0.)).height(Size::px(0.))
    }
}
