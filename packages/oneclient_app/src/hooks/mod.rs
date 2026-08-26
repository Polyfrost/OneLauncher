mod active_cluster;
mod debounce;
mod actions;
mod queries;
mod view_state;

pub use debounce::use_debounced;
pub use view_state::{PersistedView, use_view_state};

pub use active_cluster::{
    ActiveClusterState, BrowserCompatState, BrowserStateStore, BrowserUiState, LinkConfirmState,
    OnboardingSelectionState, SplashState, use_active_cluster_id, use_browser_compat,
    use_browser_state_store, use_link_confirm, use_onboarding_selection, use_provide_active_cluster,
    use_provide_browser_compat, use_provide_browser_state, use_provide_link_confirm,
    use_provide_onboarding_selection, use_provide_splash, use_splash,
};

pub use actions::{Actions, NotificationBuilder, PumpSignal};
pub use queries::{
    AddOfflineAccountKeys, BROWSE_PAGE_SIZE, BeginMicrosoftLoginMutation, CachedImageQuery,
    CancelMicrosoftLoginKeys, CancelMicrosoftLoginMutation, ClusterAction, ClusterBundles,
    ClusterLogsQuery, FinishMicrosoftLoginMutation, LogAction, LogContentQuery, MigrationQuery,
    OnboardingBundlesQuery, RefreshAccountKeys, RemoveAccountKeys, ScreenshotAction,
    SetDefaultAccountKeys, StorageAction, StorageActionMutation, StorageReportQuery, TermsQuery,
    UploadLogKeys, UploadLogMutation, UseLogAction,
    UseRefreshAccount, UseRemoveAccount, UseScreenshotAction, UseSetDefaultAccount,
    UseStorageAction, UseUploadLog,
    VERSIONS_PAGE_SIZE, accounts_have_microsoft, bundle_overrides_map, bundles_with_status_items,
    category_list, changelog_error, changelog_groups, changelog_is_loading, cluster_content_items,
    content_type_for_slug, has_migration_data, invalidate_cluster_content_queries,
    invalidate_cluster_queries, invalidate_java_queries,
    invalidate_logs_queries, invalidate_profile_queries, invalidate_screenshots_queries,
    invalidate_storage_queries, try_storage_report, use_storage_action, use_storage_report,
    java_runtimes, latest_changelog_version, loaded_image, loader_versions,
    login_code_already_handled, migration_detection,
    mutation_error, mutation_is_pending, mutation_is_running, onboarding_bundles_items, package_meta_batch,
    package_updates, pick_version_metadata, project_detail, provider_versions, query_error,
    query_is_busy, stale_hashes, use_package_updates,
    query_is_loading, reset_login_code_dedup, search_items, search_pending, search_total,
    settled_or_loading, terms_document, terms_error, terms_is_loading, try_account,
    try_accounts, try_cluster_analytics, try_cluster_logs, try_cluster_screenshots,
    try_default_account, try_game_profile, try_global_analytics, try_log_content, use_account,
    use_accounts, use_add_microsoft_account, use_add_offline_account, use_begin_microsoft_login,
    use_bundle_overrides, use_bundle_updates, use_bundles_with_status, use_cached_image,
    use_cancel_microsoft_login, use_changelog, use_cluster_analytics, use_cluster_content,
    use_cluster_logs, use_cluster_mutation, use_cluster_profile, use_cluster_screenshots,
    use_cluster, use_cluster_settings, use_clusters, use_current_account, use_default_account,
    use_finish_microsoft_login, use_game_profile, use_global_analytics, use_java_runtimes,
    use_loader_versions, use_local_image, use_log_action, use_log_content, use_migration,
    use_named_profiles, use_onboarding_bundles, use_package_categories, use_package_meta_batch,
    use_package_project, use_package_search, use_package_versions, use_package_versions_when,
    use_player_profile,
    use_player_skin, use_provider_versions, use_refresh_account, use_refresh_all_accounts,
    use_remove_account, use_screenshot_action, use_screenshot_folder_watch, use_set_default_account,
    use_terms, use_upload_log,
    use_version_metadata, use_versions, version_list, versions_metadata, versions_total,
};

use crate::notifications::NotificationSnapshot;
use crate::state::{
    AppChannel, GameState, InstallState, LauncherInit, LoginProgress, SettingsState,
};
use freya::prelude::*;
use freya::radio::use_radio;

pub fn use_provide_actions(actions: &Actions) {
    let actions = actions.clone();
    use_provide_root_context(move || actions.clone());
}

pub fn use_dispatch() -> Actions {
    consume_root_context::<Actions>()
}

/// Wakes its component only when that channel is written so a toast tick does
/// not re-render a component reading only `data_dir`
pub fn use_launcher() -> LauncherInit {
    use_radio(AppChannel::Launcher).read().launcher.clone()
}

pub fn use_settings_snapshot() -> SettingsState {
    use_radio(AppChannel::Settings).read().settings.clone()
}

/// Built on read not published on write cloning the inbox on every one of a
/// download's tens of thousands of engine changes was the channel's main cost
pub fn use_notifications_snapshot() -> NotificationSnapshot {
    let radio = use_radio(AppChannel::Notifications);
    let state = radio.read();
    state.notifications.snapshot(
        &state.inbox,
        state.center_open,
        crate::events::prompt_view(&state),
    )
}

pub fn use_account_switcher_open() -> bool {
    use_radio(AppChannel::AccountSwitcher)
        .read()
        .account_switcher_open
}

pub fn use_game_snapshot() -> GameState {
    use_radio(AppChannel::Game).read().game.clone()
}

pub fn use_installs_snapshot() -> InstallState {
    use_radio(AppChannel::Installs).read().installs.clone()
}

pub fn use_pending_launch() -> Option<String> {
    use_radio(AppChannel::PendingLaunch)
        .read()
        .pending_launch
        .clone()
}

pub fn use_microsoft_login_status() -> Option<LoginProgress> {
    use_radio(AppChannel::MicrosoftLogin)
        .read()
        .microsoft_login
        .clone()
}

