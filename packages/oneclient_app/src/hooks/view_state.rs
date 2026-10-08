use freya::prelude::*;
use oneclient_core::settings::{ViewLayout, ViewState};

use super::{use_dispatch, use_settings_snapshot};

#[derive(Clone, Copy)]
pub struct PersistedView {
    pub layout: State<ViewLayout>,
    pub sort: State<Option<String>>,
    pub show_hidden: State<bool>,
}

pub fn use_view_state(key: &str) -> PersistedView {
    let key = key.to_string();
    let settings = use_settings_snapshot().settings;
    let dispatch = use_dispatch();

    let initial = settings.view_state(&key);
    let init_layout = initial.layout;
    let init_sort = initial.sort;
    let init_show_hidden = initial.show_hidden.unwrap_or(false);
    let layout = use_state(move || init_layout);
    let sort = use_state(move || init_sort.clone());
    let show_hidden = use_state(move || init_show_hidden);

    let mut first = use_state(|| true);
    use_side_effect(move || {
        let next = ViewState {
            layout: *layout.read(),
            sort: sort.read().clone(),
            show_hidden: Some(*show_hidden.read()),
        };
        if *first.peek() {
            first.set(false);
            return;
        }
        if settings.view_state(&key) == next {
            return;
        }
        let mut updated = settings.clone();
        updated.set_view_state(key.clone(), next);
        dispatch.set_settings(updated);
    });

    PersistedView {
        layout,
        sort,
        show_hidden,
    }
}
