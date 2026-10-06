use super::folder_list::use_game_folder_in_use;
use crate::components::{Button, Icon, IconType, OverlayPopup, checkbox_controlled};
use crate::hooks::{
    Actions, use_cluster, use_dispatch, use_installs_snapshot, use_settings_snapshot,
};
use crate::launcher::{self, off_ui};
use crate::theme::colors;
use crate::view::app::settings::{section_header, settings_row};
use freya::prelude::*;
use oneclient_common::domain::GameLoader;
use oneclient_core::export::{
    ExportFormat, ExportItem, ExportPreset, ExportSelection, export_cluster_with_options,
    list_cluster_export_items,
};
use oneclient_events::GroupedProgressSession;
use std::collections::{BTreeMap, BTreeSet};

#[derive(PartialEq)]
pub(super) struct ExportingSection {
    pub cluster_id: i64,
}
impl Component for ExportingSection {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let cluster = use_cluster(cluster_id);
        let in_use = use_game_folder_in_use(cluster.as_ref());
        let installs = use_installs_snapshot();
        let mut popup = use_state(|| false);
        let blocked = in_use || installs.cluster_busy(cluster_id) || cluster.is_none();
        let prism = cluster
            .as_ref()
            .is_some_and(|c| c.mc_loader == GameLoader::Ornithe && c.mc_version == "1.8.9");
        let actions = use_dispatch();
        rect()
            .vertical()
            .width(Size::fill())
            .spacing(4.)
            .child(section_header("EXPORTING"))
            .child(settings_row(
                IconType::FolderDownload,
                "Export mods.zip",
                "Save this instance's enabled mods in a portable ZIP.",
                Button::new()
                    .small()
                    .secondary()
                    .enabled(!blocked)
                    .on_press(move |_| {
                        start_export(cluster_id, ExportFormat::Mods, None, actions.clone())
                    })
                    .text("Export"),
            ))
            .maybe_child(prism.then(|| {
                settings_row(
                    IconType::FolderDownload,
                    "Export prism instance",
                    "Choose mods, configs and other files for an Ornithe 1.8.9 Prism instance.",
                    Button::new()
                        .small()
                        .secondary()
                        .enabled(!blocked)
                        .on_press(move |_| popup.set(true))
                        .text("Choose files…"),
                )
                .into_element()
            }))
            .maybe_child((*popup.read()).then(|| {
                PrismExportDialog {
                    cluster_id,
                    open: popup,
                }
                .into_element()
            }))
    }
}
#[derive(PartialEq)]
struct PrismExportDialog {
    cluster_id: i64,
    open: State<bool>,
}
impl Component for PrismExportDialog {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let mut open = self.open;
        let saved = use_settings_snapshot()
            .settings
            .export_selections
            .get(&cluster_id)
            .cloned();
        let mut selection = use_state(|| saved.clone().unwrap_or_default());
        let items = use_state(BTreeMap::<String, Vec<ExportItem>>::new);
        let expanded = use_state(BTreeSet::<String>::new);
        let mut error = use_state(|| None::<String>);
        let mut loaded = use_state(|| false);
        use_hook(move || {
            spawn(async move {
                let result = load_items(cluster_id, String::new()).await;
                let mut items = items;
                match result {
                    Ok(children) => {
                        if saved.is_none() {
                            selection.set(ExportSelection::initial(&children));
                        }
                        items.write().insert(String::new(), children);
                        loaded.set(true);
                    }
                    Err(err) => error.set(Some(format!("{err:#}"))),
                }
            });
        });
        let actions = use_dispatch();
        let mut rows = rect().vertical().width(Size::fill()).spacing(2.);
        for row in tree_rows(cluster_id, "", 0, items, expanded, selection, error) {
            rows = rows.child(row);
        }
        let roots = items.read().get("").cloned().unwrap_or_default();
        let mut presets = rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .spacing(8.);
        for (preset, title) in [
            (ExportPreset::All, "Choose all"),
            (
                ExportPreset::Personal,
                "Choose recommended for personal use",
            ),
            (
                ExportPreset::Distribution,
                "Choose recommended for distribution",
            ),
        ] {
            let recommended = ExportSelection::preset(preset, &roots);
            let active = *loaded.read() && selection.read().rules == recommended.rules;
            presets = presets.child(
                Button::new()
                    .secondary()
                    .width(Size::flex(1.))
                    .height(Size::px(96.))
                    .enabled(*loaded.read())
                    .on_press(move |_| {
                        let open_in_prism = selection.peek().open_in_prism;
                        selection.set(ExportSelection {
                            open_in_prism,
                            ..recommended.clone()
                        });
                    })
                    .child(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .spacing(6.)
                            .cross_align(Alignment::Center)
                            .child(
                                Icon::new(if active {
                                    IconType::CheckCircle
                                } else {
                                    IconType::Square
                                })
                                .size(18.)
                                .color(if active {
                                    colors::brand()
                                } else {
                                    colors::fg_secondary()
                                }),
                            )
                            .child(
                                label()
                                    .text(title)
                                    .font_size(11.)
                                    .max_lines(3)
                                    .width(Size::fill())
                                    .text_align(TextAlign::Center),
                            ),
                    ),
            );
        }
        OverlayPopup::new().on_close(move |()| open.set(false)).child(
            rect().width(Size::window_percent(100.)).height(Size::window_percent(100.)).center().child(
                rect().vertical().width(Size::px(620.)).max_width(Size::window_percent(90.)).height(Size::window_percent(90.)).content(Content::Flex)
                    .padding(Gaps::new_all(24.)).spacing(16.).corner_radius(CornerRadius::new_all(16.)).background(colors::page_elevated())
                    .child(label().text("Export Prism instance").font_size(20.).font_weight(FontWeight::SEMI_BOLD))
                    .child(label().text("Select files and folders to include. Folder selections include their contents; expand a folder to exclude individual files.").font_size(12.).color(colors::fg_secondary()))
                    .child(presets)
                    .child(ScrollView::new().width(Size::fill()).height(Size::flex(1.)).child(rows))
                    .maybe_child(error.read().clone().map(|error| label().text(error).font_size(12.).color(colors::danger()).into_element()))
                    .child(checkbox_controlled(selection.read().open_in_prism, "Open ZIP in Prism after export", move |()| {
                        let value = selection.peek().open_in_prism;
                        selection.write().open_in_prism = !value;
                    }))
                    .child(rect().horizontal().spacing(12.).main_align(Alignment::End)
                        .child(Button::new().ghost().on_press(move |_| open.set(false)).text("Cancel"))
                        .child(Button::new().primary().enabled(*loaded.read()).on_press(move |_| {
                            let selected = selection.peek().clone();
                            open.set(false);
                            start_export(cluster_id, ExportFormat::Prism, Some(selected), actions.clone());
                        }).child(Icon::new(IconType::FolderDownload).size(18.)).text("Export")))
            )
        )
    }
}

fn tree_rows(
    cluster_id: i64,
    parent: &str,
    depth: usize,
    items: State<BTreeMap<String, Vec<ExportItem>>>,
    mut expanded: State<BTreeSet<String>>,
    mut selection: State<ExportSelection>,
    mut error: State<Option<String>>,
) -> Vec<Element> {
    // Do not retain a state reader while constructing callbacks/child components.
    let children = items.read().get(parent).cloned().unwrap_or_default();
    let mut rows = Vec::new();
    for item in children {
        let path = item.path.clone();
        let checked = selection.read().selected(&path);
        let is_expanded = expanded.read().contains(&path);
        let mut row = rect()
            .horizontal()
            .width(Size::fill())
            .cross_align(Alignment::Center)
            .spacing(6.)
            .padding(Gaps::new(2., 0., 2., depth as f32 * 18.));
        if item.folder {
            let expand_path = path.clone();
            row = row.child(
                Button::new()
                    .small()
                    .ghost()
                    .on_press(move |_| {
                        if expanded.peek().contains(&expand_path) {
                            expanded.write().remove(&expand_path);
                            return;
                        }
                        expanded.write().insert(expand_path.clone());
                        if items.peek().contains_key(&expand_path) {
                            return;
                        }
                        let path = expand_path.clone();
                        spawn(async move {
                            let mut items = items;
                            match load_items(cluster_id, path.clone()).await {
                                Ok(children) => {
                                    items.write().insert(path, children);
                                }
                                Err(err) => error.set(Some(format!("{err:#}"))),
                            }
                        });
                    })
                    .child(
                        Icon::new(if is_expanded {
                            IconType::ChevronDown
                        } else {
                            IconType::ChevronRight
                        })
                        .size(14.),
                    ),
            );
        } else {
            row = row.child(rect().width(Size::px(30.)));
        }
        let row_key = path.clone();
        row = row.child(checkbox_controlled(checked, item.name, move |()| {
            let checked = selection.peek().selected(&path);
            selection.write().set(path.clone(), !checked)
        }));
        if !item.folder {
            row = row.child(
                label()
                    .text(format!("{} KiB", item.bytes.div_ceil(1024)))
                    .font_size(10.)
                    .color(colors::fg_secondary()),
            );
        }
        rows.push(row.key(row_key).into_element());
        if item.folder && is_expanded {
            rows.extend(tree_rows(
                cluster_id,
                &item.path,
                depth + 1,
                items,
                expanded,
                selection,
                error,
            ));
        }
    }
    rows
}
async fn load_items(cluster_id: i64, parent: String) -> anyhow::Result<Vec<ExportItem>> {
    off_ui(async move {
        let state = launcher::state()?;
        let cluster = state.clusters.get(cluster_id).await?;
        list_cluster_export_items(&state, &cluster, parent).await
    })
    .await
}
fn export_name(name: &str, version: &str, format: ExportFormat) -> String {
    let clean = |value: &str| {
        polyio::sanitize_path(&value.replace(['/', '\\'], "_"))
            .to_string_lossy()
            .into_owned()
    };
    format!(
        "{}-{}{}.zip",
        clean(name),
        clean(version),
        if format == ExportFormat::Mods {
            "-mods"
        } else {
            ""
        }
    )
}
fn start_export(
    cluster_id: i64,
    format: ExportFormat,
    selection: Option<ExportSelection>,
    actions: Actions,
) {
    if !actions.begin_export(cluster_id) {
        return;
    }
    spawn_forever(async move {
        let result: anyhow::Result<Option<oneclient_core::export::ExportReport>> = async {
            let state = launcher::state()?;
            let cluster = off_ui({
                let state = state.clone();
                async move { state.clusters.get(cluster_id).await }
            })
            .await?;
            let file_name = export_name(&cluster.name, &cluster.mc_version, format);
            let Some(handle) = rfd::AsyncFileDialog::new()
                .set_title("Export instance ZIP")
                .add_filter("ZIP archive", &["zip"])
                .set_file_name(&file_name)
                .save_file()
                .await
            else {
                return Ok(None);
            };
            let mut destination = handle.path().to_path_buf();
            if destination.extension().is_none() {
                destination.set_extension("zip");
            }
            let progress = GroupedProgressSession::start(
                &state.services.events,
                format!("Exporting {}", cluster.name),
            );
            let selected = selection.clone();
            off_ui(async move {
                export_cluster_with_options(
                    &state,
                    &cluster,
                    format,
                    &destination,
                    selected,
                    Some(progress),
                )
                .await
                .map(Some)
            })
            .await
        }
        .await;
        actions.set_exporting(cluster_id, false);
        match result {
            Ok(Some(report)) => {
                if let Some(selection) = selection {
                    let open = selection.open_in_prism;
                    actions.edit_settings(move |settings| {
                        settings.export_selections.insert(cluster_id, selection);
                    });
                    if open {
                        if let Err(err) = off_ui({
                            let path = report.path.clone();
                            async move { open_in_prism(&path).await }
                        })
                        .await
                        {
                            actions
                                .notify("ZIP saved; couldn't open Prism")
                                .body(format!(
                                    "{err}. Import {} manually in Prism.",
                                    report.path.display()
                                ))
                                .error()
                                .send();
                        }
                    }
                }
                actions
                    .notify("Export created")
                    .body(format!(
                        "Saved {} files ({} mods) to {}",
                        report.files,
                        report.mods,
                        report.path.display()
                    ))
                    .icon(IconType::FolderDownload)
                    .send();
            }
            Ok(None) => {}
            Err(err) => {
                actions
                    .notify("Couldn't export this instance")
                    .body(format!("{err:#}"))
                    .error()
                    .icon(IconType::FolderDownload)
                    .send();
            }
        }
    });
}
async fn open_in_prism(path: &std::path::Path) -> std::io::Result<()> {
    let mut candidates = Vec::<std::path::PathBuf>::new();
    #[cfg(target_os = "macos")]
    {
        candidates.push("/Applications/Prism Launcher.app/Contents/MacOS/prismlauncher".into());
        if let Some(home) = std::env::var_os("HOME") {
            candidates.push(
                std::path::PathBuf::from(home)
                    .join("Applications/Prism Launcher.app/Contents/MacOS/prismlauncher"),
            );
        }
    }
    #[cfg(target_os = "windows")]
    {
        for variable in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
            if let Some(root) = std::env::var_os(variable) {
                for folder in [
                    "PrismLauncher",
                    "Prism Launcher",
                    "Programs/PrismLauncher",
                    "Programs/Prism Launcher",
                ] {
                    candidates.push(
                        std::path::PathBuf::from(&root)
                            .join(folder)
                            .join("prismlauncher.exe"),
                    );
                }
            }
        }
        candidates.push("prismlauncher.exe".into());
    }
    #[cfg(not(target_os = "windows"))]
    candidates.push("prismlauncher".into());
    for candidate in candidates {
        match tokio::process::Command::new(candidate)
            .arg("--import")
            .arg(path)
            .spawn()
        {
            Ok(mut child) => {
                // Reap the child without keeping the export task alive for Prism's lifetime.
                tokio::spawn(async move {
                    let _ = child.wait().await;
                });
                return Ok(());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "Prism Launcher was not found in its usual installation folders or PATH",
    ))
}
