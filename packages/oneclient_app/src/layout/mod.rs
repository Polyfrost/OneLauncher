mod animated_outlet;
mod app_shell;
mod artwork_brand;
pub(crate) use artwork_brand::ArtworkBrand;
mod cluster_shell;
mod ipc_commands;
mod onboarding_shell;
mod pending_launch;
mod root_layout;
mod settings_shell;

pub use animated_outlet::AnimatedAppOutlet;
pub use app_shell::{AppShell, HOME_BACKGROUND_ASSET, HomeArtPrefetch};
pub use cluster_shell::{ClusterShell, cluster_content};
pub use ipc_commands::use_ipc_commands;
pub use onboarding_shell::OnboardingShell;
pub use pending_launch::PendingLaunchDriver;
pub use root_layout::RootLayout;
pub(crate) use settings_shell::SearchItem;
pub use settings_shell::SettingsShell;
mod dialog_focus;
mod workspace_dialog;
mod workspace_rail;
pub(crate) use workspace_dialog::use_app_page;

pub use dialog_focus::DialogFocusPlugin;
pub(crate) use dialog_focus::focus_is_within;
