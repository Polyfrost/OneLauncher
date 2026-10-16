use freya::prelude::*;
use oneclient_common::domain::GameLoader;
use std::path::PathBuf;

use oneclient_core::{
    ExternalDetection, ExternalInstance, detect_external_folder, same_external_folder,
};

use super::cards::{BUNDLE_CARD_H, BUNDLE_COLUMNS, CellCard, cell_card};
use crate::components::{Button, GRID_GAP, Icon, IconType, ScrollArea, centered_spinner};
use crate::hooks::{Actions, external_launchers, use_dispatch, use_external_launchers};
use crate::launcher::off_ui;
use crate::theme::colors;
use crate::ui::fixed_grid;

/// Its own component so the disk scan only runs once this step is shown
///
/// Fills the wizard body (the shell leaves scrolling to it) so the loading
/// and empty states sit in the middle of the pane
#[derive(PartialEq)]
pub struct ImportStep {
    pub chosen: State<Vec<ExternalInstance>>,
    pub extra: State<Vec<ExternalDetection>>,
}

impl Component for ImportStep {
    fn render(&self) -> impl IntoElement {
        let query = use_external_launchers();
        let dispatch = use_dispatch();
        let chosen = self.chosen;
        let extra = self.extra;

        let Some(found) = external_launchers(&query) else {
            return rect()
                .width(Size::fill())
                .height(Size::fill())
                .child(centered_spinner("Looking for other launchers..."))
                .into_element();
        };

        let mut detections = found;
        for added in extra.read().iter() {
            if !detections.iter().any(|d| d.root == added.root) {
                detections.push(added.clone());
            }
        }

        let listed: Vec<PathBuf> = detections.iter().map(|d| d.root.clone()).collect();

        if detections.is_empty() {
            return nothing_found(dispatch, extra, listed);
        }

        ScrollArea::new()
            .width(Size::fill())
            .height(Size::fill())
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .spacing(20.)
                    .children(
                        detections
                            .iter()
                            .map(|detection| launcher_section(detection, chosen)),
                    )
                    .child(not_listed_row(dispatch, extra, listed)),
            )
            .into_element()
    }
}

fn nothing_found(
    dispatch: Actions,
    extra: State<Vec<ExternalDetection>>,
    listed: Vec<PathBuf>,
) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .center()
        .padding(Gaps::new_symmetric(0., 32.))
        .spacing(8.)
        .child(
            Icon::new(IconType::FolderDownload)
                .size(28.)
                .color(colors::fg_secondary()),
        )
        .child(
            label()
                .text("No other launchers found")
                .font_size(14.)
                .font_weight(FontWeight::MEDIUM)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(
                    "Prism Launcher, MultiMC, PolyMC and the Modrinth App were checked in their usual folders.",
                )
                .width(Size::fill())
                .font_size(12.)
                .text_align(TextAlign::Center)
                .color(colors::fg_secondary()),
        )
        .child(
            label()
                .text("Using a portable or moved install? Choose its folder instead.")
                .width(Size::fill())
                .font_size(12.)
                .text_align(TextAlign::Center)
                .color(colors::fg_secondary()),
        )
        .child(
            rect()
                .padding(Gaps::new(8., 0., 0., 0.))
                .child(choose_folder_button(dispatch, extra, listed)),
        )
        .into_element()
}

fn not_listed_row(
    dispatch: Actions,
    extra: State<Vec<ExternalDetection>>,
    listed: Vec<PathBuf>,
) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .padding(Gaps::new_symmetric(12., 14.))
        .corner_radius(CornerRadius::new_all(10.))
        .background(colors::page_elevated())
        .child(
            label()
                .width(Size::flex(1.0))
                .text("Launcher missing? Portable and moved installs can be added by folder.")
                .font_size(12.)
                .color(colors::fg_secondary()),
        )
        .child(choose_folder_button(dispatch, extra, listed))
        .into_element()
}

/// `listed` are the roots already on screen; picking one of them again,
/// however the path is spelled, must not add a second copy of its section
/// (ticking an instance in both would import it twice)
fn choose_folder_button(
    dispatch: Actions,
    mut extra: State<Vec<ExternalDetection>>,
    listed: Vec<PathBuf>,
) -> Element {
    Button::new()
        .secondary()
        .small()
        .on_press(move |_| {
            let dispatch = dispatch.clone();
            let listed = listed.clone();
            spawn(async move {
                let Some(handle) = rfd::AsyncFileDialog::new()
                    .set_title("Choose a Prism, MultiMC or Modrinth App folder")
                    .pick_folder()
                    .await
                else {
                    return;
                };
                let root = handle.path().to_path_buf();
                let found = off_ui(async move {
                    let detection = detect_external_folder(root).await?;
                    let already = detection.as_ref().is_some_and(|detection| {
                        listed
                            .iter()
                            .any(|root| same_external_folder(root, &detection.root))
                    });
                    Ok::<_, oneclient_core::LauncherError>((detection, already))
                })
                .await;
                match found {
                    Ok((Some(detection), true)) => dispatch
                        .notify("Already listed")
                        .body(format!(
                            "{} at that folder is already in the list.",
                            detection.launcher.display_name()
                        ))
                        .send(),
                    Ok((Some(detection), false)) if !detection.instances.is_empty() => {
                        extra.write().push(detection);
                    }
                    Ok(_) => dispatch
                        .notify("No instances found")
                        .body(
                            "That folder is not a Prism, MultiMC, PolyMC or Modrinth App install, or it has no instances.",
                        )
                        .error()
                        .send(),
                    Err(err) => dispatch
                        .notify("Could not read that folder")
                        .body(err.to_string())
                        .error()
                        .send(),
                }
            });
        })
        .child(Icon::new(IconType::Folder).size(14.))
        .text("Choose a folder")
        .into_element()
}

fn launcher_section(
    detection: &ExternalDetection,
    mut chosen: State<Vec<ExternalInstance>>,
) -> Element {
    let picked = chosen.read().clone();
    let all_picked = detection
        .instances
        .iter()
        .all(|instance| picked.iter().any(|c| c.game_dir == instance.game_dir));

    let cards: Vec<Element> = detection
        .instances
        .iter()
        .map(|instance| {
            let selected = picked.iter().any(|c| c.game_dir == instance.game_dir);
            let toggled = instance.clone();
            cell_card(CellCard {
                id: instance.game_dir.to_string_lossy().into_owned(),
                title: instance.name.clone(),
                blurb: version_line(instance),
                meta: meta_line(instance),
                corner: None,
                selected,
                checkbox: true,
                on_press: (move |()| {
                    let mut list = chosen.write();
                    match list.iter().position(|c| c.game_dir == toggled.game_dir) {
                        Some(index) => {
                            list.remove(index);
                        }
                        None => list.push(toggled.clone()),
                    }
                })
                .into(),
            })
        })
        .collect();

    let section_instances = detection.instances.clone();
    let toggle_all = label()
        .text(if all_picked { "Clear" } else { "Select all" })
        .font_size(12.)
        .color(colors::fg_secondary())
        .a11y_role(AccessibilityRole::Button)
        .cursor(CursorIcon::Pointer)
        .on_press(move |_| {
            let mut list = chosen.write();
            if all_picked {
                list.retain(|c| !section_instances.iter().any(|i| i.game_dir == c.game_dir));
            } else {
                for instance in &section_instances {
                    if !list.iter().any(|c| c.game_dir == instance.game_dir) {
                        list.push(instance.clone());
                    }
                }
            }
        });

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
                        .text(detection.launcher.display_name())
                        .font_size(13.)
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .width(Size::flex(1.0))
                        .text(detection.root.display().to_string())
                        .font_size(11.)
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis)
                        .color(colors::fg_secondary()),
                )
                .child(toggle_all),
        )
        .child(fixed_grid(cards, BUNDLE_COLUMNS, BUNDLE_CARD_H, GRID_GAP))
        .into_element()
}

fn version_line(instance: &ExternalInstance) -> String {
    match (instance.loader, instance.loader_version.as_deref()) {
        (GameLoader::Vanilla, _) => format!("{} · Vanilla", instance.mc_version),
        (loader, Some(version)) => format!("{} · {loader} {version}", instance.mc_version),
        (loader, None) => format!("{} · {loader}", instance.mc_version),
    }
}

fn meta_line(instance: &ExternalInstance) -> Option<String> {
    if instance.linked_pack.is_some() {
        return Some("Modpack, keeps getting updates".to_string());
    }
    if !instance.groups.is_empty() {
        return Some(instance.groups.join(", "));
    }
    let hours = instance.played_secs / 3600;
    (hours > 0).then(|| format!("{hours}h played"))
}
