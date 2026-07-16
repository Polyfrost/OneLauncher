use freya::prelude::*;

use super::settings_page;
use crate::components::{Icon, IconType};
use crate::hooks::{
    changelog_error, changelog_groups, changelog_is_loading, latest_changelog_version,
    use_changelog, use_dispatch, use_settings_snapshot,
};
use crate::theme::colors;

#[derive(PartialEq)]
pub struct SettingsChangelog;

impl Component for SettingsChangelog {
    fn render(&self) -> impl IntoElement {
        let query = use_changelog();
        let dispatch = use_dispatch();
        let settings = use_settings_snapshot().settings;
        let mut marked = use_state(|| None::<String>);
        let installed_version = env!("CARGO_PKG_VERSION").to_string();

        // Opening this page counts as reading the changelog, so clear the sidebar
        // dot once the newest entry is known. `marked` keeps us from re-sending the
        // command while the settings snapshot catches up.
        if let Some(latest) = latest_changelog_version(&query) {
            let already_marked = marked.peek().as_deref() == Some(latest.as_str());
            let already_seen = settings.seen_changelog_version.as_deref() == Some(latest.as_str());

            if !already_marked && !already_seen {
                marked.set(Some(latest.clone()));
                dispatch.set_seen_changelog_version(latest);
            }
        }

        if changelog_is_loading(&query) {
            return settings_page()
                .child(
                    label()
                        .text("Loading changelog...")
                        .font_size(14.)
                        .color(colors::fg_secondary()),
                )
                .into_element();
        }

        if let Some(error) = changelog_error(&query) {
            return settings_page()
                .child(
                    label()
                        .text(format!("Couldn't load changelog: {error}"))
                        .font_size(14.)
                        .color(colors::fg_secondary()),
                )
                .into_element();
        }

        let groups = changelog_groups(&query).unwrap_or_default();

        settings_page()
            .children(groups.into_iter().enumerate().map(|(i, group)| {
                let current = group.version == installed_version;
                ReleaseCard {
                    version: group.version,
                    current,
                    changes: group.changes,
                    initially_open: i == 0,
                }
                .into_element()
            }))
            .into_element()
    }
}

#[derive(PartialEq)]
struct ReleaseCard {
    version: String,
    current: bool,
    changes: Vec<String>,
    initially_open: bool,
}

impl Component for ReleaseCard {
    fn render(&self) -> impl IntoElement {
        let mut open = use_state(|| self.initially_open);
        let is_open = *open.read();

        let title = if self.current {
            format!("{} (Currently Installed)", self.version)
        } else {
            self.version.clone()
        };

        let changes = self.changes.clone();

        use_drop(|| {
            Cursor::set(CursorIcon::default());
        });

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(8.)
            .padding(Gaps::new_symmetric(12., 16.))
            .corner_radius(CornerRadius::new_all(12.))
            .background(colors::page_elevated())
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .cross_align(Alignment::Center)
                    .main_align(Alignment::SpaceBetween)
                    .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                    .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
                    .on_press(move |_| {
                        let next = !*open.peek();
                        *open.write() = next;
                    })
                    .child(
                        label()
                            .text(title)
                            .font_size(18.)
                            .font_weight(FontWeight::SEMI_BOLD)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        Icon::new(if is_open {
                            IconType::ChevronDown
                        } else {
                            IconType::ChevronRight
                        })
                        .size(18.),
                    ),
            )
            .maybe_child(is_open.then(|| {
                rect()
                    .vertical()
                    .width(Size::fill())
                    .spacing(4.)
                    .padding(Gaps::new(0., 0., 0., 6.))
                    .children(if changes.is_empty() {
                        vec![
                            rect()
                                .child(
                                    label()
                                        .text("No changes recorded for this version.")
                                        .font_size(12.)
                                        .color(colors::fg_secondary()),
                                )
                                .into_element(),
                        ]
                    } else {
                        changes
                            .into_iter()
                            .map(|change| {
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .spacing(8.)
                                    .child(
                                        label()
                                            .text("•")
                                            .font_size(12.)
                                            .color(colors::fg_secondary()),
                                    )
                                    .child(
                                        label()
                                            .text(change)
                                            .font_size(12.)
                                            .width(Size::flex(1.0))
                                            .color(colors::fg_primary()),
                                    )
                                    .into_element()
                            })
                            .collect()
                    })
                    .into_element()
            }))
            .into_element()
    }
}
