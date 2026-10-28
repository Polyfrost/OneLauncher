use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::{MigrationDetection, MigrationSource, SourceInstance};

use crate::components::{Button, Icon, IconType, ScrollArea};
use crate::hooks::{
    ClusterBundles, migration_detection, onboarding_bundles_items, use_migration,
    use_onboarding_bundles, use_onboarding_selection,
};
use crate::i18n::{t, tf};
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::view::onboarding::{
    choice_row, onboarding_illustration, onboarding_slide, step_heading,
};

fn migrated_categories(source: &[SourceInstance]) -> Vec<String> {
    let mut categories: Vec<String> = Vec::new();
    for instance in source {
        for category in &instance.categories {
            if !categories.iter().any(|c| c.eq_ignore_ascii_case(category)) {
                categories.push(category.clone());
            }
        }
    }
    categories
}

/// Needed for the "dedicated dir" import option to have a target
pub(crate) fn matching_new_cluster_id(
    instance: &SourceInstance,
    new: &[ClusterBundles],
) -> Option<i64> {
    let want = instance.import_version();
    let loader = instance.import_loader();
    new.iter()
        .find(|cb| cb.cluster.mc_version == want && cb.cluster.mc_loader == loader)
        .map(|cb| cb.cluster.id)
}

#[derive(PartialEq)]
pub struct OnboardingMigration;

impl Component for OnboardingMigration {
    fn render(&self) -> impl IntoElement {
        let migration_query = use_migration();
        let bundles_query = use_onboarding_bundles();
        let selection = use_onboarding_selection();
        let mut categories = selection.migrated_categories;
        // Shared across the flow the file import is dispatched later on Setup
        let mut import_folder = selection.import_folder;
        let import_dedicated = selection.import_dedicated;

        // The shell only routes here when a source was detected so this is transient
        let Some(detection) = migration_detection(&migration_query) else {
            return rect()
                .width(Size::fill())
                .height(Size::fill())
                .into_element();
        };
        let source_name = detection.source.display_name();
        let is_vanilla = detection.source == MigrationSource::Vanilla;
        let new_clusters = onboarding_bundles_items(&bundles_query).unwrap_or_default();
        let bundles_loaded = onboarding_bundles_items(&bundles_query).is_some();

        let chosen_folder = import_folder.read().clone();
        let any_importable = detection.instances.iter().any(|c| c.has_game_dir);

        let (title, subtitle) = if is_vanilla {
            (
                t("oneclient.onboarding.migration.vanilla.title"),
                t("oneclient.onboarding.migration.vanilla.subtitle").to_string(),
            )
        } else {
            (
                t("oneclient.onboarding.migration.title"),
                tf("oneclient.onboarding.migration.subtitle", &[&source_name]),
            )
        };
        let heading = step_heading(title, &subtitle);

        let content = rect()
            .vertical()
            .width(Size::fill())
            .spacing(16.)
            .child(heading)
            // "Don't import files" is the default choice pinned above the list
            .maybe_child(any_importable.then(|| {
                import_choice_card(
                    t("oneclient.onboarding.migration.skip.title"),
                    if is_vanilla {
                        t("oneclient.onboarding.migration.skip.vanilla")
                    } else {
                        t("oneclient.onboarding.migration.skip.description")
                    },
                    None,
                    chosen_folder.is_none(),
                    move |_| import_folder.set(None),
                )
            }))
            .child(import_body(
                &detection,
                is_vanilla,
                chosen_folder.as_deref(),
                &new_clusters,
                import_folder,
                import_dedicated,
            ))
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
                            .child(onboarding_illustration(if is_vanilla {
                                IconType::FolderDownload
                            } else {
                                IconType::ClockRewind
                            })),
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
                categories.set(Some(migrated_categories(&detection.instances)));

                let _ = RouterContext::get().replace(Route::OnboardingLanguage {});
            }))
            .into_element()
    }
}

fn import_body(
    detection: &MigrationDetection,
    is_vanilla: bool,
    chosen_folder: Option<&str>,
    new_clusters: &[ClusterBundles],
    mut import_folder: State<Option<String>>,
    import_dedicated: State<bool>,
) -> Element {
    if is_vanilla {
        let folder = detection
            .instances
            .first()
            .map(|instance| instance.folder_name.clone())
            .unwrap_or_default();

        return import_choice_card(
            t("oneclient.onboarding.migration.vanilla.copy.title"),
            t("oneclient.onboarding.migration.vanilla.copy.description"),
            None,
            chosen_folder.is_some(),
            move |_| import_folder.set(Some(folder.clone())),
        );
    }

    let mut version_cards: Vec<Element> = Vec::new();
    for instance in &detection.instances {
        let selected_import = chosen_folder == Some(instance.folder_name.as_str());
        let dedicated_available = matching_new_cluster_id(instance, new_clusters).is_some();

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

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .child(
            label()
                .text(t("oneclient.onboarding.migration.detected_versions"))
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
        )
        .into_element()
}

/// When the instance has a game dir it also acts as the file-import selector and exposes the shared/dedicated target choice
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
        t("oneclient.onboarding.migration.no_categories").to_string()
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
                .child(label().text(subtitle).font_size(11.).color(if selected {
                    colors::fg_primary()
                } else {
                    colors::fg_secondary()
                }))
                .child(label().text(categories).font_size(11.).color(if selected {
                    colors::fg_primary()
                } else {
                    colors::fg_secondary()
                })),
        )
        // Selection affordance only when the instance actually has files
        .maybe_child(importable.then(|| {
            if selected {
                Icon::new(IconType::Check).size(18.).into_element()
            } else {
                label()
                    .text(t("oneclient.onboarding.migration.import_files"))
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
            .cursor(CursorIcon::Pointer)
            .on_press(move |_| on_select(()));
    }

    if selected {
        let mut targets = rect()
            .vertical()
            .width(Size::fill())
            .spacing(6.)
            .child(
                label()
                    .text(t("oneclient.onboarding.migration.target.title"))
                    .font_size(12.)
                    .color(colors::fg_primary()),
            )
            .child(choice_row(
                t("oneclient.onboarding.migration.target.shared.title"),
                t("oneclient.onboarding.migration.target.shared.description"),
                !dedicated,
                move |_| import_dedicated.set(false),
            ));

        if dedicated_available {
            targets = targets.child(choice_row(
                t("oneclient.onboarding.migration.target.dedicated.title"),
                t("oneclient.onboarding.migration.target.dedicated.description"),
                dedicated,
                move |_| import_dedicated.set(true),
            ));
        }

        card = card.child(targets);
    }

    card.into_element()
}

/// A standalone import choice with no associated version (e.g. "Don't import")
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
        .cursor(CursorIcon::Pointer)
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

/// Like `onboarding_nav` but Next runs `on_next` (apply migration + navigate) instead of a plain route replace
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
                    let _ = RouterContext::get().replace(Route::OnboardingTerms {});
                })
                .text(t("oneclient.common.back")),
        )
        .child(
            Button::new()
                .primary()
                .width(Size::px(140.))
                .enabled(next_enabled)
                .on_press(move |_| on_next())
                .text(t("oneclient.common.next"))
                .child(Icon::new(IconType::ArrowRight).size(16.)),
        )
        .into_element()
}
