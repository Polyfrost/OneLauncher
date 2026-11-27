use std::path::PathBuf;
use std::sync::Arc;

use freya::prelude::*;
use freya::query::MutationStateData;
use freya::router::RouterContext;
use oneclient_common::Patch;
use oneclient_common::domain::{GameLoader, GraphicsBackend};
use oneclient_core::settings::{GameSettingsProfile, PackageUpdateMode, ProfileUpdate, Resolution};
use oneclient_core::sync::SyncStatus;
use oneclient_java::JavaRuntime;

use crate::components::{
    Button, ContextMenu, Dropdown, Icon, IconType, ScrollArea, Segment, SegmentedControl,
    SwitchSide, TextInput, kebab_button, memory_field, pill_switch, settings_file_picker, toggle,
    toggle_controlled, validate_number,
};
use crate::hooks::{
    ClusterAction, available_bundles, java_runtimes, load_is_shared, loader_versions,
    mutation_is_running, query_error, settled_or_loading, try_game_profile, use_available_bundles,
    use_cloud_sync, use_cluster_mutation, use_clusters, use_dispatch, use_game_profile,
    use_game_snapshot, use_graphics_backend, use_installs_snapshot, use_is_migration_source,
    use_java_runtimes, use_launcher, use_loader_versions, use_migratable_routes,
    use_release_migration_checking, use_settings_snapshot, use_shared_groups,
};
use crate::i18n::{t, tf};
use crate::layout::{ClusterShell, cluster_content};
use crate::routes::Route;
use crate::state::MigrationDirection;
use crate::theme::{self, colors, rounded, text_size};
use crate::ui::{border_all_color, centered_note};
use crate::view::app::clusters::{
    CreateInstanceModal, DeleteInstanceModal, EditInstanceModal, ExportInstanceModal,
    InstanceFacts, ResetInstanceModal,
};
use crate::view::app::launch_syncing;
use crate::view::app::settings::{
    PAGE_GAP, SettingsControl, first_section_header, section_header, settings_control_width,
    settings_pane, settings_row, settings_row_danger, settings_row_disabled, title_reset,
};
use oneclient_core::clusters::{DirTarget, rank_migration_sources, rank_migration_targets};

use super::cluster_not_found;
use super::imported_modpacks::ImportedModpacksSection;
use super::modpack_settings::{ModpackRepairRow, ModpackUpdateRow};
use crate::hooks::use_cluster;

#[path = "settings_file_dialogs.rs"]
mod file_dialogs;
use file_dialogs::{GameOptionsRow, SyncFilesModal};

/// The category tabs, and the space between them and the first section label
const TAB_STRIP_HEIGHT: f32 = 34.;
const TAB_STRIP_GAP: f32 = 16.;

#[derive(PartialEq, Clone, Copy)]
enum SettingsCategory {
    Instance,
    Game,
    Advanced,
}

impl SettingsCategory {
    fn label(self) -> &'static str {
        match self {
            Self::Instance => t("oneclient.cluster.settings.category.instance"),
            Self::Game => t("oneclient.cluster.settings.category.game"),
            Self::Advanced => t("oneclient.cluster.settings.category.advanced"),
        }
    }

    fn icon(self) -> IconType {
        match self {
            Self::Instance => IconType::Cube01,
            Self::Game => IconType::Sliders04,
            Self::Advanced => IconType::Settings01,
        }
    }
}

#[derive(PartialEq)]
pub struct ClusterSettings {
    pub cluster_id: i64,
}

impl Component for ClusterSettings {
    fn render(&self) -> impl IntoElement {
        let category = use_state(|| SettingsCategory::Instance);
        let active = category();
        let cluster_id = self.cluster_id;
        let global = use_settings_snapshot().settings.global_game_settings;

        let cluster = use_cluster(cluster_id);
        let profile_name = cluster
            .as_ref()
            .and_then(|c| c.setting_profile_name.clone());
        let profile_query = use_game_profile(profile_name);
        let mc_version = cluster
            .as_ref()
            .map(|c| c.mc_version.clone())
            .unwrap_or_default();
        let loader = cluster
            .as_ref()
            .map(|c| c.mc_loader)
            .unwrap_or(GameLoader::Fabric);
        let versions_query = use_loader_versions(mc_version, loader);
        let runtimes_query = use_java_runtimes();
        // The profile rows copy their value into local state when they mount, so a
        // reset bumps this to remount them with the profile it put back
        let mut resets = use_state(|| 0u32);
        let generation = *resets.read();
        let reset = use_cluster_mutation();
        let reset_running = mutation_is_running(&reset);
        let settings_scope = current_scope_id();
        let on_reset = move |()| {
            let (done_tx, done_rx) = tokio::sync::oneshot::channel();
            // Let the mutation finish if the user leaves settings, but only
            // remount controls while this page still owns their generation.
            spawn_in_window(async move {
                reset
                    .mutate_async(ClusterAction::ResetInstance { cluster_id })
                    .await;
                let _ = done_tx.send(());
            });
            spawn_in_scope(
                async move {
                    if done_rx.await.is_ok() {
                        *resets.write() += 1;
                    }
                },
                settings_scope,
            );
        };

        let Some(cluster) = cluster else {
            return cluster_not_found();
        };

        let Some(profile) = try_game_profile(&profile_query) else {
            let note = match query_error(&profile_query) {
                Some(err) => tf("oneclient.cluster.settings.load_error", &[&err]),
                None => t("oneclient.cluster.settings.loading").to_string(),
            };
            return cluster_content().child(centered_note(&note)).into_element();
        };

        let versions = loader_versions(&versions_query);
        let runtimes = java_runtimes(&runtimes_query);

        let instance_row = {
            InstanceRow {
                editable: cluster.user_created,
                facts: InstanceFacts {
                    cluster_id,
                    name: crate::utils::instance_display_name(&cluster),
                    description: cluster.description.clone(),
                    tags: cluster.tags.clone(),
                    cover: cluster.cover_file(),
                    mc_version: cluster.mc_version.clone(),
                    mc_loader: cluster.mc_loader,
                    kind: cluster.kind,
                    modpack: cluster.linked_modpack_hash.is_some(),
                },
            }
            .into_element()
        };

        let mod_loader = !cluster.lacks_mod_loader();
        let loader_section: Vec<Element> = if mod_loader {
            vec![
                section_header(t("oneclient.cluster.settings.section.loader")).into_element(),
                LoaderRow {
                    cluster_id,
                    loader,
                    selected: cluster.mc_loader_version.clone(),
                    versions,
                }
                .into_element(),
            ]
        } else {
            Vec::new()
        };
        let graphics_backend_row: Vec<Element> = match cluster.game_dir() {
            Ok(game_dir) if GraphicsBackend::supported_on(&cluster.mc_version) => {
                vec![
                    GraphicsBackendRow {
                        cluster_id,
                        game_dir,
                    }
                    .into_element(),
                ]
            }
            _ => Vec::new(),
        };
        let game_options_row: Vec<Element> = match cluster.game_dir() {
            Ok(game_dir) => vec![
                GameOptionsRow {
                    cluster_id,
                    game_dir,
                }
                .into_element(),
            ],
            Err(_) => Vec::new(),
        };

        let migrate_row: Vec<Element> = if mod_loader {
            vec![MigrateFromRow { cluster_id }.into_element()]
        } else {
            Vec::new()
        };
        let copy_mod_files_row: Vec<Element> = if mod_loader {
            vec![profile_row(
                generation,
                "CopyModFilesRow",
                ToggleRow {
                    field: ToggleField::CopyModFiles,
                    cluster_id,
                    value: profile.copy_mod_files,
                    global: global.copy_mod_files.unwrap_or(false),
                }
                .into_element(),
            )]
        } else {
            Vec::new()
        };
        let take_bundles_row: Vec<Element> = if mod_loader && !cluster.uses_bundles() {
            vec![
                TakeBundlesRow {
                    cluster_id,
                    name: cluster.name.clone(),
                    mc_version: cluster.mc_version.clone(),
                    loader: cluster.mc_loader,
                }
                .into_element(),
            ]
        } else {
            Vec::new()
        };
        let problem_mod_row: Vec<Element> = if mod_loader {
            vec![super::FindProblemModRow { cluster_id }.into_element()]
        } else {
            Vec::new()
        };

        let sync_row: Vec<Element> = if cluster.is_isolated() {
            Vec::new()
        } else {
            vec![
                SyncSharedRow {
                    cluster_id,
                    target: cluster.dir_target.clone(),
                }
                .into_element(),
            ]
        };

        let modpack_section: Vec<Element> = if cluster.linked_modpack_hash.is_some() {
            vec![
                section_header(t("oneclient.cluster.settings.section.modpack")).into_element(),
                ModpackUpdateRow { cluster_id }.into_element(),
                ModpackRepairRow { cluster_id }.into_element(),
            ]
        } else {
            Vec::new()
        };
        let imported_section = ImportedModpacksSection { cluster_id }.into_element();

        let padding = ClusterShell::workspace_padding();
        let page = settings_pane()
            .reset_key(active as u64)
            .spacing(PAGE_GAP)
            .padding(Gaps::new(0., padding, 0., 0.));

        let page = match active {
            SettingsCategory::Instance => page
                .child(first_section_header(t(
                    "oneclient.cluster.settings.section.instance",
                )))
                .append_children(cluster.user_created.then(|| instance_row.clone()))
                .child(
                    CheckUpdatesRow {
                        cluster_id,
                        uses_bundles: cluster.uses_bundles(),
                    }
                    .into_element(),
                )
                .child(
                    DirTargetRow {
                        cluster_id,
                        target: cluster.dir_target.clone(),
                        locked: cluster.is_isolated(),
                    }
                    .into_element(),
                )
                .append_children(sync_row)
                .child(section_header(t(
                    "oneclient.cluster.settings.section.sharing",
                )))
                .child(
                    CloudSyncRow {
                        cluster_id,
                        name: cluster.name.clone(),
                    }
                    .into_element(),
                )
                .child(
                    ExportRow {
                        cluster_id,
                        name: cluster.name.clone(),
                    }
                    .into_element(),
                )
                .child(section_header(t(
                    "oneclient.cluster.settings.section.content",
                )))
                .child(profile_row(
                    generation,
                    "BrowserUpdateModeRow",
                    BrowserUpdateModeRow {
                        cluster_id,
                        value: profile.browser_update_mode,
                        global: global.browser_update_mode.unwrap_or_default(),
                    }
                    .into_element(),
                ))
                .append_children(take_bundles_row)
                .append_children(copy_mod_files_row)
                .append_children(migrate_row)
                .child(section_header(t(
                    "oneclient.cluster.settings.section.shortcut",
                )))
                .child(ShortcutRow { cluster_id }.into_element()),
            SettingsCategory::Game => page
                .child(first_section_header(t(
                    "oneclient.cluster.settings.section.game",
                )))
                .child(profile_row(
                    generation,
                    "ToggleRow",
                    ToggleRow {
                        field: ToggleField::Fullscreen,
                        cluster_id,
                        value: profile.force_fullscreen,
                        global: global.force_fullscreen.unwrap_or(false),
                    }
                    .into_element(),
                ))
                .child(profile_row(
                    generation,
                    "ResolutionRow",
                    ResolutionRow {
                        cluster_id,
                        value: profile.resolution,
                        global: global.resolution.unwrap_or_default(),
                    }
                    .into_element(),
                ))
                .child(profile_row(
                    generation,
                    "MemoryRow",
                    MemoryRow {
                        cluster_id,
                        value: profile.mem_max,
                        global: global
                            .mem_max
                            .unwrap_or_else(oneclient_common::default_mem_max),
                    }
                    .into_element(),
                ))
                .append_children(graphics_backend_row)
                .append_children(game_options_row)
                .append_children(loader_section)
                .child(section_header(t("oneclient.cluster.settings.section.java")))
                .child(profile_row(
                    generation,
                    "JavaRow",
                    JavaRow {
                        cluster_id,
                        value: profile.java_path.clone(),
                        global: global.java_path.clone(),
                        runtimes,
                    }
                    .into_element(),
                ))
                .child(profile_row(
                    generation,
                    "JvmArgs",
                    text_row(cluster_id, TextField::JvmArgs, &profile, &global),
                ))
                .child(profile_row(
                    generation,
                    "GameArgs",
                    text_row(cluster_id, TextField::GameArgs, &profile, &global),
                )),
            SettingsCategory::Advanced => page
                .child(first_section_header(t(
                    "oneclient.cluster.settings.section.process",
                )))
                .child(profile_row(
                    generation,
                    "Pre",
                    text_row(cluster_id, TextField::Pre, &profile, &global),
                ))
                .child(profile_row(
                    generation,
                    "Wrapper",
                    text_row(cluster_id, TextField::Wrapper, &profile, &global),
                ))
                .child(profile_row(
                    generation,
                    "Post",
                    text_row(cluster_id, TextField::Post, &profile, &global),
                ))
                .append_children(modpack_section)
                .child(imported_section)
                .child(section_header(t(
                    "oneclient.cluster.settings.section.repair",
                )))
                .child(VerifyFilesRow { cluster_id }.into_element())
                .append_children(problem_mod_row)
                .append_children((!cluster.user_created).then_some(instance_row))
                .child(
                    ResetInstanceRow {
                        cluster_id,
                        name: cluster.name.clone(),
                        running: reset_running,
                        on_reset: on_reset.into(),
                    }
                    .into_element(),
                ),
        };

        cluster_content()
            .content(Content::Flex)
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .height(Size::px(TAB_STRIP_HEIGHT + TAB_STRIP_GAP))
                    .padding(Gaps::new(0., padding, 0., 0.))
                    .cross_align(Alignment::Start)
                    .child(
                        SegmentedControl::new(category)
                            .height(TAB_STRIP_HEIGHT)
                            .icon_size(16.)
                            .segments(
                                [
                                    SettingsCategory::Instance,
                                    SettingsCategory::Game,
                                    SettingsCategory::Advanced,
                                ]
                                .map(|item| {
                                    Segment::new(item).icon(item.icon()).label(item.label())
                                }),
                            ),
                    ),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.))
                    .child(page),
            )
            .into_element()
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Field {
    ForceFullscreen,
    Resolution,
    MemMax,
    JavaPath,
    BrowserUpdateMode,
    CopyModFiles,
}

fn profile_row(generation: u32, slot: &'static str, row: impl IntoElement) -> Element {
    rect()
        .key((generation, slot))
        .width(Size::fill())
        .child(row)
        .into_element()
}

fn clear_update(field: Field) -> ProfileUpdate {
    let mut u = ProfileUpdate::default();
    match field {
        Field::ForceFullscreen => u.force_fullscreen = Patch::Clear,
        Field::Resolution => u.resolution = Patch::Clear,
        Field::MemMax => u.mem_max = Patch::Clear,
        Field::JavaPath => u.java_path = Patch::Clear,
        Field::BrowserUpdateMode => u.browser_update_mode = Patch::Clear,
        Field::CopyModFiles => u.copy_mod_files = Patch::Clear,
    }
    u
}

fn runtime_label(runtime: &JavaRuntime) -> String {
    format!("{} {} ({})", runtime.vendor, runtime.major, runtime.version)
}

fn override_cell(
    control: impl IntoElement,
    overridden: bool,
    on_reset: EventHandler<()>,
) -> SettingsControl {
    SettingsControl::with_reset(
        control,
        overridden.then(|| title_reset(t("oneclient.cluster.settings.override.reset"), on_reset)),
    )
}

#[derive(Clone, Copy, PartialEq)]
enum ToggleField {
    Fullscreen,
    CopyModFiles,
}

#[derive(PartialEq)]
struct ToggleRow {
    field: ToggleField,
    cluster_id: i64,
    value: Option<bool>,
    global: bool,
}

impl Component for ToggleRow {
    fn render(&self) -> impl IntoElement {
        let field = self.field;
        let cluster_id = self.cluster_id;
        let overridden = self.value.is_some();
        let global = self.global;
        let dispatch = use_dispatch();

        let initial = self.value.unwrap_or(self.global);
        let mut state = use_state(move || initial);
        let mut last = use_state(move || initial);
        {
            let dispatch = dispatch.clone();
            use_side_effect(move || {
                let v = *state.read();
                if v == *last.peek() {
                    return;
                }
                last.set(v);
                let mut update = ProfileUpdate::default();
                match field {
                    ToggleField::Fullscreen => update.force_fullscreen = Patch::Set(v),
                    ToggleField::CopyModFiles => update.copy_mod_files = Patch::Set(v),
                }
                dispatch.update_cluster_profile(cluster_id, update);
            });
        }

        let on_reset: EventHandler<()> = (move |()| {
            last.set(global);
            state.set(global);
            dispatch.update_cluster_profile(
                cluster_id,
                clear_update(match field {
                    ToggleField::Fullscreen => Field::ForceFullscreen,
                    ToggleField::CopyModFiles => Field::CopyModFiles,
                }),
            );
        })
        .into();

        let (icon, title, description) = match field {
            ToggleField::Fullscreen => (
                IconType::Maximize01,
                "oneclient.cluster.settings.force_fullscreen.title",
                "oneclient.cluster.settings.force_fullscreen.description",
            ),
            ToggleField::CopyModFiles => (
                IconType::Copy01,
                "oneclient.cluster.settings.copy_mod_files.title",
                "oneclient.cluster.settings.copy_mod_files.description",
            ),
        };
        settings_row(
            icon,
            t(title),
            t(description),
            override_cell(toggle(state), overridden, on_reset),
        )
    }
}

#[derive(PartialEq)]
struct InstanceRow {
    editable: bool,
    facts: InstanceFacts,
}

impl Component for InstanceRow {
    fn render(&self) -> impl IntoElement {
        let mut editing = use_state(|| false);
        let mut deleting = use_state(|| false);
        let cluster_id = self.facts.cluster_id;
        let name = self.facts.name.clone();
        let facts = self.facts.clone();

        let editable = self.editable;
        let description = if editable {
            t("oneclient.cluster.settings.instance.description.editable")
        } else {
            t("oneclient.cluster.settings.instance.description")
        };

        let buttons = rect()
            .horizontal()
            .spacing(8.)
            .maybe_child(editable.then(|| {
                Button::new()
                    .small()
                    .secondary()
                    .on_press(move |_| editing.set(true))
                    .text(t("oneclient.common.edit"))
                    .into_element()
            }))
            .child(
                Button::new()
                    .small()
                    .danger()
                    .on_press(move |_| deleting.set(true))
                    .text(t("oneclient.common.delete")),
            );

        rect()
            .vertical()
            .width(Size::fill())
            .child(settings_row(
                if editable {
                    IconType::Pencil01
                } else {
                    IconType::Trash01
                },
                if editable {
                    t("oneclient.cluster.settings.instance.title")
                } else {
                    t("oneclient.cluster.settings.instance.delete_title")
                },
                description,
                buttons,
            ))
            .maybe_child(editing.read().then(|| {
                EditInstanceModal::new(facts.clone(), move |()| editing.set(false)).into_element()
            }))
            .maybe_child(deleting.read().then(|| {
                DeleteInstanceModal::new(cluster_id, name.clone(), move |()| deleting.set(false))
                    .into_element()
            }))
    }
}

#[derive(PartialEq)]
struct CheckUpdatesRow {
    cluster_id: i64,
    uses_bundles: bool,
}

impl Component for CheckUpdatesRow {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let dispatch = use_dispatch();
        let game = use_game_snapshot();
        let checking = game.is_checking_updates(cluster_id);
        let syncing = launch_syncing(&use_launcher(), self.uses_bundles);
        let other_checking = !checking && game.any_update_check();
        let busy = checking
            || other_checking
            || syncing
            || game.is_active(cluster_id)
            || game.is_launch_pending(cluster_id);

        let button = Button::new()
            .small()
            .secondary()
            .enabled(!busy)
            .maybe(!busy, |el| {
                el.on_press(move |_| dispatch.check_cluster_updates(cluster_id))
            })
            .maybe(game.is_active(cluster_id), |el| {
                el.tooltip(t("oneclient.cluster.settings.check_updates.game_running"))
            })
            .maybe(other_checking, |el| {
                el.tooltip(t("oneclient.cluster.settings.check_updates.other_running"))
            })
            .text(if checking {
                t("oneclient.cluster.settings.check_updates.running")
            } else {
                t("oneclient.cluster.settings.check_updates.button")
            });

        settings_row(
            IconType::RefreshCw01,
            t("oneclient.cluster.settings.check_updates.title"),
            t("oneclient.cluster.settings.check_updates.description"),
            button,
        )
    }
}

#[derive(PartialEq)]
struct ShortcutRow {
    cluster_id: i64,
}

impl Component for ShortcutRow {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let dispatch = use_dispatch();

        let button = Button::new()
            .small()
            .secondary()
            .on_press(move |_| dispatch.create_cluster_shortcut(cluster_id))
            .text(t("oneclient.cluster.settings.shortcut.create"));

        settings_row(
            IconType::Rocket02,
            t("oneclient.cluster.settings.shortcut.title"),
            t("oneclient.cluster.settings.shortcut.description"),
            button,
        )
    }
}

#[derive(PartialEq)]
struct ExportRow {
    cluster_id: i64,
    name: String,
}

impl Component for ExportRow {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let mut choosing = use_state(|| false);
        let mut shared = use_state(|| false);
        use_side_effect(move || {
            if *choosing.read() {
                return;
            }
            spawn(async move { shared.set(load_is_shared(cluster_id).await) });
        });

        let button = Button::new()
            .small()
            .secondary()
            .on_press(move |_| choosing.set(true))
            .text(if *shared.read() {
                t("oneclient.cluster.settings.export.configure")
            } else {
                t("oneclient.cluster.settings.export.share")
            });

        rect()
            .vertical()
            .width(Size::fill())
            .child(settings_row(
                IconType::Users01,
                t("oneclient.cluster.settings.export.title"),
                t("oneclient.cluster.settings.export.description"),
                button,
            ))
            .maybe_child(choosing.read().then(|| {
                ExportInstanceModal::new(cluster_id, self.name.clone(), move |()| {
                    choosing.set(false)
                })
                .into_element()
            }))
    }
}

#[derive(PartialEq)]
struct VerifyFilesRow {
    cluster_id: i64,
}

impl Component for VerifyFilesRow {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let mutation = use_cluster_mutation();
        let running = mutation_is_running(&mutation);
        let checking = use_game_snapshot().is_checking_updates(cluster_id);

        let on_press = move |_| {
            mutation.mutate(ClusterAction::VerifyFiles { cluster_id });
        };

        // The pass reads every installed file so say it takes a while otherwise the button reads as broken
        let button = Button::new()
            .small()
            .secondary()
            .enabled(!running && !checking)
            .maybe(!running && !checking, |el| el.on_press(on_press))
            .maybe(checking, |el| {
                el.tooltip(t("oneclient.cluster.settings.verify.update_check_running"))
            })
            .text(if running {
                t("oneclient.cluster.settings.verify.running")
            } else {
                t("oneclient.cluster.settings.verify.button")
            });

        settings_row(
            IconType::ClipboardCheck,
            t("oneclient.cluster.settings.verify.title"),
            t("oneclient.cluster.settings.verify.description"),
            button,
        )
    }
}

#[derive(PartialEq)]
struct TakeBundlesRow {
    cluster_id: i64,
    name: String,
    mc_version: String,
    loader: GameLoader,
}

impl Component for TakeBundlesRow {
    fn render(&self) -> impl IntoElement {
        let (cluster_id, name) = (self.cluster_id, self.name.clone());
        let dispatch = use_dispatch();
        let active = use_game_snapshot().is_active(cluster_id);
        let busy = use_installs_snapshot().modpack_busy;
        let enabled = !active && !busy;
        let bundles = use_available_bundles(self.mc_version.clone(), self.loader);
        if available_bundles(&bundles).is_none_or(|bundles| bundles.is_empty()) {
            return rect().into_element();
        }

        let button = Button::new()
            .small()
            .secondary()
            .enabled(enabled)
            .maybe(enabled, |el| {
                el.on_press(move |_| dispatch.take_offered_bundles(cluster_id, name.clone()))
            })
            .text(t("oneclient.cluster.settings.take_bundles.button"));

        settings_row(
            IconType::LayersThree01,
            t("oneclient.cluster.settings.take_bundles.title"),
            t("oneclient.cluster.settings.take_bundles.description"),
            button,
        )
        .into_element()
    }
}

#[derive(PartialEq)]
struct ResetInstanceRow {
    cluster_id: i64,
    name: String,
    running: bool,
    on_reset: EventHandler<()>,
}

impl Component for ResetInstanceRow {
    fn render(&self) -> impl IntoElement {
        let mut confirming = use_state(|| false);
        let cluster_id = self.cluster_id;
        let name = self.name.clone();
        let running = self.running;
        let on_reset = self.on_reset.clone();

        let button = Button::new()
            .small()
            .danger()
            .enabled(!running)
            .maybe(!running, |el| el.on_press(move |_| confirming.set(true)))
            .text(t("oneclient.common.reset"));

        rect()
            .vertical()
            .width(Size::fill())
            .child(settings_row_danger(
                IconType::RefreshCw01,
                t("oneclient.cluster.settings.reset_instance.title"),
                t("oneclient.cluster.settings.reset_instance.description"),
                button,
            ))
            .maybe_child(confirming.read().then(|| {
                ResetInstanceModal::new(
                    cluster_id,
                    name.clone(),
                    move |()| confirming.set(false),
                    on_reset,
                )
                .into_element()
            }))
    }
}

#[derive(PartialEq)]
struct SyncSharedRow {
    cluster_id: i64,
    target: DirTarget,
}

impl Component for SyncSharedRow {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let dispatch = use_dispatch();
        let syncing = use_game_snapshot().is_syncing_shared(cluster_id);
        let groups = settled_or_loading(&use_shared_groups()).unwrap_or_default();
        let mut picked = use_state(|| None::<Option<String>>);

        let sources: Vec<Option<String>> = std::iter::once(None)
            .chain(groups.into_iter().map(Some))
            .filter(|source| self.target != DirTarget::Shared(source.clone()))
            .collect();
        let Some(first) = sources.first().cloned() else {
            return rect().into_element();
        };
        let from = picked
            .read()
            .clone()
            .filter(|source| sources.contains(source))
            .unwrap_or(first);
        let label = |source: &Option<String>| {
            source.clone().unwrap_or_else(|| {
                t("oneclient.cluster.settings.dedicated_dir.option.shared").to_string()
            })
        };

        let picker = Dropdown::new(label(&from), sources.iter().map(label).collect())
            .width(Size::px(160.))
            .height(Size::px(34.))
            .on_select({
                let sources = sources.clone();
                move |idx: usize| {
                    if let Some(source) = sources.get(idx) {
                        picked.set(Some(source.clone()));
                    }
                }
            });
        let button = Button::new()
            .small()
            .secondary()
            .enabled(!syncing)
            .maybe(!syncing, |el| {
                el.on_press(move |_| dispatch.sync_from_shared_dir(cluster_id, from.clone()))
            })
            .text(if syncing {
                t("oneclient.cluster.settings.sync.running")
            } else {
                t("oneclient.cluster.settings.sync.button")
            });

        settings_row(
            IconType::RefreshCcw02,
            t("oneclient.cluster.settings.sync.title"),
            t("oneclient.cluster.settings.sync.description"),
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(8.)
                .child(picker)
                .child(button),
        )
        .into_element()
    }
}

#[derive(PartialEq)]
struct DirTargetRow {
    cluster_id: i64,
    target: DirTarget,
    locked: bool,
}

impl Component for DirTargetRow {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let current = self.target.clone();
        let mutation = use_cluster_mutation();
        let dispatch = use_dispatch();
        let groups = settled_or_loading(&use_shared_groups());
        let missing = matches!(&self.target, DirTarget::Shared(Some(group))
            if groups.as_ref().is_some_and(|groups| !groups.contains(group)));
        let groups = groups.unwrap_or_default();
        let mut naming = use_state(|| false);
        let mut name = use_state(String::new);

        let dedicated = t("oneclient.cluster.settings.dedicated_dir.option.dedicated").to_string();
        let shared = t("oneclient.cluster.settings.dedicated_dir.option.shared").to_string();

        if self.locked {
            return settings_row_disabled(
                IconType::Folder,
                t("oneclient.cluster.settings.dedicated_dir.title"),
                t("oneclient.cluster.settings.dedicated_dir.locked"),
                Dropdown::new(dedicated.clone(), vec![dedicated])
                    .width(settings_control_width())
                    .height(Size::px(34.)),
            )
            .into_element();
        }

        let switch = move |target: DirTarget, create: bool| {
            if target == current && !create {
                return;
            }
            let action = ClusterAction::SetDirTarget {
                cluster_id,
                target: target.clone(),
                create,
            };
            let DirTarget::Shared(from) = current.clone() else {
                mutation.mutate(action);
                return;
            };
            if target != DirTarget::Dedicated || missing {
                mutation.mutate(action);
                return;
            }
            // The new folder starts empty, so offer to bring the shared worlds and settings along
            let dispatch = dispatch.clone();
            spawn_in_window(async move {
                let reader = mutation.mutate_async(action).await;
                if matches!(
                    &*reader.state(),
                    MutationStateData::Settled { res: Ok(_), .. }
                ) {
                    dispatch.sync_from_shared_dir(cluster_id, from);
                }
            });
        };

        let control = if *naming.read() {
            let mut submit = {
                let switch = switch.clone();
                move || {
                    let typed = name.peek().trim().to_string();
                    if typed.is_empty() {
                        return;
                    }
                    naming.set(false);
                    name.set(String::new());
                    switch(DirTarget::Shared(Some(typed)), true);
                }
            };
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(8.)
                .child(
                    TextInput::new(name)
                        .placeholder(t("oneclient.cluster.settings.dedicated_dir.folder_name"))
                        .compact()
                        .auto_focus(true)
                        .width(Size::px(160.))
                        .on_submit(move |_: String| submit()),
                )
                .child(
                    Button::new()
                        .small()
                        .secondary()
                        .on_press(move |_| naming.set(false))
                        .text(t("oneclient.common.cancel")),
                )
                .into_element()
        } else {
            let selected = match &self.target {
                DirTarget::Dedicated => dedicated.clone(),
                DirTarget::Shared(None) => shared.clone(),
                DirTarget::Shared(Some(group)) if missing => {
                    tf("oneclient.cluster.settings.dedicated_dir.missing", &[group])
                }
                DirTarget::Shared(Some(group)) => group.clone(),
            };
            let mut options = vec![dedicated, shared];
            options.extend(groups.iter().cloned());
            options.push(t("oneclient.cluster.settings.dedicated_dir.option.new").to_string());
            let create = options.len() - 1;
            Dropdown::new(selected, options)
                .width(settings_control_width())
                .height(Size::px(34.))
                .on_select(move |idx: usize| match idx {
                    0 => switch(DirTarget::Dedicated, false),
                    1 => switch(DirTarget::Shared(None), false),
                    idx if idx == create => naming.set(true),
                    idx => {
                        if let Some(group) = groups.get(idx - 2) {
                            switch(DirTarget::Shared(Some(group.clone())), false);
                        }
                    }
                })
                .into_element()
        };

        settings_row(
            IconType::Folder,
            t("oneclient.cluster.settings.dedicated_dir.title"),
            t("oneclient.cluster.settings.dedicated_dir.description"),
            control,
        )
        .into_element()
    }
}

#[derive(PartialEq)]
struct MemoryRow {
    cluster_id: i64,
    value: Option<u32>,
    global: u32,
}

impl Component for MemoryRow {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let overridden = self.value.is_some();
        let dispatch = use_dispatch();

        let global = self.global;
        let initial = self.value.map(|v| v.to_string()).unwrap_or_default();
        let mut memory = use_state({
            let v = initial.clone();
            move || v
        });
        let mut last = use_state(move || initial);
        {
            let dispatch = dispatch.clone();
            use_side_effect(move || {
                let raw = memory.read().clone();
                if raw == *last.peek() {
                    return;
                }
                last.set(raw.clone());
                let mem_max = match raw.trim() {
                    "" => Patch::Clear,
                    m => m.parse::<u32>().map(Patch::Set).unwrap_or(Patch::Unchanged),
                };
                dispatch.update_cluster_profile(
                    cluster_id,
                    ProfileUpdate {
                        mem_max,
                        ..Default::default()
                    },
                );
            });
        }

        let on_reset: EventHandler<()> = (move |()| {
            last.set(String::new());
            memory.set(String::new());
            dispatch.update_cluster_profile(cluster_id, clear_update(Field::MemMax));
        })
        .into();

        let control = memory_field(
            memory,
            t("oneclient.cluster.settings.memory.global"),
            global,
        );

        settings_row(
            IconType::Database01,
            t("oneclient.cluster.settings.memory.title"),
            t("oneclient.cluster.settings.memory.description"),
            override_cell(control, overridden, on_reset),
        )
    }
}

#[derive(PartialEq)]
struct ResolutionRow {
    cluster_id: i64,
    value: Option<Resolution>,
    global: Resolution,
}

impl Component for ResolutionRow {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let overridden = self.value.is_some();
        let dispatch = use_dispatch();
        let global = self.global;
        let resolved = self.value.unwrap_or(self.global);

        let mut width = use_state({
            let v = resolved.width.to_string();
            move || v
        });
        let mut height = use_state({
            let v = resolved.height.to_string();
            move || v
        });
        let mut last = use_state(move || (resolved.width.to_string(), resolved.height.to_string()));
        {
            let dispatch = dispatch.clone();
            use_side_effect(move || {
                let w = width.read().clone();
                let h = height.read().clone();
                if (w.clone(), h.clone()) == *last.peek() {
                    return;
                }
                last.set((w.clone(), h.clone()));
                let resolution = match (w.trim(), h.trim()) {
                    ("", "") => Patch::Clear,
                    (w, h) => match (w.parse::<u32>(), h.parse::<u32>()) {
                        (Ok(w), Ok(h)) => Patch::Set(Resolution::new(w, h)),
                        _ => Patch::Unchanged,
                    },
                };
                dispatch.update_cluster_profile(
                    cluster_id,
                    ProfileUpdate {
                        resolution,
                        ..Default::default()
                    },
                );
            });
        }

        let on_reset: EventHandler<()> = (move |()| {
            let gw = global.width.to_string();
            let gh = global.height.to_string();
            last.set((gw.clone(), gh.clone()));
            width.set(gw);
            height.set(gh);
            dispatch.update_cluster_profile(cluster_id, clear_update(Field::Resolution));
        })
        .into();

        let control = rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(8.)
            .child(
                TextInput::new(width)
                    .placeholder("854")
                    .on_validate(validate_number)
                    .text_align(TextAlign::Center)
                    .width(Size::px(70.)),
            )
            .child(
                Icon::new(IconType::X)
                    .size(14.)
                    .color(colors::fg_secondary()),
            )
            .child(
                TextInput::new(height)
                    .placeholder("480")
                    .on_validate(validate_number)
                    .text_align(TextAlign::Center)
                    .width(Size::px(70.)),
            );

        settings_row(
            IconType::LayoutTop,
            t("oneclient.cluster.settings.resolution.title"),
            t("oneclient.cluster.settings.resolution.description"),
            override_cell(control, overridden, on_reset),
        )
    }
}

#[derive(Clone, Copy, PartialEq)]
enum TextField {
    JvmArgs,
    GameArgs,
    Pre,
    Wrapper,
    Post,
}

impl TextField {
    fn meta(self) -> (IconType, &'static str, &'static str, &'static str) {
        match self {
            Self::JvmArgs => (
                IconType::Terminal,
                t("oneclient.cluster.settings.jvm_args.title"),
                t("oneclient.cluster.settings.jvm_args.description"),
                "-XX:+UseG1GC",
            ),
            Self::GameArgs => (
                IconType::Play,
                t("oneclient.cluster.settings.game_args.title"),
                t("oneclient.cluster.settings.game_args.description"),
                "--tracy --tracyNoImages",
            ),
            Self::Pre => (
                IconType::FilePlus02,
                t("oneclient.cluster.settings.pre_launch.title"),
                t("oneclient.cluster.settings.pre_launch.description"),
                "echo 'Game started'",
            ),
            Self::Wrapper => (
                IconType::ParagraphWrap,
                t("oneclient.cluster.settings.wrapper.title"),
                t("oneclient.cluster.settings.wrapper.description"),
                "gamescope",
            ),
            Self::Post => (
                IconType::FileX02,
                t("oneclient.cluster.settings.post_exit.title"),
                t("oneclient.cluster.settings.post_exit.description"),
                "echo 'Game exited'",
            ),
        }
    }

    fn value(self, profile: &GameSettingsProfile) -> Option<String> {
        match self {
            Self::JvmArgs => profile.launch_args.clone(),
            Self::GameArgs => profile.game_args.clone(),
            Self::Pre => profile.hook_pre.clone(),
            Self::Wrapper => profile.hook_wrapper.clone(),
            Self::Post => profile.hook_post.clone(),
        }
    }

    fn patch(self, value: Patch<String>) -> ProfileUpdate {
        let mut u = ProfileUpdate::default();
        match self {
            Self::JvmArgs => u.launch_args = value,
            Self::GameArgs => u.game_args = value,
            Self::Pre => u.hook_pre = value,
            Self::Wrapper => u.hook_wrapper = value,
            Self::Post => u.hook_post = value,
        }
        u
    }
}

fn text_row(
    cluster_id: i64,
    field: TextField,
    profile: &GameSettingsProfile,
    global: &GameSettingsProfile,
) -> impl IntoElement {
    TextRow {
        cluster_id,
        field,
        value: field.value(profile),
        global: field.value(global).unwrap_or_default(),
    }
    .into_element()
}

#[derive(PartialEq)]
struct TextRow {
    cluster_id: i64,
    field: TextField,
    value: Option<String>,
    global: String,
}

impl Component for TextRow {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let field = self.field;
        let overridden = self.value.is_some();
        let (icon, title, description, placeholder) = field.meta();
        let dispatch = use_dispatch();

        let global = self.global.clone();
        let initial = self.value.clone().unwrap_or_else(|| self.global.clone());
        let mut text = use_state({
            let v = initial.clone();
            move || v
        });
        let mut last = use_state(move || initial);
        {
            let dispatch = dispatch.clone();
            use_side_effect(move || {
                let raw = text.read().clone();
                if raw == *last.peek() {
                    return;
                }
                last.set(raw.clone());
                let patch = match raw.trim() {
                    "" => Patch::Clear,
                    v => Patch::Set(v.to_string()),
                };
                dispatch.update_cluster_profile(cluster_id, field.patch(patch));
            });
        }

        let on_reset: EventHandler<()> = (move |()| {
            last.set(global.clone());
            text.set(global.clone());
            dispatch.update_cluster_profile(cluster_id, field.patch(Patch::Clear));
        })
        .into();

        let control = TextInput::new(text)
            .placeholder(placeholder)
            .expandable(true)
            .width(settings_control_width());

        settings_row(
            icon,
            title,
            description,
            override_cell(control, overridden, on_reset),
        )
    }
}

#[derive(PartialEq)]
struct JavaRow {
    cluster_id: i64,
    value: Option<String>,
    global: Option<String>,
    runtimes: Vec<JavaRuntime>,
}

impl Component for JavaRow {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let overridden = self.value.is_some();
        let dispatch = use_dispatch();
        let runtimes = self.runtimes.clone();

        let launcher_default = t("oneclient.cluster.settings.java.launcher_default");
        let inherited = self
            .global
            .as_ref()
            .and_then(|path| {
                runtimes
                    .iter()
                    .find(|runtime| &runtime.absolute_path == path)
            })
            .map(runtime_label)
            .unwrap_or_else(|| t("oneclient.cluster.settings.java.automatic").into());
        let default_runtime = tf(
            "oneclient.cluster.settings.java.default_runtime",
            &[&inherited],
        );
        let description = format!(
            "{} {}",
            t("oneclient.cluster.settings.java.description"),
            default_runtime,
        );

        let control: Element = if runtimes.is_empty() && !overridden {
            label()
                .text(t("oneclient.cluster.settings.java.none"))
                .font_size(text_size(12.))
                .color(colors::fg_secondary())
                .into_element()
        } else {
            let mut options: Vec<String> = vec![launcher_default.into()];
            options.extend(runtimes.iter().map(runtime_label));

            let selected = self
                .value
                .as_ref()
                .map(|path| {
                    runtimes
                        .iter()
                        .find(|r| &r.absolute_path == path)
                        .map(runtime_label)
                        .unwrap_or_else(|| t("oneclient.cluster.settings.java.automatic").into())
                })
                .unwrap_or_else(|| launcher_default.into());

            let paths: Vec<String> = runtimes.iter().map(|r| r.absolute_path.clone()).collect();

            let dispatch = dispatch.clone();
            Dropdown::new(selected, options)
                .width(settings_control_width())
                .height(Size::px(34.))
                .on_select(move |idx: usize| {
                    if idx == 0 {
                        dispatch.update_cluster_profile(cluster_id, clear_update(Field::JavaPath));
                    } else if let Some(path) = paths.get(idx - 1) {
                        dispatch.update_cluster_profile(
                            cluster_id,
                            ProfileUpdate {
                                java_path: Patch::Set(path.clone()),
                                ..Default::default()
                            },
                        );
                    }
                })
                .into_element()
        };

        let on_reset: EventHandler<()> = (move |()| {
            dispatch.update_cluster_profile(cluster_id, clear_update(Field::JavaPath));
        })
        .into();

        let manage = Button::new()
            .small()
            .ghost()
            .icon()
            .tooltip(t("oneclient.cluster.settings.java.manage"))
            .corner_radius(rounded(7.))
            .on_press(|_| {
                let _ = RouterContext::get().push(Route::SettingsJava {});
            })
            .child(
                Icon::new(IconType::Download01)
                    .size(15.)
                    .color(colors::fg_secondary()),
            );

        settings_row(
            IconType::CodeSnippet02,
            t("oneclient.cluster.settings.java.title"),
            description,
            override_cell(
                rect()
                    .horizontal()
                    .cross_align(Alignment::Center)
                    .spacing(10.)
                    .child(control)
                    .child(manage),
                overridden,
                on_reset,
            ),
        )
    }
}

#[derive(PartialEq)]
struct BrowserUpdateModeRow {
    cluster_id: i64,
    value: Option<PackageUpdateMode>,
    global: PackageUpdateMode,
}

impl Component for BrowserUpdateModeRow {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let overridden = self.value.is_some();
        let resolved = self.value.unwrap_or(self.global);
        let dispatch = use_dispatch();

        let options: Vec<String> = PackageUpdateMode::ALL
            .iter()
            .map(|mode| crate::view::app::settings::update_mode_label(*mode).to_string())
            .collect();

        let control = {
            let dispatch = dispatch.clone();
            Dropdown::new(
                crate::view::app::settings::update_mode_label(resolved),
                options,
            )
            .width(settings_control_width())
            .height(Size::px(34.))
            .on_select(move |idx: usize| {
                if let Some(mode) = PackageUpdateMode::ALL.get(idx).copied() {
                    dispatch.update_cluster_profile(
                        cluster_id,
                        ProfileUpdate {
                            browser_update_mode: Patch::Set(mode),
                            ..Default::default()
                        },
                    );
                }
            })
        };

        let on_reset: EventHandler<()> = (move |()| {
            dispatch.update_cluster_profile(cluster_id, clear_update(Field::BrowserUpdateMode));
        })
        .into();

        settings_row(
            IconType::RefreshCw01,
            t("oneclient.cluster.settings.browser_updates.title"),
            t("oneclient.cluster.settings.browser_updates.description"),
            override_cell(control, overridden, on_reset),
        )
    }
}

#[derive(PartialEq)]
struct GraphicsBackendRow {
    cluster_id: i64,
    game_dir: PathBuf,
}

impl Component for GraphicsBackendRow {
    fn render(&self) -> impl IntoElement {
        let game_dir = self.game_dir.clone();
        let current = use_graphics_backend(game_dir.clone());
        let running = use_game_snapshot().is_running(self.cluster_id);
        let dispatch = use_dispatch();

        let options: Vec<String> = GraphicsBackend::ALL
            .iter()
            .map(ToString::to_string)
            .collect();
        let control = Dropdown::new(
            current.unwrap_or(GraphicsBackend::Default).to_string(),
            options,
        )
        .width(settings_control_width())
        .height(Size::px(34.))
        .on_select(move |idx: usize| {
            if let Some(backend) = GraphicsBackend::ALL.get(idx).copied() {
                dispatch.set_graphics_backend(game_dir.clone(), backend);
            }
        });

        let locked = if running {
            Some(t(
                "oneclient.cluster.settings.graphics_backend.locked_running",
            ))
        } else if current.is_none() {
            Some(t(
                "oneclient.cluster.settings.graphics_backend.locked_never_started",
            ))
        } else {
            None
        };
        if let Some(reason) = locked {
            return settings_row_disabled(
                IconType::PaintPour,
                t("oneclient.cluster.settings.graphics_backend.title"),
                reason,
                control,
            )
            .into_element();
        }

        settings_row(
            IconType::PaintPour,
            t("oneclient.cluster.settings.graphics_backend.title"),
            t("oneclient.cluster.settings.graphics_backend.description"),
            control,
        )
        .into_element()
    }
}

#[derive(PartialEq)]
struct MigrateFromRow {
    cluster_id: i64,
}

impl Component for MigrateFromRow {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let dispatch = use_dispatch();
        let checking = use_release_migration_checking(cluster_id);
        let game = use_game_snapshot();
        let clusters = settled_or_loading(&use_clusters()).unwrap_or_default();
        let mut menu_at = use_state(|| None::<(f32, f32)>);
        let mut creating = use_state(|| false);
        let is_source = use_is_migration_source(cluster_id);
        let this = clusters.iter().find(|cluster| cluster.id == cluster_id);
        let source_loader = this.map(|cluster| cluster.mc_loader);

        let routes: Vec<(i64, i64)> = this
            .map(|this| {
                rank_migration_sources(this, &clusters)
                    .iter()
                    .map(|source| (cluster_id, source.id))
                    .chain(
                        rank_migration_targets(this, &clusters)
                            .iter()
                            .map(|target| (target.id, cluster_id)),
                    )
                    .collect()
            })
            .unwrap_or_default();
        let has_partners = !routes.is_empty();
        let migratable = use_migratable_routes(routes.clone());
        let can_migrate = routes.iter().any(|route| migratable.contains(route));
        let running = game.is_running(cluster_id);
        let updating = game.is_checking_updates(cluster_id);

        let mut button = Button::new()
            .secondary()
            .enabled(can_migrate && !checking && !running && !updating)
            .on_press(move |_| {
                dispatch.open_manual_migration(cluster_id, MigrationDirection::Into, None);
            });
        button = if checking {
            button
                .child(Icon::new(IconType::Loading02).size(14.))
                .text(t("oneclient.cluster.settings.migrate.checking"))
        } else {
            button
                .child(Icon::new(IconType::Copy01).size(14.))
                .text(t("oneclient.cluster.settings.migrate.button"))
        };
        if running {
            button = button.tooltip(t("oneclient.cluster.settings.migrate.game_running"));
        }
        if updating {
            button = button.tooltip(t("oneclient.cluster.settings.migrate.update_check_running"));
        }
        if !can_migrate {
            button = button.tooltip(if has_partners {
                t("oneclient.cluster.settings.migrate.nothing")
            } else {
                t("oneclient.cluster.settings.migrate.no_partners")
            });
        }

        let menu = (*menu_at.read()).map(|(x, y)| {
            ContextMenu::new(x, y)
                .action(
                    IconType::Plus,
                    t("oneclient.cluster.settings.migrate.new_instance"),
                    move |()| creating.set(true),
                )
                .on_close(move |()| menu_at.set(None))
                .into_element()
        });

        rect()
            .vertical()
            .width(Size::fill())
            .child(settings_row(
                IconType::Copy01,
                t("oneclient.cluster.settings.migrate.title"),
                t("oneclient.cluster.settings.migrate.description"),
                rect()
                    .horizontal()
                    .cross_align(Alignment::Center)
                    .spacing(8.)
                    .child(button)
                    .maybe_child(
                        (is_source && source_loader.is_some())
                            .then(|| kebab_button((move |point| menu_at.set(Some(point))).into())),
                    ),
            ))
            .maybe_child(menu)
            .maybe_child(source_loader.filter(|_| *creating.read()).map(|loader| {
                CreateInstanceModal::new(move |()| creating.set(false))
                    .migrate_from(cluster_id, loader)
                    .into_element()
            }))
    }
}

#[derive(PartialEq)]
struct LoaderRow {
    cluster_id: i64,
    loader: GameLoader,
    selected: Option<String>,
    versions: Arc<[String]>,
}

impl Component for LoaderRow {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let dispatch = use_dispatch();
        let versions = self.versions.clone();
        let selected = self
            .selected
            .clone()
            .unwrap_or_else(|| versions.first().cloned().unwrap_or_else(|| "Latest".into()));

        let control: Element = if versions.is_empty() {
            label()
                .text(t("oneclient.cluster.settings.loader.none"))
                .font_size(text_size(12.))
                .color(colors::fg_secondary())
                .into_element()
        } else {
            let options = versions.to_vec();
            Dropdown::new(selected, options.clone())
                .width(settings_control_width())
                .height(Size::px(34.))
                .on_select(move |idx: usize| {
                    if let Some(version) = options.get(idx) {
                        dispatch.set_cluster_loader_version(cluster_id, version.clone());
                    }
                })
                .into_element()
        };

        settings_row(
            IconType::Rocket02,
            t("oneclient.cluster.settings.loader.title"),
            tf(
                "oneclient.cluster.settings.loader.description",
                &[&self.loader],
            ),
            control,
        )
        .into_element()
    }
}

#[derive(PartialEq)]
struct CloudSyncRow {
    cluster_id: i64,
    name: String,
}

impl Component for CloudSyncRow {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let name = self.name.clone();
        let dispatch = use_dispatch();
        let cloud = use_cloud_sync();
        let status = cloud.statuses.get(&cluster_id).cloned();
        let mut choosing = use_state(|| false);

        if !cloud.signed_in {
            return settings_row_disabled(
                IconType::DownloadCloud02,
                t("oneclient.cluster.settings.cloud_sync.title"),
                t("oneclient.cluster.settings.cloud_sync.signed_out"),
                toggle_controlled(false, (|()| {}).into()),
            )
            .into_element();
        }

        let on = status.is_some();
        let busy = matches!(status, Some(SyncStatus::Syncing));
        let description = match &status {
            None => t("oneclient.cluster.settings.cloud_sync.off").to_string(),
            Some(SyncStatus::Synced) => {
                t("oneclient.cluster.settings.cloud_sync.synced").to_string()
            }
            Some(SyncStatus::Pending) => {
                t("oneclient.cluster.settings.cloud_sync.pending").to_string()
            }
            Some(SyncStatus::Syncing) => {
                t("oneclient.cluster.settings.cloud_sync.syncing").to_string()
            }
            Some(SyncStatus::Conflict { .. }) => {
                t("oneclient.cluster.settings.cloud_sync.conflict").to_string()
            }
            Some(SyncStatus::Error { message }) => {
                tf("oneclient.cluster.settings.cloud_sync.error", &[message])
            }
        };

        let toggle = {
            let dispatch = dispatch.clone();
            toggle_controlled(
                on,
                (move |()| {
                    if !busy {
                        dispatch.set_cloud_sync(cluster_id, !on);
                    }
                })
                .into(),
            )
        };

        let action = match &status {
            Some(SyncStatus::Conflict { .. }) => {
                let dispatch = dispatch.clone();
                Some(
                    Button::new()
                        .small()
                        .secondary()
                        .on_press(move |_| {
                            dispatch.resolve_cloud_conflict(cluster_id, name.clone())
                        })
                        .text(t("oneclient.cluster.settings.cloud_sync.resolve"))
                        .into_element(),
                )
            }
            Some(SyncStatus::Error { .. }) => {
                let dispatch = dispatch.clone();
                Some(
                    Button::new()
                        .small()
                        .secondary()
                        .on_press(move |_| dispatch.sync_cluster_now(cluster_id))
                        .text(t("oneclient.common.retry"))
                        .into_element(),
                )
            }
            _ => None,
        };

        let files = (on && !busy).then(|| {
            Button::new()
                .small()
                .ghost()
                .on_press(move |_| choosing.set(true))
                .text(t("oneclient.cluster.settings.cloud_sync.files"))
                .into_element()
        });

        let trailing = rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(10.)
            .maybe_child(files)
            .maybe_child(action)
            .child(toggle);

        rect()
            .width(Size::fill())
            .child(settings_row(
                IconType::DownloadCloud02,
                t("oneclient.cluster.settings.cloud_sync.title"),
                description,
                trailing,
            ))
            .maybe_child(choosing.read().then(|| {
                SyncFilesModal {
                    cluster_id,
                    on_close: (move |()| choosing.set(false)).into(),
                }
                .into_element()
            }))
            .into_element()
    }
}
