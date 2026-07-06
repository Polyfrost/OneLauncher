mod animated_outlet;
mod app_shell;
mod cluster_shell;
mod onboarding_shell;
mod root_layout;
mod settings_shell;

pub use animated_outlet::AnimatedAppOutlet;
pub use app_shell::{AppShell, HOME_BACKGROUND_ASSET};
pub(crate) use app_shell::gradient_overlay_radial;
pub use cluster_shell::{ClusterShell, cluster_content};
pub use onboarding_shell::OnboardingShell;
pub use root_layout::RootLayout;
pub use settings_shell::SettingsShell;
