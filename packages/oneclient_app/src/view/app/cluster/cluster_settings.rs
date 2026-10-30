use std::path::PathBuf;
use std::sync::Arc;

use freya::prelude::*;
use freya::query::MutationStateData;
use oneclient_common::Patch;
use oneclient_common::domain::{GameLoader, GraphicsBackend};
use oneclient_core::settings::{GameSettingsProfile, PackageUpdateMode, ProfileUpdate, Resolution};
use oneclient_java::JavaRuntime;

use crate::components::{
    Button, Dropdown, Icon, IconType, ScrollArea, TextInput, memory_field, toggle,
    toggle_controlled, validate_number,
};
use crate::hooks::{
    ClusterAction, java_runtimes, loader_versions, mutation_is_running, query_error,
    settled_or_loading, try_game_profile, use_cluster_mutation, use_clusters, use_dispatch,
    use_game_profile, use_game_snapshot, use_graphics_backend, use_java_runtimes, use_launcher,
    use_loader_versions, use_migratable_routes, use_release_migration_checking,
    use_settings_snapshot,
};
use crate::i18n::{t, tf};
use crate::layout::cluster_content;
use crate::theme::colors;
use crate::ui::centered_note;
use crate::view::app::clusters::{
    ClusterPickerButton, DeleteInstanceModal, EditInstanceModal, ExportInstanceModal,
    InstanceFacts, ResetInstanceModal,
};
use crate::view::app::launch_syncing;
use crate::view::app::settings::{
    section_header, settings_row, settings_row_danger, settings_row_disabled,
};
use oneclient_core::clusters::{Cluster, rank_migration_sources};

use super::cluster_not_found;
use super::imported_modpacks::ImportedModpacksSection;
use super::modpack_settings::{ModpackRepairRow, ModpackUpdateRow};
use crate::hooks::use_cluster;

#[derive(PartialEq)]
pub struct ClusterSettings {
    pub cluster_id: i64,
}

impl Component for ClusterSettings {
    fn render(&self) -> impl IntoElement {
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
                    name: cluster.name.clone(),
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

        let migrate_row: Vec<Element> = if mod_loader {
            vec![MigrateFromRow { cluster_id }.into_element()]
        } else {
            Vec::new()
        };
        let problem_mod_row: Vec<Element> = if mod_loader {
            vec![super::FindProblemModRow { cluster_id }.into_element()]
        } else {
            Vec::new()
        };

        let sync_row: Vec<Element> = if cluster.uses_dedicated_dir() && !cluster.is_isolated() {
            vec![SyncSharedRow { cluster_id }.into_element()]
        } else {
            Vec::new()
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

        cluster_content()
            .child(
                ScrollArea::new()
                    .width(Size::fill())
                    .height(Size::fill())
                    .scrollbar_gutter(true)
                    .spacing(4.)
                    .child(section_header(t(
                        "oneclient.cluster.settings.section.instance",
                    )))
                    .child(instance_row)
                    .child(
                        CheckUpdatesRow {
                            cluster_id,
                            uses_bundles: cluster.uses_bundles(),
                        }
                        .into_element(),
                    )
                    .child(
                        DedicatedDirRow {
                            cluster_id,
                            dedicated: cluster.uses_dedicated_dir(),
                            locked: cluster.is_isolated(),
                        }
                        .into_element(),
                    )
                    .append_children(sync_row)
                    .child(section_header(t(
                        "oneclient.cluster.settings.section.shortcut",
                    )))
                    .child(ShortcutRow { cluster_id }.into_element())
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
                    .append_children(migrate_row)
                    .child(section_header(t("oneclient.cluster.settings.section.game")))
                    .child(profile_row(
                        generation,
                        "ToggleRow",
                        ToggleRow {
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
                    .append_children(loader_section)
                    .child(section_header(t(
                        "oneclient.cluster.settings.section.sharing",
                    )))
                    .child(
                        ExportRow {
                            cluster_id,
                            name: cluster.name.clone(),
                        }
                        .into_element(),
                    )
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
                    ))
                    .child(section_header(t(
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
                    .child(
                        ResetInstanceRow {
                            cluster_id,
                            name: cluster.name.clone(),
                            on_reset: (move |()| *resets.write() += 1).into(),
                        }
                        .into_element(),
                    ),
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
    }
    u
}

fn runtime_label(runtime: &JavaRuntime) -> String {
    format!("{} {} ({})", runtime.vendor, runtime.major, runtime.version)
}

fn reset_button(overridden: bool, on_reset: EventHandler<()>) -> impl IntoElement {
    let color = if overridden {
        colors::fg_secondary()
    } else {
        colors::fg_secondary().with_a(60)
    };

    Button::new()
        .small()
        .ghost()
        .icon()
        .tooltip(if overridden {
            t("oneclient.cluster.settings.override.reset")
        } else {
            t("oneclient.cluster.settings.override.global")
        })
        .corner_radius(CornerRadius::new_all(7.))
        .maybe(overridden, |el| el.on_press(move |_| on_reset.call(())))
        .enabled(overridden)
        .child(Icon::new(IconType::RefreshCcw02).size(15.).color(color))
        .into_element()
}

fn override_cell(
    control: impl IntoElement,
    overridden: bool,
    on_reset: EventHandler<()>,
) -> impl IntoElement {
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(10.)
        .child(control)
        .child(reset_button(overridden, on_reset))
        .into_element()
}

#[derive(PartialEq)]
struct ToggleRow {
    cluster_id: i64,
    value: Option<bool>,
    global: bool,
}

impl Component for ToggleRow {
    fn render(&self) -> impl IntoElement {
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
                dispatch.update_cluster_profile(
                    cluster_id,
                    ProfileUpdate {
                        force_fullscreen: Patch::Set(v),
                        ..Default::default()
                    },
                );
            });
        }

        let on_reset: EventHandler<()> = (move |()| {
            last.set(global);
            state.set(global);
            dispatch.update_cluster_profile(cluster_id, clear_update(Field::ForceFullscreen));
        })
        .into();

        settings_row(
            IconType::Maximize01,
            t("oneclient.cluster.settings.force_fullscreen.title"),
            t("oneclient.cluster.settings.force_fullscreen.description"),
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
                IconType::Pencil01,
                t("oneclient.cluster.settings.instance.title"),
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

        let button = Button::new()
            .small()
            .secondary()
            .on_press(move |_| choosing.set(true))
            .text(t("oneclient.cluster.settings.export.button"));

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
struct ResetInstanceRow {
    cluster_id: i64,
    name: String,
    on_reset: EventHandler<()>,
}

impl Component for ResetInstanceRow {
    fn render(&self) -> impl IntoElement {
        let mut confirming = use_state(|| false);
        let cluster_id = self.cluster_id;
        let name = self.name.clone();
        let mutation = use_cluster_mutation();
        let running = mutation_is_running(&mutation);
        let on_reset = self.on_reset.clone();
        // The confirm modal unmounts as soon as Reset is pressed, so the wait for the reset
        // runs in this row's scope to still report back once it has finished
        let scope = current_scope_id();

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
                let on_reset = on_reset.clone();
                ResetInstanceModal::new(
                    cluster_id,
                    name.clone(),
                    move |()| confirming.set(false),
                    move |()| {
                        let on_reset = on_reset.clone();
                        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
                        // Detached like `mutate` so leaving the page can't cut the reset short
                        spawn_forever(async move {
                            mutation
                                .mutate_async(ClusterAction::ResetInstance { cluster_id })
                                .await;
                            let _ = done_tx.send(());
                        });
                        spawn_in_scope(
                            async move {
                                if done_rx.await.is_ok() {
                                    on_reset.call(());
                                }
                            },
                            scope,
                        );
                    },
                )
                .into_element()
            }))
    }
}

#[derive(PartialEq)]
struct SyncSharedRow {
    cluster_id: i64,
}

impl Component for SyncSharedRow {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let dispatch = use_dispatch();
        let syncing = use_game_snapshot().is_syncing_shared(cluster_id);

        let button = Button::new()
            .small()
            .secondary()
            .enabled(!syncing)
            .maybe(!syncing, |el| {
                el.on_press(move |_| dispatch.sync_from_shared_dir(cluster_id))
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
            button,
        )
    }
}

#[derive(PartialEq)]
struct DedicatedDirRow {
    cluster_id: i64,
    dedicated: bool,
    locked: bool,
}

impl Component for DedicatedDirRow {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let dedicated = self.dedicated;
        let mutation = use_cluster_mutation();
        let dispatch = use_dispatch();

        if self.locked {
            return settings_row_disabled(
                IconType::Folder,
                t("oneclient.cluster.settings.dedicated_dir.title"),
                t("oneclient.cluster.settings.dedicated_dir.locked"),
                toggle_controlled(true, (|()| {}).into()),
            )
            .into_element();
        }

        let on_toggle: EventHandler<()> = (move |()| {
            let action = ClusterAction::SetDedicatedDir {
                cluster_id,
                dedicated: !dedicated,
            };
            if dedicated {
                mutation.mutate(action);
                return;
            }
            // The new folder starts empty, so offer to bring the shared worlds and settings along
            let dispatch = dispatch.clone();
            spawn_forever(async move {
                let reader = mutation.mutate_async(action).await;
                if matches!(
                    &*reader.state(),
                    MutationStateData::Settled { res: Ok(_), .. }
                ) {
                    dispatch.sync_from_shared_dir(cluster_id);
                }
            });
        })
        .into();

        settings_row(
            IconType::Folder,
            t("oneclient.cluster.settings.dedicated_dir.title"),
            t("oneclient.cluster.settings.dedicated_dir.description"),
            toggle_controlled(dedicated, on_toggle),
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
            .width(Size::px(220.));

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

        let resolved = self.value.clone().or_else(|| self.global.clone());

        let control: Element = if runtimes.is_empty() {
            label()
                .text(t("oneclient.cluster.settings.java.none"))
                .font_size(12.)
                .color(colors::fg_secondary())
                .into_element()
        } else {
            let mut options: Vec<String> =
                vec![t("oneclient.cluster.settings.java.automatic").into()];
            options.extend(runtimes.iter().map(runtime_label));

            let selected = resolved
                .as_ref()
                .and_then(|path| runtimes.iter().find(|r| &r.absolute_path == path))
                .map(runtime_label)
                .unwrap_or_else(|| t("oneclient.cluster.settings.java.automatic").into());

            let paths: Vec<String> = runtimes.iter().map(|r| r.absolute_path.clone()).collect();

            let dispatch = dispatch.clone();
            Dropdown::new(selected, options)
                .width(Size::px(260.))
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

        settings_row(
            IconType::CodeSnippet02,
            t("oneclient.cluster.settings.java.title"),
            t("oneclient.cluster.settings.java.description"),
            override_cell(control, overridden, on_reset),
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
            .map(|mode| mode.label().to_string())
            .collect();

        let control = {
            let dispatch = dispatch.clone();
            Dropdown::new(resolved.label(), options)
                .width(Size::px(220.))
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
        .width(Size::px(220.))
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
        let mut picked = use_state(|| None::<i64>);

        let mut sources: Vec<Cluster> = clusters
            .iter()
            .find(|cluster| cluster.id == cluster_id)
            .map(|this| rank_migration_sources(this, &clusters))
            .unwrap_or_default();
        let has_partners = !sources.is_empty();
        let migratable = use_migratable_routes(
            sources
                .iter()
                .map(|source| (cluster_id, source.id))
                .collect(),
        );
        sources.retain(|source| migratable.contains(&(cluster_id, source.id)));

        let selected = picked
            .read()
            .and_then(|id| sources.iter().find(|source| source.id == id))
            .or_else(|| sources.first())
            .cloned();
        let selected_pair = selected.as_ref().map(|source| (cluster_id, source.id));
        let running = selected_pair
            .is_some_and(|(target, source)| game.is_running(target) || game.is_running(source));
        let updating = selected_pair.is_some_and(|(target, _)| game.is_checking_updates(target));

        let mut button = Button::new()
            .secondary()
            .enabled(selected_pair.is_some() && !checking && !running && !updating)
            .on_press(move |_| {
                if let Some((target, source)) = selected_pair {
                    dispatch.open_manual_migration(target, source);
                }
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

        let picker: Element = if sources.is_empty() {
            label()
                .text(if has_partners {
                    t("oneclient.cluster.settings.migrate.nothing")
                } else {
                    t("oneclient.cluster.settings.migrate.no_partners")
                })
                .font_size(12.)
                .color(colors::fg_secondary())
                .into_element()
        } else {
            ClusterPickerButton {
                label: selected
                    .as_ref()
                    .map(|source| tf("oneclient.cluster.settings.migrate.from", &[&source.name]))
                    .unwrap_or_default(),
                selected: selected.as_ref().map(|source| source.id),
                only: Some(sources.iter().map(|source| source.id).collect()),
                width: Size::px(220.),
                height: 34.,
                on_pick: (move |cluster: Cluster| picked.set(Some(cluster.id))).into(),
            }
            .into_element()
        };

        settings_row(
            IconType::Copy01,
            t("oneclient.cluster.settings.migrate.title"),
            t("oneclient.cluster.settings.migrate.description"),
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(8.)
                .child(picker)
                .child(button),
        )
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
                .font_size(12.)
                .color(colors::fg_secondary())
                .into_element()
        } else {
            let options = versions.to_vec();
            Dropdown::new(selected, options.clone())
                .width(Size::px(220.))
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
