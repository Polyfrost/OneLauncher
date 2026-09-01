use freya::prelude::*;
use oneclient_common::Patch;
use oneclient_java::JavaRuntime;
use oneclient_common::domain::GameLoader;
use oneclient_core::settings::{
    GameSettingsProfile, PackageUpdateMode, ProfileUpdate, Resolution,
};

use freya::router::RouterContext;

use crate::components::{
    Button, Dropdown, Icon, IconType, OverlayPopup, ScrollArea, TextInput, memory_field, toggle,
    toggle_controlled, validate_number,
};
use crate::hooks::{
    ClusterAction, java_runtimes, loader_versions, mutation_is_running, query_error,
    try_game_profile, use_active_cluster_id, use_cluster_mutation, use_dispatch, use_game_profile,
    use_java_runtimes, use_loader_versions, use_settings_snapshot,
};
use crate::layout::cluster_content;
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::{border_all_color, centered_note};
use crate::view::app::settings::{section_header, settings_row};

use super::cluster_not_found;
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

        let Some(cluster) = cluster else {
            return cluster_not_found();
        };

        let Some(profile) = try_game_profile(&profile_query) else {
            let note = match query_error(&profile_query) {
                Some(err) => format!("Could not load these settings: {err}"),
                None => "Loading settings...".to_string(),
            };
            return cluster_content().child(centered_note(&note)).into_element();
        };

        let versions = loader_versions(&versions_query);
        let runtimes = java_runtimes(&runtimes_query);

        cluster_content()
            .child(
                ScrollArea::new()
                    .width(Size::fill())
                    .height(Size::fill())
                    .spacing(4.)
                    .child(section_header("INSTANCE"))
                    .children(if cluster.is_provisioned() {
                        vec![default_instance_row()]
                    } else {
                        vec![
                            NameRow {
                                cluster_id,
                                name: cluster.name.clone(),
                            }
                            .into_element(),
                            DeleteRow {
                                cluster_id,
                                name: cluster.name.clone(),
                                dedicated: cluster.uses_dedicated_dir(),
                            }
                            .into_element(),
                        ]
                    })
                    .child(section_header("LOADER"))
                    .child(
                        LoaderRow {
                            cluster_id,
                            loader,
                            selected: cluster.mc_loader_version.clone(),
                            versions,
                        }
                        .into_element(),
                    )
                    .child(section_header("JAVA"))
                    .child(
                        JavaRow {
                            cluster_id,
                            value: profile.java_path.clone(),
                            global: global.java_path.clone(),
                            runtimes,
                        }
                        .into_element(),
                    )
                    .child(text_row(cluster_id, TextField::JvmArgs, &profile, &global))
                    .child(section_header("GAME"))
                    .child(
                        ToggleRow {
                            cluster_id,
                            value: profile.force_fullscreen,
                            global: global.force_fullscreen.unwrap_or(false),
                        }
                        .into_element(),
                    )
                    .child(
                        ResolutionRow {
                            cluster_id,
                            value: profile.resolution,
                            global: global.resolution.unwrap_or_default(),
                        }
                        .into_element(),
                    )
                    .child(
                        MemoryRow {
                            cluster_id,
                            value: profile.mem_max,
                            global: global.mem_max.unwrap_or(4096),
                        }
                        .into_element(),
                    )
                    .child(section_header("CONTENT"))
                    .child(
                        BrowserUpdateModeRow {
                            cluster_id,
                            value: profile.browser_update_mode,
                            global: global.browser_update_mode.unwrap_or_default(),
                        }
                        .into_element(),
                    )
                    .child(section_header("DIRECTORY"))
                    .child(
                        DedicatedDirRow {
                            cluster_id,
                            dedicated: cluster.uses_dedicated_dir(),
                        }
                        .into_element(),
                    )
                    .child(section_header("PROCESS"))
                    .child(text_row(cluster_id, TextField::Pre, &profile, &global))
                    .child(text_row(cluster_id, TextField::Wrapper, &profile, &global))
                    .child(text_row(cluster_id, TextField::Post, &profile, &global))
                    .child(section_header("REPAIR"))
                    .child(VerifyFilesRow { cluster_id }.into_element()),
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
            "Force Fullscreen",
            "Force Minecraft to start in fullscreen mode.",
            override_cell(toggle(state), overridden, on_reset),
        )
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

        let on_press = move |_| {
            mutation.mutate(ClusterAction::VerifyFiles { cluster_id });
        };

        // The pass reads every installed file so say it takes a while otherwise the button reads as broken
        let button = Button::new()
            .small()
            .secondary()
            .enabled(!running)
            .maybe(!running, |el| el.on_press(on_press))
            .text(if running { "Verifying..." } else { "Verify Files" });

        settings_row(
            IconType::ClipboardCheck,
            "Verify Files",
            "Check every installed file against its official checksum and \
             re-download anything corrupt or missing. Takes a few minutes.",
            button,
        )
    }
}

fn default_instance_row() -> Element {
    settings_row(
        IconType::InfoCircle,
        "Default Instance",
        "This version's default instance stays as it is. Duplicate it from the Versions page \
         to get a copy you can rename, remod and delete.",
        rect(),
    )
    .into_element()
}

#[derive(PartialEq)]
struct NameRow {
    cluster_id: i64,
    name: String,
}

impl Component for NameRow {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let cluster_id = self.cluster_id;
        let current = self.name.clone();

        let mut text = use_state({
            let initial = current.clone();
            move || initial
        });
        let mut last_seen = use_state({
            let initial = current.clone();
            move || initial
        });

        if *last_seen.read() != current {
            last_seen.set(current.clone());
            text.set(current.clone());
        }

        let trimmed = text.read().trim().to_string();
        let dirty = !trimmed.is_empty() && trimmed != current;

        settings_row(
            IconType::Pencil01,
            "Name",
            "What this instance is called in your list.",
            rect()
                .horizontal()
                .spacing(8.)
                .cross_align(Alignment::Center)
                .child(TextInput::new(text).width(Size::px(220.)))
                .child(
                    Button::new()
                        .primary()
                        .small()
                        .enabled(dirty)
                        .on_press(move |_| {
                            if dirty {
                                dispatch.rename_cluster(cluster_id, trimmed.clone());
                            }
                        })
                        .text("Save"),
                ),
        )
    }
}

#[derive(PartialEq)]
struct DeleteRow {
    cluster_id: i64,
    name: String,
    dedicated: bool,
}

impl Component for DeleteRow {
    fn render(&self) -> impl IntoElement {
        let mut confirming = use_state(|| false);

        rect()
            .width(Size::fill())
            .child(settings_row(
                IconType::Trash01,
                "Delete Instance",
                "Deletes this instance, its settings and its folder. Downloaded mods stay in the shared cache.",
                Button::new()
                    .danger()
                    .small()
                    .on_press(move |_| {
                        *confirming.write() = true;
                    })
                    .text("Delete"),
            ))
            .maybe_child(confirming.read().then(|| {
                DeleteConfirm {
                    cluster_id: self.cluster_id,
                    name: self.name.clone(),
                    dedicated: self.dedicated,
                    confirming,
                }
                .into_element()
            }))
    }
}

#[derive(PartialEq)]
struct DeleteConfirm {
    cluster_id: i64,
    name: String,
    dedicated: bool,
    confirming: State<bool>,
}

fn delete_warning(dedicated: bool) -> &'static str {
    if dedicated {
        "This instance's folder is deleted, including its worlds, options and configs. \
         Downloaded mods stay in the shared cache. Other instances of this version are \
         not affected."
    } else {
        "This instance's folder is deleted, including its mod list. Worlds and options in \
         the shared game folder are kept, and other instances of this version are not \
         affected."
    }
}

impl Component for DeleteConfirm {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let active = use_active_cluster_id();
        let cluster_id = self.cluster_id;

        let mut confirming = self.confirming;

        let confirm = move |_| {
            dispatch.delete_cluster(cluster_id, active);
            *confirming.write() = false;
            let _ = RouterContext::get().push(Route::Clusters {});
        };

        OverlayPopup::new()
            .on_close(move |()| {
                let mut confirming = confirming;
                *confirming.write() = false;
            })
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(420.))
                            .max_width(Size::window_percent(90.))
                            .spacing(14.)
                            .padding(Gaps::new_all(20.))
                            .corner_radius(CornerRadius::new_all(14.))
                            .background(colors::page_elevated())
                            .border(border_all_color(1., colors::component_border()))
                            .child(
                                label()
                                    .text(format!("Delete {}?", self.name))
                                    .font_size(16.)
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                label()
                                    .text(delete_warning(self.dedicated))
                                    .font_size(12.)
                                    .max_lines(5)
                                    .width(Size::fill())
                                    .color(colors::fg_secondary()),
                            )
                            .child(
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .main_align(Alignment::End)
                                    .spacing(8.)
                                    .child(
                                        Button::new()
                                            .secondary()
                                            .text("Cancel")
                                            .on_press(move |_| {
                                                *confirming.write() = false;
                                            }),
                                    )
                                    .child(
                                        Button::new()
                                            .danger()
                                            .text("Delete")
                                            .on_press(confirm),
                                    ),
                            ),
                    ),
            )
    }
}

#[derive(PartialEq)]
struct DedicatedDirRow {
    cluster_id: i64,
    dedicated: bool,
}

impl Component for DedicatedDirRow {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let dedicated = self.dedicated;
        let mutation = use_cluster_mutation();

        let on_toggle: EventHandler<()> = (move |()| {
            mutation.mutate(ClusterAction::SetDedicatedDir {
                cluster_id,
                dedicated: !dedicated,
            });
        })
        .into();

        settings_row(
            IconType::Folder,
            "Dedicated Directory",
            "Run this cluster in its own .minecraft instead of the shared one.",
            toggle_controlled(dedicated, on_toggle),
        )
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

        let global = self.global.to_string();
        let initial = self.value.unwrap_or(self.global).to_string();
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
            last.set(global.clone());
            memory.set(global.clone());
            dispatch.update_cluster_profile(cluster_id, clear_update(Field::MemMax));
        })
        .into();

        let control = memory_field(memory);

        settings_row(
            IconType::Database01,
            "Memory",
            "The amount of memory in megabytes allocated for the game. Presets leave 2 GB for the system.",
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
            "Resolution",
            "The game window resolution in pixels.",
            override_cell(control, overridden, on_reset),
        )
    }
}

#[derive(Clone, Copy, PartialEq)]
enum TextField {
    JvmArgs,
    Pre,
    Wrapper,
    Post,
}

impl TextField {
    fn meta(self) -> (IconType, &'static str, &'static str, &'static str) {
        match self {
            Self::JvmArgs => (
                IconType::Terminal,
                "JVM Arguments",
                "Extra arguments passed to Java. Separate them with spaces; quote values containing spaces.",
                "-XX:+UseG1GC",
            ),
            Self::Pre => (
                IconType::FilePlus02,
                "Pre-Launch Command",
                "Command to run before launching the game.",
                "echo 'Game started'",
            ),
            Self::Wrapper => (
                IconType::ParagraphWrap,
                "Wrapper Command",
                "Command to run when launching the game.",
                "gamescope",
            ),
            Self::Post => (
                IconType::FileX02,
                "Post-Exit Command",
                "Command to run after exiting the game.",
                "echo 'Game exited'",
            ),
        }
    }

    fn value(self, profile: &GameSettingsProfile) -> Option<String> {
        match self {
            Self::JvmArgs => profile.launch_args.clone(),
            Self::Pre => profile.hook_pre.clone(),
            Self::Wrapper => profile.hook_wrapper.clone(),
            Self::Post => profile.hook_post.clone(),
        }
    }

    fn patch(self, value: Patch<String>) -> ProfileUpdate {
        let mut u = ProfileUpdate::default();
        match self {
            Self::JvmArgs => u.launch_args = value,
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
                .text("No runtimes installed")
                .font_size(12.)
                .color(colors::fg_secondary())
                .into_element()
        } else {
            let mut options: Vec<String> = vec!["Automatic".into()];
            options.extend(runtimes.iter().map(runtime_label));

            let selected = resolved
                .as_ref()
                .and_then(|path| runtimes.iter().find(|r| &r.absolute_path == path))
                .map(runtime_label)
                .unwrap_or_else(|| "Automatic".into());

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
            "Java Runtime",
            "The Java runtime used to launch this cluster.",
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
            "Browser Package Updates",
            "What to do when content installed from the browser has a newer version. Bundle content is not affected.",
            override_cell(control, overridden, on_reset),
        )
    }
}

#[derive(PartialEq)]
struct LoaderRow {
    cluster_id: i64,
    loader: GameLoader,
    selected: Option<String>,
    versions: Vec<String>,
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
                .text("No versions available")
                .font_size(12.)
                .color(colors::fg_secondary())
                .into_element()
        } else {
            let options = versions.clone();
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
            "Loader Version",
            format!(
                "The {} loader version used to launch this cluster.",
                self.loader
            ),
            control,
        )
    }
}
