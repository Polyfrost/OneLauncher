use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_db::console::{ConsoleQueryResult, run_console_query};

use oneclient_auth::preview_samples;
use oneclient_core::simulate::Damage;
use oneclient_net::status::{self, ServiceStatus};

use crate::components::{Button, Dropdown, Icon, IconType, TextInput, login_dialog, toggle};
use crate::hooks::use_dispatch;
use crate::notifications::{
    ClusterUpdateItem, ClusterUpdateSummary, NotificationAction, NotificationActionKind,
};
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::border_all_color;

type SqlResult = Option<Result<ConsoleQueryResult, String>>;

#[derive(PartialEq)]
pub struct Debug;

impl Component for Debug {
    fn render(&self) -> impl IntoElement {
        let log_debug_info = use_state(|| false);
        let show_dev_stuff = use_state(|| false);
        let seen_onboarding = use_state(|| true);
        let use_grid_on_mods_list = use_state(|| true);

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .padding(40.)
            .spacing(8.)
            .child(
                ScrollView::new()
                    .child(
                        rect()
                            .vertical()
                            .child(
                                label()
                                    .text("Debug")
                                    .font_size(32.)
                                    .font_weight(FontWeight::BOLD)
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                label()
                                    .text("The end user won't really be looking at this page.")
                                    .font_size(13.)
                                    .color(colors::fg_secondary()),
                            ),
                    )
                    .child(divider())
                    .child(section(
                        "Settings",
                        vec![
                            toggle_row("Log Debug Info", log_debug_info),
                            toggle_row("Show Dev stuff", show_dev_stuff),
                            toggle_row("Seen Onboarding", seen_onboarding),
                            toggle_row("Use Grid On Mods List", use_grid_on_mods_list),
                        ],
                    ))
                    .child(divider())
                    .child(section(
                        "Toast Controller",
                        vec![ToastController.into_element()],
                    ))
                    .child(divider())
                    .child(section(
                        "Cluster Update Simulator",
                        vec![ClusterUpdateSimulator.into_element()],
                    ))
                    .child(divider())
                    .child(section(
                        "Launcher Auto Update",
                        vec![LauncherUpdateSimulator.into_element()],
                    ))
                    .child(divider())
                    .child(section(
                        "Connectivity Status Bar",
                        vec![StatusBarSimulator.into_element()],
                    ))
                    .child(divider())
                    .child(section(
                        "Auth Error Guidance",
                        vec![AuthGuidancePreview.into_element()],
                    ))
                    .child(divider())
                    .child(section(
                        "Corruption Simulator",
                        vec![CorruptionSimulator.into_element()],
                    ))
                    .child(divider())
                    .child(section("SQL Console", vec![SqlConsole.into_element()]))
                    .child(divider())
                    .child(section(
                        "Other",
                        vec![action_row(vec![
                            ("Open Dev Tools", IconType::CodeSnippet02),
                            ("Open Onboarding", IconType::Rocket02),
                            ("Open Launcher Data", IconType::Folder),
                            ("Log Running Processes", IconType::Terminal),
                        ])],
                    )),
            )
    }
}

#[derive(PartialEq)]
struct ToastController;

impl Component for ToastController {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let title = use_state(|| "Downloading assets".to_string());
        let body = use_state(|| "PolyBlock by Polyfrost".to_string());
        let mut progress = use_state(|| 0u64);

        let info = dispatch.clone();
        let error = dispatch.clone();
        let prog = dispatch.clone();
        let reset = dispatch;

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(10.)
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .spacing(12.)
                    .child(
                        rect()
                            .width(Size::flex(1.0))
                            .child(TextInput::new(title).placeholder("Title")),
                    )
                    .child(
                        rect()
                            .width(Size::flex(1.0))
                            .child(TextInput::new(body).placeholder("Body")),
                    ),
            )
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .spacing(12.)
                    .child(
                        Button::new()
                            .primary()
                            .child(Icon::new(IconType::InfoCircle).size(16.))
                            .text("Send Info")
                            .on_press(move |_| {
                                info.notify(title.read().clone())
                                    .body(body.read().clone())
                                    .info()
                                    .send();
                            }),
                    )
                    .child(
                        Button::new()
                            .danger()
                            .child(Icon::new(IconType::AlertTriangle).size(16.))
                            .text("Send Error")
                            .on_press(move |_| {
                                error
                                    .notify(title.read().clone())
                                    .body(body.read().clone())
                                    .error()
                                    .send();
                            }),
                    )
                    .child(
                        Button::new()
                            .secondary()
                            .child(Icon::new(IconType::FolderDownload).size(16.))
                            .text("Progress +10")
                            .on_press(move |_| {
                                let next = (*progress.read() + 10).min(100);
                                progress.set(next);
                                prog.send_test_progress(next, 100);
                            }),
                    )
                    .child(
                        Button::new()
                            .secondary()
                            .text("Reset Progress")
                            .on_press(move |_| {
                                progress.set(0);
                                reset.send_test_progress(0, 100);
                            }),
                    ),
            )
            .into_element()
    }
}

#[derive(PartialEq)]
struct ClusterUpdateSimulator;

impl Component for ClusterUpdateSimulator {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let cluster_id = use_state(|| "1".to_string());
        let cluster_name = use_state(|| "PolyBlock".to_string());
        let updated = use_state(|| "Sodium 0.5 → 0.6, Iris 1.7 → 1.8".to_string());
        let added = use_state(|| "Lithium".to_string());
        let removed = use_state(|| "OptiFine".to_string());

        let simulate = dispatch.clone();

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(10.)
            .child(
                label()
                    .text("Builds the same notification the bundle sync sends: one \"View changes\" action carrying every changed cluster, opened into the changes modal.")
                    .font_size(13.)
                    .color(colors::fg_secondary()),
            )
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .spacing(12.)
                    .child(
                        rect()
                            .width(Size::px(120.))
                            .child(TextInput::new(cluster_id).placeholder("Cluster ID")),
                    )
                    .child(
                        rect()
                            .width(Size::flex(1.0))
                            .child(TextInput::new(cluster_name).placeholder("Cluster name")),
                    ),
            )
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .spacing(12.)
                    .child(
                        rect().width(Size::flex(1.0)).child(
                            TextInput::new(updated).placeholder("Updated (comma separated)"),
                        ),
                    )
                    .child(
                        rect()
                            .width(Size::flex(1.0))
                            .child(TextInput::new(added).placeholder("Added (comma separated)")),
                    )
                    .child(
                        rect().width(Size::flex(1.0)).child(
                            TextInput::new(removed).placeholder("Removed (comma separated)"),
                        ),
                    ),
            )
            .child(
                Button::new()
                    .primary()
                    .child(Icon::new(IconType::DownloadCloud02).size(16.))
                    .text("Simulate Cluster Update")
                    .on_press(move |_| {
                        let summary = ClusterUpdateSummary {
                            cluster_id: cluster_id.read().trim().parse().unwrap_or(1),
                            cluster_name: cluster_name.read().clone(),
                            updated: cluster_update_items(&split_csv(&updated.read())),
                            added: cluster_update_items(&split_csv(&added.read())),
                            removed: cluster_update_items(&split_csv(&removed.read())),
                        };
                        send_cluster_update(&simulate, vec![summary]);
                    }),
            )
            .child(
                label()
                    .text("Presets: every shape the notification and modal can take.")
                    .font_size(13.)
                    .color(colors::fg_secondary()),
            )
            .children(cluster_update_preset_rows(&dispatch))
            .into_element()
    }
}

type ClusterUpdatePreset = fn() -> Vec<ClusterUpdateSummary>;

/// Each preset pins one variation plural copy empty category tab single- vs multi-cluster footer grouping header overflow
const CLUSTER_UPDATE_PRESETS: [(&str, IconType, ClusterUpdatePreset); 6] = [
    (
        "1 cluster · all categories",
        IconType::DownloadCloud02,
        || {
            vec![preset_summary(
                1,
                "PolyBlock",
                &["Sodium 0.5 → 0.6", "Iris 1.7 → 1.8", "Patcher 1.8 → 1.9"],
                &["Lithium", "FerriteCore"],
                &["OptiFine"],
            )]
        },
    ),
    ("1 cluster · updates only", IconType::RefreshCw01, || {
        vec![preset_summary(
            1,
            "PolyBlock",
            &[
                "Sodium 0.5 → 0.6",
                "Iris 1.7 → 1.8",
                "Indium 1.0 → 1.1",
                "Fabric API 0.92 → 0.100",
            ],
            &[],
            &[],
        )]
    }),
    ("1 cluster · single change", IconType::Plus, || {
        vec![preset_summary(1, "PolyBlock", &[], &["Lithium"], &[])]
    }),
    ("3 clusters · mixed", IconType::DotsGrid, || {
        vec![
            preset_summary(
                1,
                "PolyBlock",
                &["Sodium 0.5 → 0.6", "Iris 1.7 → 1.8"],
                &["Lithium"],
                &[],
            ),
            preset_summary(2, "Skyblock", &[], &[], &["OptiFine", "Skytils"]),
            preset_summary(3, "Vanilla+", &[], &["Sodium", "Iris", "FerriteCore"], &[]),
        ]
    }),
    ("2 clusters · removals only", IconType::Trash01, || {
        vec![
            preset_summary(1, "PolyBlock", &[], &[], &["OptiFine"]),
            preset_summary(2, "Skyblock", &[], &[], &["Skytils", "NotEnoughUpdates"]),
        ]
    }),
    ("6 clusters · long names", IconType::Database01, || {
        (1..=6)
            .map(|i| {
                ClusterUpdateSummary {
                    cluster_id: i,
                    cluster_name: format!(
                        "Cluster {i} with a deliberately overlong name that has to truncate"
                    ),
                    updated: cluster_update_items(&[
                        format!("Sodium 0.{i} → 0.{}", i + 1),
                        format!("Iris 1.{i} → 1.{}", i + 1),
                    ]),
                    added: cluster_update_items(&[format!("Lithium {i}")]),
                    removed: Vec::new(),
                }
            })
            .collect()
    }),
];

fn cluster_update_preset_rows(dispatch: &crate::Actions) -> Vec<Element> {
    CLUSTER_UPDATE_PRESETS
        .chunks(3)
        .map(|chunk| {
            let mut row = rect().horizontal().width(Size::fill()).spacing(12.);

            for (text, icon, build) in chunk {
                let dispatch = dispatch.clone();
                let build = *build;
                row = row.child(
                    Button::new()
                        .secondary()
                        .child(Icon::new(*icon).size(16.))
                        .text(*text)
                        .on_press(move |_| send_cluster_update(&dispatch, build())),
                );
            }

            row.into_element()
        })
        .collect()
}

fn preset_summary(
    cluster_id: i64,
    cluster_name: &str,
    updated: &[&str],
    added: &[&str],
    removed: &[&str],
) -> ClusterUpdateSummary {
    ClusterUpdateSummary {
        cluster_id,
        cluster_name: cluster_name.to_string(),
        updated: cluster_update_items(updated),
        added: cluster_update_items(added),
        removed: cluster_update_items(removed),
    }
}

fn cluster_update_items(names: &[impl AsRef<str>]) -> Vec<ClusterUpdateItem> {
    names
        .iter()
        .map(|name| ClusterUpdateItem::from_name(name.as_ref()))
        .collect()
}

/// Mirrors the copy the bridge builds so the simulated notification matches the real one
fn send_cluster_update(dispatch: &crate::Actions, summaries: Vec<ClusterUpdateSummary>) {
    if summaries.is_empty() {
        return;
    }

    let total: usize = summaries.iter().map(|s| s.total()).sum();
    let plural = if total == 1 { "" } else { "s" };
    let (title, body) = match summaries.as_slice() {
        [only] => (
            "Cluster updated",
            format!("{total} package{plural} changed in {}", only.cluster_name),
        ),
        many => (
            "Mods updated",
            format!(
                "{total} package{plural} updated across {} clusters",
                many.len()
            ),
        ),
    };

    dispatch
        .notify(title)
        .body(body)
        .icon(IconType::DownloadCloud02)
        .action(NotificationAction {
            label: "View changes".to_string(),
            kind: NotificationActionKind::OpenClusterUpdate(summaries),
        })
        .send();
}

#[derive(PartialEq)]
struct LauncherUpdateSimulator;

impl Component for LauncherUpdateSimulator {
    fn render(&self) -> impl IntoElement {
        rect()
            .vertical()
            .width(Size::fill())
            .spacing(10.)
            .child(
                label()
                    .text("Drives the real auto-update UX (prompt → download progress → \"restart to apply\") against a fake release, with no network or disk. \"Check for Updates Now\" runs the real check against the release feed.")
                    .font_size(13.)
                    .color(colors::fg_secondary()),
            )
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .spacing(12.)
                    .child(
                        Button::new()
                            .primary()
                            .child(Icon::new(IconType::DownloadCloud02).size(16.))
                            .text("Simulate Auto Update")
                            .on_press(|_| crate::updater::spawn_simulated_update()),
                    )
                    .child(
                        Button::new()
                            .secondary()
                            .child(Icon::new(IconType::RefreshCw01).size(16.))
                            .text("Check for Updates Now")
                            .on_press(|_| crate::updater::spawn_update_check(false)),
                    ),
            )
            .into_element()
    }
}

/// Pins `oneclient_net::status` so the connectivity banner can be seen without unplugging anything
#[derive(PartialEq)]
struct StatusBarSimulator;

const STATUS_PRESETS: [(&str, IconType, ServiceStatus); 4] = [
    (
        "No internet",
        IconType::Globe01,
        ServiceStatus {
            online: false,
            mc_auth_up: false,
            polyfrost_up: false,
        },
    ),
    (
        "Minecraft auth down",
        IconType::AlertTriangle,
        ServiceStatus {
            online: true,
            mc_auth_up: false,
            polyfrost_up: true,
        },
    ),
    (
        "Polyfrost down",
        IconType::AlertCircle,
        ServiceStatus {
            online: true,
            mc_auth_up: true,
            polyfrost_up: false,
        },
    ),
    (
        "Both services down",
        IconType::AlertTriangle,
        ServiceStatus {
            online: true,
            mc_auth_up: false,
            polyfrost_up: false,
        },
    ),
];

impl Component for StatusBarSimulator {
    fn render(&self) -> impl IntoElement {
        let mut pinned = use_state(status::forced);

        let mut row = rect().horizontal().width(Size::fill()).spacing(12.);
        for (text, icon, forced) in STATUS_PRESETS {
            row = row.child(
                Button::new()
                    .secondary()
                    .child(Icon::new(icon).size(16.))
                    .text(text)
                    .on_press(move |_| {
                        status::force(Some(forced));
                        pinned.set(Some(forced));
                    }),
            );
        }

        let current = *pinned.read();
        let state_text = match current {
            Some(_) => "Pinned. Probing still runs, but its result is not published.".to_string(),
            None => "Not pinned; showing the real probe result.".to_string(),
        };

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(10.)
            .child(
                label()
                    .text("Forces what the connectivity prober reports, so the real status bar renders its banner. Dismissing a banner is remembered until the issue clears.")
                    .font_size(13.)
                    .color(colors::fg_secondary()),
            )
            .child(row)
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .cross_align(Alignment::Center)
                    .spacing(12.)
                    .child(
                        Button::new()
                            .primary()
                            .child(Icon::new(IconType::RefreshCw01).size(16.))
                            .text("Clear + recheck")
                            .enabled(current.is_some())
                            .on_press(move |_| {
                                status::force(None);
                                pinned.set(None);
                            }),
                    )
                    .child(
                        label()
                            .text(state_text)
                            .font_size(13.)
                            .color(colors::fg_secondary()),
                    ),
            )
            .into_element()
    }
}

#[derive(PartialEq)]
struct AuthGuidancePreview;

impl Component for AuthGuidancePreview {
    fn render(&self) -> impl IntoElement {
        let samples = preview_samples();
        // Index 0 is the no-error state the rest line up with `samples`
        let mut choice = use_state(|| 0usize);
        let mut open = use_state(|| false);

        let mut options: Vec<String> = vec!["No error (in progress)".to_string()];
        options.extend(samples.iter().map(|s| s.label.to_string()));

        let index = (*choice.read()).min(options.len() - 1);
        let selected_label = options[index].clone();

        let popup = open().then(|| {
            let (error, guidance) = match index.checked_sub(1) {
                None => (None, None),
                Some(i) => {
                    let s = &samples[i];
                    (Some(s.message.clone()), s.guidance.clone())
                }
            };
            login_dialog(
                "https://login.live.com/oauth20_authorize (preview)".to_string(),
                "ABCD-EFGH".to_string(),
                "https://www.microsoft.com/link".to_string(),
                false,
                None,
                error,
                guidance,
                move || open.set(false),
            )
        });

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(10.)
            .child(
                label()
                    .text("Opens the real Microsoft sign-in popup exactly as users see it, in the chosen error state.")
                    .font_size(13.)
                    .color(colors::fg_secondary()),
            )
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .cross_align(Alignment::Center)
                    .spacing(12.)
                    .child(
                        Dropdown::new(selected_label, options)
                            .width(Size::px(300.))
                            .height(Size::px(34.))
                            .on_select(move |idx: usize| choice.set(idx)),
                    )
                    .child(
                        Button::new()
                            .primary()
                            .child(Icon::new(IconType::Eye).size(16.))
                            .text("Open preview")
                            .on_press(move |_| open.set(true)),
                    ),
            )
            .maybe_child(popup)
            .into_element()
    }
}

/// Presets are grouped by which layer should catch them same-length corruption only the hashing pass finds truncation the launch-time size check
#[derive(PartialEq)]
struct CorruptionSimulator;

/// `(label, icon, what it does, which check is meant to catch it)`
type DamagePreset = (&'static str, IconType, DamageKind, &'static str);

#[derive(Clone, Copy, PartialEq, Eq)]
enum DamageKind {
    Assets(usize, Damage),
    Libraries(usize, Damage),
    Content(usize, Damage),
    DeleteAssetsTree,
}

const DAMAGE_PRESETS: [DamagePreset; 6] = [
    (
        "Corrupt 25 assets",
        IconType::AlertTriangle,
        DamageKind::Assets(25, Damage::Corrupt),
        "Survives launch; only Verify Files finds it",
    ),
    (
        "Truncate 25 assets",
        IconType::FileX02,
        DamageKind::Assets(25, Damage::Truncate),
        "Caught at launch by the size check",
    ),
    (
        "Delete 25 assets",
        IconType::Trash01,
        DamageKind::Assets(25, Damage::Delete),
        "Caught at launch, re-downloaded",
    ),
    (
        "Corrupt 5 libraries",
        IconType::AlertCircle,
        DamageKind::Libraries(5, Damage::Corrupt),
        "Crashes with ZipException, then offers repair",
    ),
    (
        "Corrupt 5 mods",
        IconType::File02,
        DamageKind::Content(5, Damage::Corrupt),
        "Repaired from the provider, not Mojang",
    ),
    (
        "Delete the assets tree",
        IconType::Folder,
        DamageKind::DeleteAssetsTree,
        "The reported bug; auto-repairs on launch",
    ),
];

impl Component for CorruptionSimulator {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let cluster_id = use_state(|| "1".to_string());

        let mut rows = Vec::new();
        for chunk in DAMAGE_PRESETS.chunks(3) {
            let mut row = rect().horizontal().width(Size::fill()).spacing(8.);

            for (text, icon, kind, hint) in chunk {
                let dispatch = dispatch.clone();
                let kind = *kind;
                let hint = *hint;

                row = row.child(
                    rect()
                        .vertical()
                        .width(Size::flex(1.0))
                        .spacing(2.)
                        .child(
                            Button::new()
                                .danger()
                                .width(Size::fill())
                                .child(Icon::new(*icon).size(16.))
                                .text(*text)
                                .on_press(move |_| {
                                    let target =
                                        cluster_id.read().trim().parse::<i64>().unwrap_or(1);
                                    run_damage(&dispatch, kind, target);
                                }),
                        )
                        .child(
                            label()
                                .text(hint)
                                .font_size(11.)
                                .max_lines(1)
                                .color(colors::fg_secondary()),
                        ),
                );
            }

            rows.push(row.into_element());
        }

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(10.)
            .child(
                label()
                    .text("Damages the real installation so the repair paths can be exercised. Everything here is repairable by \"Verify Files\" in cluster settings, or by launching - which is the point.")
                    .font_size(13.)
                    .color(colors::fg_secondary()),
            )
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .cross_align(Alignment::Center)
                    .spacing(12.)
                    .child(
                        label()
                            .text("Cluster (for mods)")
                            .font_size(13.)
                            .color(colors::fg_secondary()),
                    )
                    .child(
                        rect()
                            .width(Size::px(120.))
                            .child(TextInput::new(cluster_id).placeholder("Cluster ID")),
                    ),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .spacing(8.)
                    .children(rows),
            )
            .child(
                label()
                    .text("Modals: the real prompts, built by the code that raises them for real.")
                    .font_size(13.)
                    .color(colors::fg_secondary()),
            )
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .spacing(12.)
                    .child(prompt_button(
                        "Failed assets prompt",
                        IconType::AlertTriangle,
                        3,
                        0,
                    ))
                    .child(prompt_button(
                        "Failed libraries prompt",
                        IconType::AlertCircle,
                        0,
                        2,
                    ))
                    .child(prompt_button("Mixed failures prompt", IconType::File02, 4, 1)),
            )
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .spacing(12.)
                    .child(crash_repair_button(
                        "Crash repair prompt (named jar)",
                        Some("fabric-loader-0.15.11.jar"),
                        cluster_id,
                    ))
                    .child(crash_repair_button(
                        "Crash repair prompt (no jar named)",
                        None,
                        cluster_id,
                    )),
            )
            .into_element()
    }
}

/// Calls the same function the download path calls so the simulated modal cannot drift from the real one
fn prompt_button(
    text: &'static str,
    icon: IconType,
    failed_assets: usize,
    failed_libraries: usize,
) -> impl IntoElement {
    Button::new()
        .secondary()
        .child(Icon::new(icon).size(16.))
        .text(text)
        .on_press(move |_| {
            spawn(async move {
                let Ok(state) = crate::launcher::state() else {
                    return;
                };
                let answer = oneclient_core::game::confirm_incomplete_install(
                    &state.services.mc(),
                    failed_assets,
                    failed_libraries,
                )
                .await;
                tracing::info!(?answer, "incomplete install prompt answered");
            });
        })
}

/// Accepting really runs the verify pass
/// To test detection itself corrupt libraries above and launch the game
fn crash_repair_button(
    text: &'static str,
    jar: Option<&'static str>,
    cluster_id: State<String>,
) -> impl IntoElement {
    Button::new()
        .secondary()
        .child(Icon::new(IconType::AlertCircle).size(16.))
        .text(text)
        .on_press(move |_| {
            let target = cluster_id.read().trim().parse::<i64>().unwrap_or(1);
            spawn(async move {
                let Ok(state) = crate::launcher::state() else {
                    return;
                };
                let diagnosis = oneclient_core::game::CrashDiagnosis::CorruptArchive {
                    file: jar.map(str::to_string),
                };
                oneclient_core::game::offer_repair(&state, target, &diagnosis).await;
            });
        })
}

fn run_damage(dispatch: &crate::Actions, kind: DamageKind, cluster_id: i64) {
    let dispatch = dispatch.clone();
    spawn(async move {
        let result = match kind {
            DamageKind::Assets(count, damage) => oneclient_core::simulate::damage_assets(count, damage)
                .await
                .map(|report| (report, damage.verb())),
            DamageKind::Libraries(count, damage) => {
                oneclient_core::simulate::damage_libraries(count, damage)
                    .await
                    .map(|report| (report, damage.verb()))
            }
            DamageKind::Content(count, damage) => match crate::launcher::state() {
                Ok(state) => oneclient_core::simulate::damage_cluster_content(
                    &state, cluster_id, count, damage,
                )
                .await
                .map(|report| (report, damage.verb())),
                Err(err) => Err(err),
            },
            DamageKind::DeleteAssetsTree => oneclient_core::simulate::delete_assets_tree()
                .await
                .map(|report| (report, "Deleted")),
        };

        match result {
            Ok((report, verb)) => {
                dispatch
                    .notify("Damage simulated")
                    .body(report.summary(verb))
                    .icon(IconType::AlertTriangle)
                    .error()
                    .send();
            }
            Err(err) => {
                dispatch
                    .notify("Simulation failed")
                    .body(err.to_string())
                    .error()
                    .send();
            }
        }
    });
}

#[derive(PartialEq)]
struct SqlConsole;

impl Component for SqlConsole {
    fn render(&self) -> impl IntoElement {
        let query = use_state(|| "SELECT * FROM clusters;".to_string());
        let result = use_state(|| None::<Result<ConsoleQueryResult, String>>);
        let running = use_state(|| false);

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(10.)
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .cross_align(Alignment::Center)
                    .spacing(12.)
                    .child(
                        rect().width(Size::flex(1.0)).child(
                            TextInput::new(query)
                                .placeholder("SELECT * FROM …")
                                .on_submit(move |_| run_sql(query, result, running)),
                        ),
                    )
                    .child(
                        Button::new()
                            .primary()
                            .child(Icon::new(IconType::Terminal).size(16.))
                            .text(if *running.read() { "Running…" } else { "Run" })
                            .on_press(move |_| run_sql(query, result, running)),
                    ),
            )
            .child(sql_result(&result.read()))
            .into_element()
    }
}

fn run_sql(query: State<String>, mut result: State<SqlResult>, mut running: State<bool>) {
    if *running.read() {
        return;
    }
    let sql = query.read().clone();
    if sql.trim().is_empty() {
        return;
    }

    running.set(true);
    spawn(async move {
        let res = match crate::launcher::state() {
            Ok(state) => run_console_query(&state.services.db, &sql)
                .await
                .map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        };
        result.set(Some(res));
        running.set(false);
    });
}

fn sql_result(state: &SqlResult) -> Element {
    match state {
        None => label()
            .text("No query run yet.")
            .font_size(13.)
            .color(colors::fg_secondary())
            .into_element(),
        Some(Err(err)) => rect()
            .width(Size::fill())
            .padding(Gaps::new_symmetric(10., 14.))
            .corner_radius(CornerRadius::new_all(8.))
            .background(colors::component_bg())
            .border(border_all_color(1., colors::danger()))
            .child(
                label()
                    .text(err.clone())
                    .font_size(13.)
                    .color(colors::danger()),
            )
            .into_element(),
        Some(Ok(res)) if !res.is_select => label()
            .text(format!("OK, {} row(s) affected.", res.rows_affected))
            .font_size(13.)
            .color(colors::success())
            .into_element(),
        Some(Ok(res)) => sql_table(res),
    }
}

fn sql_table(res: &ConsoleQueryResult) -> Element {
    if res.columns.is_empty() {
        return label()
            .text("0 rows returned.")
            .font_size(13.)
            .color(colors::fg_secondary())
            .into_element();
    }

    let header = table_row(&res.columns, true);
    let mut table = rect()
        .vertical()
        .width(Size::fill())
        .corner_radius(CornerRadius::new_all(8.))
        .border(border_all_color(1., colors::component_border()))
        .child(header);

    for row in &res.rows {
        table = table.child(table_row(row, false));
    }

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(6.)
        .child(
            label()
                .text(format!("{} row(s)", res.rows.len()))
                .font_size(12.)
                .color(colors::fg_secondary()),
        )
        .child(table)
        .into_element()
}

fn table_row(cells: &[String], header: bool) -> Element {
    let mut row = rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .background(if header {
            colors::page_elevated()
        } else {
            colors::component_bg()
        });

    for cell in cells {
        row = row.child(
            rect()
                .width(Size::flex(1.0))
                .padding(Gaps::new_symmetric(6., 10.))
                .border(border_all_color(0.5, colors::component_border()))
                .child(
                    label()
                        .text(cell.clone())
                        .font_size(12.)
                        .max_lines(1)
                        .font_weight(if header {
                            FontWeight::SEMI_BOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .color(if header {
                            colors::fg_primary()
                        } else {
                            colors::fg_secondary()
                        }),
                ),
        );
    }

    row.into_element()
}

fn split_csv(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn divider() -> impl IntoElement {
    crate::ui::divider()
        .margin(Gaps::new_symmetric(8., 0.))
        .corner_radius(CornerRadius::new_all(1.))
        .into_element()
}

fn section(title: &'static str, rows: Vec<Element>) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .child(
            label()
                .text(title)
                .font_size(22.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        )
        .children(rows)
        .into_element()
}

fn toggle_row(title: &'static str, on: State<bool>) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .main_align(Alignment::SpaceBetween)
        .padding(Gaps::new_symmetric(10., 14.))
        .corner_radius(CornerRadius::new_all(10.))
        .background(colors::page_elevated())
        .child(
            label()
                .text(title)
                .font_size(14.)
                .color(colors::fg_primary()),
        )
        .child(toggle(on))
        .into_element()
}

fn action_row(buttons: Vec<(&'static str, IconType)>) -> Element {
    let mut row = rect().horizontal().width(Size::fill()).spacing(12.);

    for (text, icon) in buttons {
        let mut button = Button::new()
            .secondary()
            .child(Icon::new(icon).size(16.))
            .text(text);

        if text == "Open Onboarding" {
            button = button.on_press(|_| {
                let _ = RouterContext::get().replace(Route::OnboardingWelcome {});
            });
        }

        row = row.child(button);
    }

    row.into_element()
}
