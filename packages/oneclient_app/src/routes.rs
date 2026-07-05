use freya::prelude::*;
use freya::router::*;

use crate::layout::AppShell;
use crate::layout::ClusterShell;
use crate::layout::OnboardingShell;
use crate::layout::RootLayout;
use crate::layout::SettingsShell;

use crate::view::{
    NotFound, Startup,
    app::{
        AccountSkins, Accounts, Clusters, Debug, Home, Stats,
        browser::{Browser, BrowserPackage},
        cluster::{
            ClusterLogs, ClusterMods, ClusterOverview, ClusterScreenshots, ClusterSettings,
            ClusterShaders, ClusterTextures, ProcessLogs,
        },
        settings::{
            SettingsApis, SettingsAppearance, SettingsChangelog, SettingsDeveloper,
            SettingsJava, SettingsLanguage, SettingsLauncher, SettingsMinecraft,
        },
    },
    onboarding::{
        OnboardingAccount, OnboardingBundles, OnboardingDownloading, OnboardingLanguage,
        OnboardingPreferences, OnboardingWelcome,
    },
};

#[derive(Routable, Debug, Clone, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(RootLayout)]
        #[route("/")]
        Startup {},

        #[layout(OnboardingShell)]
            #[route("/onboarding")]
            OnboardingWelcome {},
            #[route("/onboarding/language")]
            OnboardingLanguage {},
            #[route("/onboarding/account")]
            OnboardingAccount {},
            #[route("/onboarding/preferences")]
            OnboardingPreferences {},
            #[route("/onboarding/bundles")]
            OnboardingBundles {},
            #[route("/onboarding/downloading")]
            OnboardingDownloading {},
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
                #[route("/app/clusters/:cluster_id/settings")]
                ClusterSettings { cluster_id: i64 },
            #[end_layout]

            #[route("/app/browser/:cluster_id/:package_type")]
            Browser { cluster_id: i64, package_type: String },
            #[route("/app/browser/:cluster_id/:package_type/package/:package_id")]
            BrowserPackage { cluster_id: i64, package_type: String, package_id: String },
            #[route("/app/accounts")]
            Accounts {},
            #[route("/app/account/skins")]
            AccountSkins {},
            #[route("/app/stats")]
            Stats {},
            #[route("/app/debug")]
            Debug {},

            #[layout(SettingsShell)]
                #[route("/app/settings/appearance")]
                SettingsAppearance {},
                #[route("/app/settings/minecraft")]
                SettingsMinecraft {},
                #[route("/app/settings/launcher")]
                SettingsLauncher {},
                #[route("/app/settings/java")]
                SettingsJava {},
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
        match self {
            Route::Startup { .. } => "Startup".to_string(),
            Route::OnboardingWelcome { .. } => "Welcome".to_string(),
            Route::OnboardingLanguage { .. } => "Language".to_string(),
            Route::OnboardingAccount { .. } => "Account".to_string(),
            Route::OnboardingPreferences { .. } => "Accessibility".to_string(),
            Route::OnboardingBundles { .. } => "Bundles".to_string(),
            Route::OnboardingDownloading { .. } => "Finishing".to_string(),
            Route::Home { .. } => "Home".to_string(),
            Route::Clusters { .. } => "Versions".to_string(),
            Route::ClusterOverview { .. } => "Cluster".to_string(),
            Route::ClusterLogs { .. } => "Cluster Logs".to_string(),
            Route::ProcessLogs { .. } => "Process".to_string(),
            Route::ClusterScreenshots { .. } => "Cluster Screenshots".to_string(),
            Route::ClusterMods { .. } => "Cluster Mods".to_string(),
            Route::ClusterShaders { .. } => "Cluster Shaders".to_string(),
            Route::ClusterTextures { .. } => "Cluster Textures".to_string(),
            Route::ClusterSettings { .. } => "Cluster Settings".to_string(),
            Route::Browser { .. } => "Browser".to_string(),
            Route::BrowserPackage { .. } => "Browser".to_string(),
            Route::Accounts { .. } => "Accounts".to_string(),
            Route::AccountSkins { .. } => "Skins".to_string(),
            Route::Stats { .. } => "Statistics".to_string(),
            Route::Debug { .. } => "Debug".to_string(),
            Route::SettingsAppearance { .. } => "Settings".to_string(),
            Route::SettingsMinecraft { .. } => "Minecraft Settings".to_string(),
            Route::SettingsLauncher { .. } => "Launcher Settings".to_string(),
            Route::SettingsJava { .. } => "Java".to_string(),
            Route::SettingsApis { .. } => "APIs".to_string(),
            Route::SettingsLanguage { .. } => "Language".to_string(),
            Route::SettingsDeveloper { .. } => "Developer Options".to_string(),
            Route::SettingsChangelog { .. } => "Changelog".to_string(),
            // Route::SettingsFeedback { .. } => "Feedback".to_string(),
            Route::NotFound { .. } => "Not Found".to_string(),
        }
    }
}

pub fn router() -> impl IntoElement {
    Router::<Route>::new(|| RouterConfig::default().with_initial_path(Route::Startup {}))
}
