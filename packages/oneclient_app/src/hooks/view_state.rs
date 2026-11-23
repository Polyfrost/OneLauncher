use std::collections::BTreeMap;

use freya::prelude::*;
use oneclient_core::settings::{LauncherSettings, ViewLayout, ViewState};

use super::{use_dispatch, use_settings_snapshot};

#[derive(Clone, Copy)]
pub struct PersistedView {
    pub layout: State<ViewLayout>,
    pub sort: State<Option<String>>,
    pub filters: State<BTreeMap<String, String>>,
}

pub fn use_view_state(key: &str) -> PersistedView {
    let key = key.to_string();
    let settings = use_settings_snapshot().settings;
    let dispatch = use_dispatch();

    let initial = settings.view_state(&key);
    let init_layout = initial.layout;
    let init_sort = initial.sort.clone();
    let init_filters = initial.filters.clone();
    let layout = use_state(move || init_layout);
    let sort = use_state(move || init_sort.clone());
    let filters = use_state(move || init_filters.clone());

    let mut last = use_state(move || initial);
    use_side_effect(move || {
        let next = ViewState {
            layout: *layout.read(),
            sort: sort.read().clone(),
            filters: filters.read().clone(),
        };
        let previous = last.peek().clone();
        if previous == next {
            return;
        }
        last.set(next.clone());
        dispatch.edit_settings(|settings| apply_view_changes(settings, &key, &previous, next));
    });

    PersistedView {
        layout,
        sort,
        filters,
    }
}

fn apply_view_changes(
    settings: &mut LauncherSettings,
    key: &str,
    previous: &ViewState,
    next: ViewState,
) {
    let mut current = settings.view_state(key);
    if previous.layout != next.layout {
        current.layout = next.layout;
    }
    if previous.sort != next.sort {
        current.sort = next.sort;
    }
    for filter in previous.filters.keys() {
        if !next.filters.contains_key(filter) {
            current.filters.remove(filter);
        }
    }
    for (filter, value) in next.filters {
        if previous.filters.get(&filter) != Some(&value) {
            current.filters.insert(filter, value);
        }
    }
    settings.set_view_state(key, current);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_changes_preserve_settings_written_since_mount() {
        let mut settings = LauncherSettings {
            microsoft_java_migrated: true,
            ..LauncherSettings::default()
        };
        settings.set_view_state(
            "browse",
            ViewState {
                sort: Some("downloads".into()),
                filters: BTreeMap::from([("provider".into(), "modrinth".into())]),
                ..ViewState::default()
            },
        );
        let previous = ViewState {
            filters: BTreeMap::from([("category".into(), "utility".into())]),
            ..ViewState::default()
        };
        let next = ViewState {
            layout: ViewLayout::List,
            filters: BTreeMap::from([("loader".into(), "fabric".into())]),
            ..ViewState::default()
        };

        apply_view_changes(&mut settings, "browse", &previous, next);

        assert!(settings.microsoft_java_migrated);
        assert_eq!(
            settings.view_state("browse"),
            ViewState {
                layout: ViewLayout::List,
                sort: Some("downloads".into()),
                filters: BTreeMap::from([
                    ("loader".into(), "fabric".into()),
                    ("provider".into(), "modrinth".into()),
                ]),
            },
        );
    }
}
