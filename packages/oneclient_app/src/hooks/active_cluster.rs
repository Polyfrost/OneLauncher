use std::collections::{HashMap, HashSet};

use freya::prelude::*;
use oneclient_core::packages::ProviderId;
use oneclient_db::models::ClusterId;

#[derive(Clone)]
pub struct ActiveClusterState(pub State<Option<ClusterId>>);

pub fn use_provide_active_cluster(active: ActiveClusterState) {
    use_provide_root_context(move || active.clone());
}

pub fn use_active_cluster_id() -> State<Option<ClusterId>> {
    consume_root_context::<ActiveClusterState>().0
}

#[derive(Clone)]
pub struct BrowserCompatState(pub State<bool>);

pub fn use_provide_browser_compat(state: BrowserCompatState) {
    use_provide_root_context(move || state.clone());
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
    use_provide_root_context(move || store.clone());
}

pub fn use_browser_state_store() -> State<HashMap<String, BrowserUiState>> {
    consume_root_context::<BrowserStateStore>().0
}

#[derive(Clone)]
pub struct OnboardingSelectionState {
    pub selected: State<HashSet<String>>,
    /// Set once the user changes the selection themselves. Until then the shell
    /// keeps re-deriving `selected` from the catalog, so a bundle list that
    /// settles late still gets the right defaults.
    pub user_touched: State<bool>,
    /// Bundle categories the user had in their v1 install, once the migration
    /// step has been answered. `None` means "no migration decision yet".
    pub migrated_categories: State<Option<Vec<String>>>,
    pub language: State<String>,
    pub reduce_motion: State<bool>,
    pub predownload: State<bool>,
    pub setup_started: State<bool>,
    /// v1-migration: source folder to import files from (`None` = don't import).
    pub import_folder: State<Option<String>>,
    /// v1-migration: import into the matching cluster's own dir instead of shared.
    pub import_dedicated: State<bool>,
}

pub fn use_provide_onboarding_selection(state: OnboardingSelectionState) {
    use_provide_root_context(move || state.clone());
}

pub fn use_onboarding_selection() -> OnboardingSelectionState {
    consume_root_context::<OnboardingSelectionState>()
}
