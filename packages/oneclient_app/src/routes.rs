use freya::prelude::*;
use freya::router::*;

use crate::i18n::t;
use crate::layout::AppShell;
use crate::layout::ClusterShell;
use crate::layout::OnboardingShell;
use crate::layout::RootLayout;
use crate::layout::SettingsShell;

use crate::view::{
    NotFound, Relocating, Startup,
    app::{
        Clusters, Debug, Home, Stats, Wardrobe,
        browser::{Browser, BrowserPackage},
        cluster::{
            ClusterDataPacks, ClusterLogs, ClusterMods, ClusterOverview, ClusterScreenshots,
            ClusterSettings, ClusterShaders, ClusterTextures, ClusterWorlds, ProcessLogs,
        },
        settings::{
            SettingsAccounts, SettingsApis, SettingsAppearance, SettingsChangelog,
            SettingsDeveloper, SettingsJava, SettingsLanguage, SettingsLauncher, SettingsMinecraft,
            SettingsStorage,
        },
    },
    onboarding::{
        OnboardingAccount, OnboardingBundles, OnboardingLanguage, OnboardingLocation,
        OnboardingMigration, OnboardingPreferences, OnboardingSummary, OnboardingTerms,
        OnboardingWelcome,
    },
};

#[derive(Routable, Debug, Clone, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(RootLayout)]
        #[route("/")]
        Startup {},
        #[route("/moving")]
        Relocating {},

        #[layout(OnboardingShell)]
            #[route("/onboarding")]
            OnboardingWelcome {},
            #[route("/onboarding/location")]
            OnboardingLocation {},
            #[route("/onboarding/terms")]
            OnboardingTerms {},
            #[route("/onboarding/migration")]
            OnboardingMigration {},
            #[route("/onboarding/language")]
            OnboardingLanguage {},
            #[route("/onboarding/account")]
            OnboardingAccount {},
            #[route("/onboarding/preferences")]
            OnboardingPreferences {},
            #[route("/onboarding/bundles")]
            OnboardingBundles {},
            #[route("/onboarding/summary")]
            OnboardingSummary {},
        #[end_layout]
        
        #[layout(AppShell)]
            #[route("/app")]
            Home {},
            #[route("/app/clusters")]
            Clusters {},

            #[layout(ClusterShell)]
                #[route("/app/clusters/:cluster_id")]
                ClusterOverview { cluster_id: i64 },
                #[route("/app/clusters/:cluster_id/logs")]
                ClusterLogs { cluster_id: i64 },
                #[route("/app/clusters/:cluster_id/process")]
                ProcessLogs { cluster_id: i64 },
                #[route("/app/clusters/:cluster_id/screenshots")]
                ClusterScreenshots { cluster_id: i64 },
                #[route("/app/clusters/:cluster_id/mods")]
                ClusterMods { cluster_id: i64 },
                #[route("/app/clusters/:cluster_id/shaders")]
                ClusterShaders { cluster_id: i64 },
                #[route("/app/clusters/:cluster_id/textures")]
                ClusterTextures { cluster_id: i64 },
                #[route("/app/clusters/:cluster_id/worlds")]
                ClusterWorlds { cluster_id: i64 },
                #[route("/app/clusters/:cluster_id/datapacks")]
                ClusterDataPacks { cluster_id: i64 },
                #[route("/app/clusters/:cluster_id/settings")]
                ClusterSettings { cluster_id: i64 },
            #[end_layout]

            #[route("/app/browser/:cluster_id/:package_type?:pick_cluster")]
            Browser { cluster_id: i64, package_type: String, pick_cluster: bool },
            #[route("/app/browser/:cluster_id/:package_type/package/:package_id?:add_to_cluster")]
            BrowserPackage {
                cluster_id: i64,
                package_type: String,
                package_id: String,
                add_to_cluster: bool,
            },
            #[route("/app/wardrobe")]
            Wardrobe {},
            #[route("/app/stats")]
            Stats {},
            #[route("/app/debug")]
            Debug {},

            #[layout(SettingsShell)]
                #[route("/app/settings/appearance")]
                SettingsAppearance {},
                #[route("/app/settings/minecraft")]
                SettingsMinecraft {},
                #[route("/app/settings/accounts")]
                SettingsAccounts {},
                #[route("/app/settings/launcher")]
                SettingsLauncher {},
                #[route("/app/settings/java")]
                SettingsJava {},
                #[route("/app/settings/storage")]
                SettingsStorage {},
                #[route("/app/settings/apis")]
                SettingsApis {},
                #[route("/app/settings/language")]
                SettingsLanguage {},
                #[route("/app/settings/developer")]
                SettingsDeveloper {},
                #[route("/app/settings/changelog")]
                SettingsChangelog {},
                // #[route("/app/settings/feedback")]
                // SettingsFeedback {},
            #[end_layout]
        #[end_layout]

        #[route("/:..path")]
        NotFound { path: Vec<String> },
}

impl Route {
    pub fn title(&self) -> String {
        let title = match self {
            Route::Startup { .. } => t("oneclient.routes.startup"),
            Route::Relocating { .. } => t("oneclient.routes.relocating"),
            Route::OnboardingWelcome { .. } => t("oneclient.routes.onboarding_welcome"),
            Route::OnboardingLocation { .. } => t("oneclient.routes.onboarding_location"),
            Route::OnboardingTerms { .. } => t("oneclient.routes.onboarding_terms"),
            Route::OnboardingMigration { .. } => t("oneclient.routes.onboarding_migration"),
            Route::OnboardingLanguage { .. } => t("oneclient.routes.onboarding_language"),
            Route::OnboardingAccount { .. } => t("oneclient.routes.onboarding_account"),
            Route::OnboardingPreferences { .. } => t("oneclient.routes.onboarding_preferences"),
            Route::OnboardingBundles { .. } => t("oneclient.routes.onboarding_bundles"),
            Route::OnboardingSummary { .. } => t("oneclient.routes.onboarding_summary"),
            Route::Home { .. } => t("oneclient.routes.home"),
            Route::Clusters { .. } => t("oneclient.routes.clusters"),
            Route::ClusterOverview { .. } => t("oneclient.routes.cluster_overview"),
            Route::ClusterLogs { .. } => t("oneclient.routes.cluster_logs"),
            Route::ProcessLogs { .. } => t("oneclient.routes.process_logs"),
            Route::ClusterScreenshots { .. } => t("oneclient.routes.cluster_screenshots"),
            Route::ClusterMods { .. } => t("oneclient.routes.cluster_mods"),
            Route::ClusterShaders { .. } => t("oneclient.routes.cluster_shaders"),
            Route::ClusterTextures { .. } => t("oneclient.routes.cluster_textures"),
            Route::ClusterWorlds { .. } => t("oneclient.routes.cluster_worlds"),
            Route::ClusterDataPacks { .. } => t("oneclient.routes.cluster_data_packs"),
            Route::ClusterSettings { .. } => t("oneclient.routes.cluster_settings"),
            Route::Browser { .. } => t("oneclient.routes.browser"),
            Route::BrowserPackage { .. } => t("oneclient.routes.browser_package"),
            Route::Wardrobe { .. } => t("oneclient.routes.wardrobe"),
            Route::Stats { .. } => t("oneclient.routes.stats"),
            Route::Debug { .. } => t("oneclient.routes.debug"),
            Route::SettingsAppearance { .. } => t("oneclient.routes.settings_appearance"),
            Route::SettingsMinecraft { .. } => t("oneclient.routes.settings_minecraft"),
            Route::SettingsAccounts { .. } => t("oneclient.routes.settings_accounts"),
            Route::SettingsLauncher { .. } => t("oneclient.routes.settings_launcher"),
            Route::SettingsJava { .. } => t("oneclient.routes.settings_java"),
            Route::SettingsStorage { .. } => t("oneclient.routes.settings_storage"),
            Route::SettingsApis { .. } => t("oneclient.routes.settings_apis"),
            Route::SettingsLanguage { .. } => t("oneclient.routes.settings_language"),
            Route::SettingsDeveloper { .. } => t("oneclient.routes.settings_developer"),
            Route::SettingsChangelog { .. } => t("oneclient.routes.settings_changelog"),
            // Route::SettingsFeedback { .. } => "Feedback".to_string(),
            Route::NotFound { .. } => t("oneclient.routes.not_found"),
        };
        title.to_string()
    }
}

pub fn router() -> impl IntoElement {
    Router::<Route>::new(|| RouterConfig::default().with_initial_path(Route::Startup {}))
}
