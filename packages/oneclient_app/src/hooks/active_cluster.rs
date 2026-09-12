use std::collections::{HashMap, HashSet};

use freya::prelude::*;
use oneclient_content::packages::ProviderId;
use oneclient_db::models::ClusterId;

#[derive(Clone)]
pub struct ActiveClusterState(pub State<Option<ClusterId>>);

pub fn use_provide_active_cluster(active: ActiveClusterState) {
    use_hook(move || provide_root_context(active));
}

pub fn use_active_cluster_id() -> State<Option<ClusterId>> {
    consume_root_context::<ActiveClusterState>().0
}

/// Startup curtain stays up past the Startup route until home content settles
/// so the app never reveals a half-populated home
#[derive(Clone, Copy)]
pub struct SplashState {
    pub active: State<bool>,
    pub home_ready: State<bool>,
}

pub fn use_provide_splash(state: SplashState) {
    use_provide_root_context(move || state);
}

pub fn use_splash() -> SplashState {
    consume_root_context::<SplashState>()
}

/// The settings value the window was built from, carried in from startup because
/// the settings channel is still on its defaults during the first frames
#[derive(Clone, Copy)]
pub struct StartMaximizedState(pub bool);

pub fn use_provide_start_maximized(state: StartMaximizedState) {
    use_provide_root_context(move || state);
}

pub fn use_start_maximized() -> bool {
    consume_root_context::<StartMaximizedState>().0
}

pub const BROWSER_COMPAT_DEFAULT: bool = true;

#[derive(Clone)]
pub struct BrowserCompatState(pub State<bool>);

pub fn use_provide_browser_compat(state: BrowserCompatState) {
    use_hook(move || provide_root_context(state));
}

pub fn use_browser_compat() -> State<bool> {
    consume_root_context::<BrowserCompatState>().0
}

#[derive(Clone)]
pub struct LinkConfirmState(pub State<Option<String>>);

pub fn use_provide_link_confirm(state: LinkConfirmState) {
    use_provide_root_context(move || state.clone());
}

pub fn use_link_confirm() -> State<Option<String>> {
    consume_root_context::<LinkConfirmState>().0
}

#[derive(Clone)]
pub struct BrowserUiState {
    pub query: String,
    pub provider: ProviderId,
    pub categories: Vec<String>,
    pub page: usize,
}

impl Default for BrowserUiState {
    fn default() -> Self {
        Self {
            query: String::new(),
            provider: ProviderId::Modrinth,
            categories: Vec::new(),
            page: 0,
        }
    }
}

#[derive(Clone)]
pub struct BrowserStateStore(pub State<HashMap<String, BrowserUiState>>);

pub fn use_provide_browser_state(store: BrowserStateStore) {
    use_hook(move || provide_root_context(store));
}

pub fn use_browser_state_store() -> State<HashMap<String, BrowserUiState>> {
    consume_root_context::<BrowserStateStore>().0
}

#[derive(Clone)]
pub struct OnboardingSelectionState {
    pub selected: State<HashSet<String>>,
    /// Until set the shell keeps re-deriving `selected` from the catalog so a
    /// late-settling bundle list still gets the right defaults
    pub user_touched: State<bool>,
    /// `None` means "no migration decision yet"
    pub migrated_categories: State<Option<Vec<String>>>,
    pub language: State<String>,
    pub reduce_motion: State<bool>,
    pub predownload: State<bool>,
    pub setup_started: State<bool>,
    /// `None` = don't import
    pub import_folder: State<Option<String>>,
    pub import_dedicated: State<bool>,
    pub picks_location: State<bool>,
}

pub fn use_provide_onboarding_selection(state: OnboardingSelectionState) {
    use_hook(move || provide_root_context(state));
}

pub fn use_onboarding_selection() -> OnboardingSelectionState {
    consume_root_context::<OnboardingSelectionState>()
}

#[cfg(test)]
mod tests {
    use freya_testing::TestingRunner;

    use super::*;

    /// Stands in for a route layout: it owns the state and publishes it as a root
    /// context, so unmounting it drops the value the context still points at
    #[derive(PartialEq)]
    struct Shell;

    impl Component for Shell {
        fn render(&self) -> impl IntoElement {
            let active = use_state(|| None::<ClusterId>);
            use_provide_active_cluster(ActiveClusterState(active));
            rect().child(Reader)
        }
    }

    #[derive(PartialEq)]
    struct Reader;

    impl Component for Reader {
        fn render(&self) -> impl IntoElement {
            let active = *use_active_cluster_id().read();
            label().text(format!("{active:?}"))
        }
    }

    fn app() -> impl IntoElement {
        let mounted = use_consume::<State<bool>>();
        rect().maybe_child(mounted().then(|| Shell.into_element()))
    }

    #[test]
    fn a_remounted_shell_replaces_the_dropped_root_state() {
        let (mut test, mut mounted) = TestingRunner::new(
            app,
            Size2D::new(300., 300.),
            |runner| runner.provide_root_context(|| State::create(true)),
            1.,
        );
        test.sync_and_update();

        // Leaving the shell drops the state the root context holds
        test.run_in(|| mounted.set(false));
        test.sync_and_update();

        // Coming back must not hand the reader that dropped state
        test.run_in(|| mounted.set(true));
        test.sync_and_update();
    }
}
