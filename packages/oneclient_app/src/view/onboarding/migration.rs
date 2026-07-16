use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::{MigrationDetection, SourceInstance};

use crate::components::{Button, Icon, IconType, ScrollArea};
use crate::hooks::{
    ClusterBundles, ImportSelection, migration_detections, onboarding_bundles_items, use_migration,
    use_onboarding_bundles, use_onboarding_selection,
};
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::view::onboarding::{
    choice_row, onboarding_illustration, onboarding_slide, step_heading,
};

fn migrated_categories(detections: &[MigrationDetection]) -> Vec<String> {
    let mut categories: Vec<String> = Vec::new();
    for instance in detections.iter().flat_map(|d| &d.instances) {
        for category in &instance.categories {
            if !categories.iter().any(|c| c.eq_ignore_ascii_case(category)) {
                categories.push(category.clone());
            }
        }
    }
    categories
}

pub(crate) fn find_instance<'a>(
    detections: &'a [MigrationDetection],
    chosen: &ImportSelection,
) -> Option<&'a SourceInstance> {
    detections
        .iter()
        .find(|d| d.source == chosen.source)?
        .instances
        .iter()
        .find(|i| i.folder_name == chosen.folder_name)
}

pub(crate) fn sources_sentence(detections: &[MigrationDetection]) -> String {
    let names: Vec<&str> = detections.iter().map(|d| d.source.display_name()).collect();

    match names.split_last() {
        None => String::new(),
        Some((last, [])) => (*last).to_string(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    }
}

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
        let mut categories = selection.migrated_categories;
        let mut import_selection = selection.import_selection;
        let import_dedicated = selection.import_dedicated;

        let detections = migration_detections(&migration_query);
        if detections.is_empty() {
            return rect()
                .width(Size::fill())
                .height(Size::fill())
                .into_element();
        }

        let new_clusters = onboarding_bundles_items(&bundles_query).unwrap_or_default();
        let bundles_loaded = onboarding_bundles_items(&bundles_query).is_some();

        let chosen = import_selection.read().clone();
        let any_importable = detections
            .iter()
            .flat_map(|d| &d.instances)
            .any(|c| c.has_game_dir);

        let mut source_groups: Vec<Element> = Vec::new();
        for detection in &detections {
            let source = detection.source;
            let mut version_cards: Vec<Element> = Vec::new();

            for instance in &detection.instances {
                let selected_import = chosen
                    .as_ref()
                    .is_some_and(|c| c.source == source && c.folder_name == instance.folder_name);
                let dedicated_available =
                    matching_new_cluster_id(instance, &new_clusters).is_some();

                version_cards.push(version_card(
                    instance,
                    selected_import,
                    *import_dedicated.read(),
                    dedicated_available,
                    import_dedicated,
                    {
                        let folder_name = instance.folder_name.clone();
                        move |_| {
                            import_selection.set(Some(ImportSelection {
                                source,
                                folder_name: folder_name.clone(),
                            }))
                        }
                    },
                ));
            }

            source_groups.push(source_group(
                source.display_name(),
                detection.instances.len(),
                version_cards,
            ));
        }

        let content = rect()
            .vertical()
            .width(Size::fill())
            .spacing(16.)
            .child(step_heading(
                "Bring over your setup",
                &format!(
                    "We found data from {}. Your installed categories are reselected \
                     automatically. Optionally, pick one version to copy its files \
                     (worlds, config, and packs).",
                    sources_sentence(&detections)
                ),
            ))
            .maybe_child(any_importable.then(|| {
                import_choice_card(
                    "Don't import files",
                    "Start fresh — only your bundle selection carries over.",
                    None,
                    chosen.is_none(),
                    move |_| import_selection.set(None),
                )
            }))
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .spacing(16.)
                    .children(source_groups),
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
                categories.set(Some(migrated_categories(&detections)));

                let _ = RouterContext::get().replace(Route::OnboardingLanguage {});
            }))
            .into_element()
    }
}

pub fn source_group(name: &str, instance_count: usize, cards: Vec<Element>) -> Element {
    let counted = if instance_count == 1 {
        "1 version".to_string()
    } else {
        format!("{instance_count} versions")
    };

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::Flex)
                .cross_align(Alignment::Center)
                .spacing(8.)
                .child(
                    label()
                        .text(name.to_string())
                        .font_size(13.)
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_primary()),
                )
                .child(
                    rect()
                        .width(Size::flex(1.0))
                        .height(Size::px(1.))
                        .background(colors::component_border()),
                )
                .child(
                    label()
                        .text(counted)
                        .font_size(11.)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(8.)
                .children(cards),
        )
        .into_element()
}

pub fn chip(text: String, on_brand: bool) -> Element {
    let (bg, border, fg) = if on_brand {
        (
            colors::fg_primary().with_a(38),
            colors::fg_primary().with_a(70),
            colors::fg_primary(),
        )
    } else {
        (
            colors::component_bg(),
            colors::component_border(),
            colors::fg_secondary(),
        )
    };

    rect()
        .center()
        .padding(Gaps::new_symmetric(2., 8.))
        .corner_radius(CornerRadius::new_all(999.))
        .background(bg)
        .border(border_all_color(1., border))
        .child(
            label()
                .text(text)
                .font_size(10.)
                .font_weight(FontWeight::MEDIUM)
                .color(fg),
        )
        .into_element()
}

pub fn version_card(
    instance: &SourceInstance,
    selected: bool,
    dedicated: bool,
    dedicated_available: bool,
    mut import_dedicated: State<bool>,
    on_select: impl FnMut(()) + 'static,
) -> Element {
    let importable = instance.has_game_dir;
    let mut on_select = on_select;

    let (bg, border_color) = if selected {
        (colors::brand(), colors::brand())
    } else {
        (colors::page_elevated(), colors::component_border())
    };
    let subtitle_color = if selected {
        colors::fg_primary()
    } else {
        colors::fg_secondary()
    };

    let icon_tile = rect()
        .width(Size::px(34.))
        .height(Size::px(34.))
        .center()
        .corner_radius(CornerRadius::new_all(8.))
        .background(if selected {
            colors::fg_primary().with_a(38)
        } else {
            colors::component_bg()
        })
        .child(
            Icon::new(if !importable {
                IconType::FileX02
            } else if selected {
                IconType::FolderCheck
            } else {
                IconType::Folder
            })
            .size(17.)
            .color(subtitle_color),
        );

    let header = rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .child(icon_tile)
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
                        .text(format!("{} · {}", instance.mc_version, instance.mc_loader))
                        .font_size(11.)
                        .color(subtitle_color),
                ),
        )
        .child(if !importable {
            label()
                .text("No files")
                .font_size(11.)
                .color(colors::fg_secondary())
                .into_element()
        } else if selected {
            Icon::new(IconType::Check).size(18.).into_element()
        } else {
            label()
                .text("Import files")
                .font_size(12.)
                .color(colors::fg_secondary())
                .into_element()
        });

    let categories = if instance.categories.is_empty() {
        label()
            .text("No bundle categories")
            .font_size(11.)
            .color(subtitle_color)
            .into_element()
    } else {
        rect()
            .horizontal()
            .width(Size::fill())
            .cross_align(Alignment::Center)
            .spacing(6.)
            .children(
                instance
                    .categories
                    .iter()
                    .map(|c| chip(c.clone(), selected))
                    .collect::<Vec<_>>(),
            )
            .into_element()
    };

    let mut card = rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .padding(Gaps::new_symmetric(12., 14.))
        .corner_radius(CornerRadius::new_all(10.))
        .background(bg)
        .border(border_all_color(1., border_color))
        .child(header)
        .child(categories);

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
            .child(choice_row(
                "Shared game directory",
                "Available to every version.",
                !dedicated,
                move |_| import_dedicated.set(false),
            ));

        if dedicated_available {
            targets = targets.child(choice_row(
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
                    let _ = RouterContext::get().replace(Route::OnboardingTerms {});
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use oneclient_core::MigrationSource;
    use oneclient_core::packages::domain::GameLoader;

    use super::*;

    fn instance(folder: &str, categories: &[&str]) -> SourceInstance {
        SourceInstance {
            instance_id: 0,
            folder_name: folder.to_string(),
            mc_version: "1.21.10".to_string(),
            mc_loader: GameLoader::Fabric,
            categories: categories.iter().map(|c| c.to_string()).collect(),
            has_game_dir: true,
        }
    }

    fn detection(source: MigrationSource, instances: Vec<SourceInstance>) -> MigrationDetection {
        MigrationDetection {
            source,
            root: PathBuf::from("/tmp"),
            instances,
        }
    }

    fn both() -> Vec<MigrationDetection> {
        vec![
            detection(
                MigrationSource::OneClientV1,
                vec![instance("1.21", &["HUD", "QoL"])],
            ),
            detection(
                MigrationSource::LunarClient,
                vec![instance("1.21", &["QoL", "PvP"]), instance("1.8", &["PvP"])],
            ),
        ]
    }

    #[test]
    fn categories_pool_across_every_detected_launcher() {
        let categories = migrated_categories(&both());

        assert_eq!(categories, vec!["HUD", "QoL", "PvP"]);
    }

    #[test]
    fn categories_are_not_duplicated_across_launchers() {
        // "QoL" is present in both detections above.
        let categories = migrated_categories(&both());

        assert_eq!(categories.iter().filter(|c| *c == "QoL").count(), 1);
    }

    #[test]
    fn same_folder_name_in_two_launchers_resolves_to_the_chosen_one() {
        // Both launchers have a "1.21"; only the source tells them apart.
        let chosen = ImportSelection {
            source: MigrationSource::LunarClient,
            folder_name: "1.21".to_string(),
        };

        let detections = both();
        let found = find_instance(&detections, &chosen).expect("instance");

        assert_eq!(found.categories, vec!["QoL", "PvP"]);
    }

    #[test]
    fn a_selection_pointing_at_nothing_resolves_to_none() {
        let chosen = ImportSelection {
            source: MigrationSource::OneClientV1,
            folder_name: "1.8".to_string(),
        };

        assert!(find_instance(&both(), &chosen).is_none());
    }

    #[test]
    fn sources_read_as_a_sentence() {
        let detections = both();

        assert_eq!(
            sources_sentence(&detections),
            "OneClient and Lunar Client".to_string()
        );
        assert_eq!(sources_sentence(&detections[..1]), "OneClient".to_string());
        assert_eq!(sources_sentence(&[]), String::new());
    }
}
