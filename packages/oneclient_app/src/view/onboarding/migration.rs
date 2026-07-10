use std::collections::HashSet;

use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::SourceInstance;

use crate::components::{Button, Icon, IconType, ScrollArea};
use crate::hooks::{
    ClusterBundles, migration_detection, onboarding_bundles_items, use_migration,
    use_onboarding_bundles, use_onboarding_selection,
};
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::view::onboarding::{onboarding_illustration, onboarding_slide, step_heading};

/// Bundle-selection key, identical to `bundles::pkg_key` and the format the
/// downloading step reads. Must stay in sync with those.
fn pkg_key(cluster_id: i64, bundle_name: &str, package_id: &str) -> String {
    format!("{cluster_id}|{bundle_name}|{package_id}")
}

/// Build the pre-selected bundle set from the source install: for each new
/// cluster, match a source instance by version+loader, then select every
/// enabled/visible file of bundles whose category the user had installed.
fn build_migrated_selection(source: &[SourceInstance], new: &[ClusterBundles]) -> HashSet<String> {
    let mut selected = HashSet::new();

    for cb in new {
        let Some(instance) = source.iter().find(|o| {
            o.mc_version == cb.cluster.mc_version && o.mc_loader == cb.cluster.mc_loader
        }) else {
            continue;
        };

        for archive in &cb.archives {
            let category = archive.manifest.category.trim();
            let wanted = instance
                .categories
                .iter()
                .any(|c| c.eq_ignore_ascii_case(category));
            if !wanted {
                continue;
            }

            for file in &archive.manifest.files {
                if file.enabled && !file.hidden {
                    selected.insert(pkg_key(
                        cb.cluster.id,
                        &archive.manifest.name,
                        &file.kind.package_id(),
                    ));
                }
            }
        }
    }

    selected
}

/// Whether a new cluster exists matching this source instance's version+loader
/// (needed for the "dedicated dir" import option to have a target).
pub(crate) fn matching_new_cluster_id(
    instance: &SourceInstance,
    new: &[ClusterBundles],
) -> Option<i64> {
    new.iter()
        .find(|cb| {
            cb.cluster.mc_version == instance.mc_version
                && cb.cluster.mc_loader == instance.mc_loader
        })
        .map(|cb| cb.cluster.id)
}

#[derive(PartialEq)]
pub struct OnboardingMigration;

impl Component for OnboardingMigration {
    fn render(&self) -> impl IntoElement {
        let migration_query = use_migration();
        let bundles_query = use_onboarding_bundles();
        let selection = use_onboarding_selection();
        let mut selected = selection.selected;
        let mut seeded = selection.seeded;
        // Shared across the flow; the file import is dispatched later, on Setup.
        let mut import_folder = selection.import_folder;
        let import_dedicated = selection.import_dedicated;

        // No detection yet -> render nothing meaningful; the shell only routes
        // here when a source was detected, so this is a transient loading state.
        let Some(detection) = migration_detection(&migration_query) else {
            return rect().width(Size::fill()).height(Size::fill()).into_element();
        };
        let source_name = detection.source.display_name();
        let new_clusters = onboarding_bundles_items(&bundles_query).unwrap_or_default();
        let bundles_loaded = onboarding_bundles_items(&bundles_query).is_some();

        let chosen_folder = import_folder.read().clone();
        let any_importable = detection.instances.iter().any(|c| c.has_game_dir);

        // One card per detected version. Importable cards double as the file-
        // import selector; non-importable ones stay informational.
        let mut version_cards: Vec<Element> = Vec::new();
        for instance in &detection.instances {
            let selected_import = chosen_folder.as_deref() == Some(instance.folder_name.as_str());
            let dedicated_available =
                matching_new_cluster_id(instance, &new_clusters).is_some();

            version_cards.push(version_card(
                instance,
                selected_import,
                *import_dedicated.read(),
                dedicated_available,
                import_dedicated,
                {
                    let folder = instance.folder_name.clone();
                    move |_| import_folder.set(Some(folder.clone()))
                },
            ));
        }

        let content = rect()
            .vertical()
            .width(Size::fill())
            .spacing(16.)
            .child(step_heading(
                "Bring over your setup",
                &format!(
                    "We found data from your previous {source_name} install. Your installed \
                     categories are reselected automatically. Optionally, pick one version to \
                     copy its files (worlds, config, and packs)."
                ),
            ))
            // "Don't import files" is the default choice, pinned above the list.
            .maybe_child(any_importable.then(|| {
                import_choice_card(
                    "Don't import files",
                    "Start fresh — only your bundle selection carries over.",
                    None,
                    chosen_folder.is_none(),
                    move |_| import_folder.set(None),
                )
            }))
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .spacing(10.)
                    .child(
                        label()
                            .text("Detected versions")
                            .font_size(13.)
                            .font_weight(FontWeight::MEDIUM)
                            .color(colors::fg_secondary()),
                    )
                    .child(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .spacing(8.)
                            .children(version_cards),
                    ),
            )
            .into_element();

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .content(Content::Flex)
            .child(onboarding_slide(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .height(Size::fill())
                    .content(Content::Flex)
                    .child(
                        rect()
                            .width(Size::flex(1.0))
                            .height(Size::fill())
                            .center()
                            .padding(Gaps::new_all(48.))
                            .child(onboarding_illustration(IconType::ClockRewind)),
                    )
                    .child(
                        rect()
                            .vertical()
                            .width(Size::flex(1.0))
                            .height(Size::fill())
                            .content(Content::Flex)
                            .padding(Gaps::new(48., 80., 24., 24.))
                            .child(
                                ScrollArea::new()
                                    .width(Size::fill())
                                    .height(Size::flex(1.0))
                                    .child(content),
                            ),
                    ),
            ))
            .child(migration_nav(bundles_loaded, move || {
                // Apply migrated category selection and mark seeded so the
                // Bundles page does not overwrite it with "everything on". The
                // file import itself is deferred to the Setup step, like every
                // other onboarding action; the chosen folder/mode lives in the
                // shared selection state.
                let migrated = build_migrated_selection(&detection.instances, &new_clusters);
                selected.set(migrated);
                seeded.set(true);

                let _ = RouterContext::get().replace(Route::OnboardingLanguage {});
            }))
            .into_element()
    }
}

/// A detected source instance rendered as a card. Shows version + categories
/// (always-reselected info); when the instance has a game dir it also acts as
/// the file-import selector and, when selected, exposes the shared/dedicated
/// target choice inline.
fn version_card(
    instance: &SourceInstance,
    selected: bool,
    dedicated: bool,
    dedicated_available: bool,
    mut import_dedicated: State<bool>,
    on_select: impl FnMut(()) + 'static,
) -> Element {
    let importable = instance.has_game_dir;
    let mut on_select = on_select;

    let categories = if instance.categories.is_empty() {
        "No bundle categories".to_string()
    } else {
        instance.categories.join(", ")
    };
    let subtitle = format!("{} · {}", instance.mc_version, instance.mc_loader);

    let (bg, border_color) = if selected {
        (colors::brand(), colors::brand())
    } else {
        (colors::page_elevated(), colors::component_border())
    };

    let header = rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(3.)
                .child(
                    label()
                        .text(instance.folder_name.clone())
                        .font_size(14.)
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(subtitle)
                        .font_size(11.)
                        .color(if selected {
                            colors::fg_primary()
                        } else {
                            colors::fg_secondary()
                        }),
                )
                .child(
                    label()
                        .text(categories)
                        .font_size(11.)
                        .color(if selected {
                            colors::fg_primary()
                        } else {
                            colors::fg_secondary()
                        }),
                ),
        )
        // Selection affordance only when the instance actually has files.
        .maybe_child(importable.then(|| {
            if selected {
                Icon::new(IconType::Check).size(18.).into_element()
            } else {
                label()
                    .text("Import files")
                    .font_size(12.)
                    .color(colors::fg_secondary())
                    .into_element()
            }
        }));

    let mut card = rect()
        .vertical()
        .width(Size::fill())
        .spacing(12.)
        .padding(Gaps::new_symmetric(12., 14.))
        .corner_radius(CornerRadius::new_all(10.))
        .background(bg)
        .border(border_all_color(1., border_color))
        .child(header);

    if importable {
        card = card
            .a11y_role(AccessibilityRole::Button)
            .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
            .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
            .on_press(move |_| on_select(()));
    }

    // When this version is the chosen import, offer where its files should go.
    if selected {
        let mut targets = rect()
            .vertical()
            .width(Size::fill())
            .spacing(6.)
            .child(
                label()
                    .text("Where should these files go?")
                    .font_size(12.)
                    .color(colors::fg_primary()),
            )
            .child(target_option(
                "Shared game directory",
                "Available to every version.",
                !dedicated,
                move |_| import_dedicated.set(false),
            ));

        if dedicated_available {
            targets = targets.child(target_option(
                "This version only",
                "Keep the files isolated to the matching version.",
                dedicated,
                move |_| import_dedicated.set(true),
            ));
        }

        card = card.child(targets);
    }

    card.into_element()
}

/// A standalone import choice with no associated version (e.g. "Don't import").
fn import_choice_card(
    title: &str,
    subtitle: &str,
    trailing: Option<IconType>,
    active: bool,
    on_press: impl FnMut(()) + 'static,
) -> Element {
    let mut on_press = on_press;
    let (bg, border_color) = if active {
        (colors::brand(), colors::brand())
    } else {
        (colors::page_elevated(), colors::component_border())
    };

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(10.)
        .padding(Gaps::new_symmetric(12., 14.))
        .corner_radius(CornerRadius::new_all(10.))
        .background(bg)
        .border(border_all_color(1., border_color))
        .a11y_role(AccessibilityRole::Button)
        .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
        .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
        .on_press(move |_| on_press(()))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(2.)
                .child(
                    label()
                        .text(title.to_string())
                        .font_size(14.)
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(subtitle.to_string())
                        .font_size(11.)
                        .color(if active {
                            colors::fg_primary()
                        } else {
                            colors::fg_secondary()
                        }),
                ),
        )
        .maybe_child(
            active
                .then(|| Icon::new(IconType::Check).size(16.))
                .or_else(|| trailing.map(|i| Icon::new(i).size(16.))),
        )
        .into_element()
}

/// A shared/dedicated radio row nested inside a selected version card.
fn target_option(
    title: &str,
    subtitle: &str,
    active: bool,
    on_press: impl FnMut(()) + 'static,
) -> Element {
    let mut on_press = on_press;
    let border_color = if active {
        colors::fg_primary()
    } else {
        colors::component_border()
    };

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(10.)
        .padding(Gaps::new_symmetric(8., 12.))
        .corner_radius(CornerRadius::new_all(8.))
        .border(border_all_color(1., border_color))
        .a11y_role(AccessibilityRole::Button)
        .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
        .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
        .on_press(move |_| on_press(()))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(2.)
                .child(
                    label()
                        .text(title.to_string())
                        .font_size(13.)
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(subtitle.to_string())
                        .font_size(11.)
                        .color(colors::fg_primary()),
                ),
        )
        .maybe_child(active.then(|| Icon::new(IconType::Check).size(15.)))
        .into_element()
}

/// Custom nav: like `onboarding_nav` but the Next button runs `on_next` (which
/// applies the migration + navigates) instead of a plain route replace.
fn migration_nav(next_enabled: bool, on_next: impl FnMut() + 'static) -> impl IntoElement {
    let mut on_next = on_next;
    rect()
        .horizontal()
        .width(Size::fill())
        .main_align(Alignment::End)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .padding(Gaps::new(0., 40., 32., 40.))
        .child(
            Button::new()
                .secondary()
                .width(Size::px(128.))
                .on_press(move |_| {
                    let _ = RouterContext::get().replace(Route::OnboardingWelcome {});
                })
                .text("Back"),
        )
        .child(
            Button::new()
                .primary()
                .width(Size::px(140.))
                .enabled(next_enabled)
                .on_press(move |_| on_next())
                .text("Next")
                .child(Icon::new(IconType::ArrowRight).size(16.)),
        )
        .into_element()
}
