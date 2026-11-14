use std::path::PathBuf;

use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_common::GameLoader;
use oneclient_core::GameVersionKind;
use oneclient_core::servers::{self, DEFAULT_MEMORY_MB, DEFAULT_PORT, LocalServer, ServerKind};

use crate::Route;
use crate::components::{
    Button, Icon, IconType, ScrollArea, TextInput, centered_spinner, checkbox_labeled,
    memory_field, validate_number,
};
use crate::hooks::{
    game_versions, java_majors, settled_or_loading, use_clusters, use_dispatch, use_game_versions,
    use_java_majors, use_version_loaders,
};
use crate::i18n::{t, tf};
use crate::launcher::off_ui;
use crate::servers::{self as runtime, Prepare};
use crate::theme::{colors, text_size};
use crate::ui::fixed_grid;
use crate::view::app::clusters::{
    CellCard, LoaderChoice, Rail, Shell, VersionRow, WideCard, cell_card, facts_card, field,
    loader_mark, rail, shell, version_row, wide_card,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Type,
    Instance,
    Pack,
    Folder,
    Version,
    Loader,
    Details,
}

fn steps(kind: ServerKind) -> &'static [Step] {
    match kind {
        ServerKind::FromInstance => &[Step::Type, Step::Instance, Step::Details],
        ServerKind::Scratch => &[Step::Type, Step::Version, Step::Loader, Step::Details],
        ServerKind::Modpack => &[Step::Type, Step::Pack, Step::Details],
        ServerKind::Import => &[
            Step::Type,
            Step::Folder,
            Step::Version,
            Step::Loader,
            Step::Details,
        ],
    }
}

fn kind_label(kind: ServerKind) -> &'static str {
    match kind {
        ServerKind::FromInstance => t("oneclient.servers.create.type.instance.title"),
        ServerKind::Scratch => t("oneclient.servers.create.type.scratch.title"),
        ServerKind::Modpack => t("oneclient.servers.create.type.modpack.title"),
        ServerKind::Import => t("oneclient.servers.create.type.import.title"),
    }
}

/// What the picked source decides on its own
#[derive(Clone, PartialEq)]
struct Source {
    mc_version: String,
    loader: GameLoader,
    loader_version: Option<String>,
    name: String,
    game_dir: Option<PathBuf>,
}

#[derive(Clone, Copy, PartialEq)]
struct Form {
    step: State<usize>,
    kind: State<ServerKind>,
    name: State<String>,
    version: State<Option<String>>,
    loader: State<LoaderChoice>,
    cluster: State<Option<i64>>,
    pack: State<Option<PathBuf>>,
    folder: State<Option<PathBuf>>,
    source: State<Option<Source>>,
    memory: State<String>,
    port: State<String>,
    eula: State<bool>,
    query: State<String>,
}

#[derive(PartialEq)]
pub struct CreateServerModal {
    on_close: EventHandler<()>,
}

impl CreateServerModal {
    pub fn new(on_close: impl Into<EventHandler<()>>) -> Self {
        Self {
            on_close: on_close.into(),
        }
    }
}

impl Component for CreateServerModal {
    fn render(&self) -> impl IntoElement {
        let form = Form {
            step: use_state(|| 0),
            kind: use_state(|| ServerKind::FromInstance),
            name: use_state(String::new),
            version: use_state(|| None),
            loader: use_state(|| LoaderChoice::Fabric),
            cluster: use_state(|| None),
            pack: use_state(|| None),
            folder: use_state(|| None),
            source: use_state(|| None),
            memory: use_state(|| DEFAULT_MEMORY_MB.to_string()),
            port: use_state(|| DEFAULT_PORT.to_string()),
            eula: use_state(|| false),
            query: use_state(String::new),
        };
        let clusters = settled_or_loading(&use_clusters()).unwrap_or_default();
        let station = use_dispatch().station();
        // The create task outlives this modal, so the router has to be taken while it renders
        let router = RouterContext::get();

        let kind = *form.kind.read();
        let order = steps(kind);
        let index = (*form.step.read()).min(order.len() - 1);
        let step = order[index];
        let last = index + 1 == order.len();

        let available = settled_or_loading(&use_version_loaders(
            form.version.read().clone().unwrap_or_default(),
        ))
        .unwrap_or_default();

        // A picked instance or pack fixes the version and loader
        let source = form.source.read().clone();
        let (mc_version, loader, loader_version) = match (&source, kind) {
            (Some(src), ServerKind::FromInstance | ServerKind::Modpack) => (
                Some(src.mc_version.clone()),
                Some(src.loader),
                src.loader_version.clone(),
            ),
            _ => {
                let choice = *form.loader.read();
                (
                    form.version.read().clone(),
                    Some(choice.resolve(&available).unwrap_or(choice.primary())),
                    None,
                )
            }
        };

        let java = use_java_majors(
            mc_version
                .clone()
                .map(|v| vec![(v, GameLoader::Vanilla)])
                .unwrap_or_default(),
        );
        let java_major = mc_version
            .as_ref()
            .and_then(|v| java_majors(&java).get(v).copied());

        let name = form.name.read().trim().to_string();
        let port: Option<u16> = form.port.read().trim().parse().ok().filter(|p| *p > 0);
        let memory: Option<u32> = match form.memory.read().trim() {
            "" => Some(DEFAULT_MEMORY_MB),
            raw => raw.parse().ok().filter(|m| *m >= 512),
        };

        let ready = match step {
            Step::Type => true,
            Step::Instance | Step::Pack => source.is_some(),
            Step::Folder => form.folder.read().is_some(),
            Step::Version => form.version.read().is_some(),
            Step::Loader => true,
            Step::Details => {
                !name.is_empty() && port.is_some() && memory.is_some() && *form.eula.read()
            }
        };

        let dash = super::DASH.to_string();
        let world = match kind {
            ServerKind::Modpack => t("oneclient.servers.create.world.pack"),
            ServerKind::Import => t("oneclient.servers.create.world.folder"),
            _ => t("oneclient.servers.create.world.new"),
        };
        let rail_title = if name.is_empty() {
            t("oneclient.servers.create.untitled").to_string()
        } else {
            name.clone()
        };
        let art = match mc_version
            .as_deref()
            .and_then(oneclient_common::parse_mc_version)
        {
            Some(parsed) => {
                crate::components::DynamicArt::for_version(parsed.major, parsed.key(), loader)
            }
            None => crate::components::DynamicArt::fallback(),
        };
        let rail = rail(Rail {
            art,
            title: rail_title,
            subtitle: t("oneclient.servers.local").to_string(),
            card: facts_card(vec![
                (
                    t("oneclient.servers.facts.type"),
                    kind_label(kind).to_string(),
                ),
                (
                    t("oneclient.servers.facts.version"),
                    mc_version.clone().unwrap_or(dash.clone()),
                ),
                (
                    t("oneclient.servers.facts.loader"),
                    if step == Step::Type && kind != ServerKind::Scratch {
                        dash.clone()
                    } else {
                        loader.map(|l| l.to_string()).unwrap_or(dash.clone())
                    },
                ),
                (
                    t("oneclient.servers.facts.java"),
                    java_major.map(|j| j.to_string()).unwrap_or(dash.clone()),
                ),
                (t("oneclient.servers.facts.world"), world.to_string()),
                (
                    t("oneclient.servers.facts.memory"),
                    memory
                        .map(crate::utils::format_memory_gb)
                        .unwrap_or(dash.clone()),
                ),
                (
                    t("oneclient.servers.facts.address"),
                    format!("localhost:{}", port.unwrap_or(DEFAULT_PORT)),
                ),
            ]),
            tags: Vec::new(),
        });

        let (title, subtitle) = match step {
            Step::Type => (
                t("oneclient.servers.create.type.title"),
                t("oneclient.servers.create.type.subtitle"),
            ),
            Step::Instance => (
                t("oneclient.servers.create.instance.title"),
                t("oneclient.servers.create.instance.subtitle"),
            ),
            Step::Pack => (
                t("oneclient.servers.create.pack.title"),
                t("oneclient.servers.create.pack.subtitle"),
            ),
            Step::Folder => (
                t("oneclient.servers.create.folder.title"),
                t("oneclient.servers.create.folder.subtitle"),
            ),
            Step::Version => (
                t("oneclient.servers.create.version.title"),
                t("oneclient.servers.create.version.subtitle"),
            ),
            Step::Loader => (
                t("oneclient.servers.create.loader.title"),
                t("oneclient.servers.create.loader.subtitle"),
            ),
            Step::Details => (
                t("oneclient.servers.create.details.title"),
                t("oneclient.servers.create.details.subtitle"),
            ),
        };
        let note = match (step, kind) {
            (Step::Type, ServerKind::FromInstance) => t("oneclient.servers.create.note.instance"),
            (Step::Type, ServerKind::Scratch) => t("oneclient.servers.create.note.scratch"),
            (Step::Type, ServerKind::Modpack) => t("oneclient.servers.create.note.modpack"),
            (Step::Type, ServerKind::Import) => t("oneclient.servers.create.note.import"),
            (Step::Details, _) => t("oneclient.servers.create.note.details"),
            _ => "",
        };

        let body = match step {
            Step::Type => type_step(form),
            Step::Instance => instance_step(form, &clusters),
            Step::Pack => pick_step(form, false),
            Step::Folder => pick_step(form, true),
            Step::Version => VersionStep { form }.into_element(),
            Step::Loader => LoaderStep {
                form,
                version: form.version.read().clone().unwrap_or_default(),
            }
            .into_element(),
            Step::Details => details_step(form),
        };

        let close_x = self.on_close.clone();
        let close_back = self.on_close.clone();
        let close_done = self.on_close.clone();
        let mut step_state = form.step;

        shell(Shell {
            rail,
            eyebrow: None,
            title: title.to_string(),
            subtitle: subtitle.to_string(),
            body,
            scrolls_itself: step == Step::Version,
            note: note.to_string(),
            secondary_label: if index == 0 {
                t("oneclient.common.cancel")
            } else {
                t("oneclient.common.back")
            }
            .to_string(),
            primary_label: if last {
                t("oneclient.servers.create.create")
            } else {
                t("oneclient.common.next")
            }
            .to_string(),
            primary_enabled: ready,
            on_close: (move |()| close_x.call(())).into(),
            on_secondary: (move |()| {
                if index == 0 {
                    close_back.call(());
                } else {
                    step_state.set(index - 1);
                }
            })
            .into(),
            on_primary: (move |()| {
                if !last {
                    step_state.set(index + 1);
                    return;
                }
                let (Some(mc_version), Some(loader), Some(port), Some(memory)) =
                    (mc_version.clone(), loader, port, memory)
                else {
                    return;
                };
                let mut server = LocalServer::new(name.clone(), kind, mc_version, loader);
                server.loader_version = loader_version.clone();
                server.memory_mb = memory;
                server.eula = true;
                server.source_cluster = *form.cluster.read();
                let prepare = match kind {
                    ServerKind::FromInstance => form
                        .cluster
                        .read()
                        .zip(form.source.read().as_ref().and_then(|s| s.game_dir.clone()))
                        .map_or(Prepare::None, |(id, dir)| Prepare::Instance(id, dir)),
                    ServerKind::Modpack => form
                        .pack
                        .read()
                        .clone()
                        .map_or(Prepare::None, Prepare::Modpack),
                    ServerKind::Import => form
                        .folder
                        .read()
                        .clone()
                        .map_or(Prepare::None, Prepare::Import),
                    ServerKind::Scratch => Prepare::None,
                };
                close_done.call(());
                spawn_in_window(async move {
                    let name = server.name.clone();
                    match off_ui(async move { servers::allocate_id(&name).await }).await {
                        Ok(id) => {
                            server.id = id.clone();
                            runtime::install(station, server, prepare, port);
                            let _ = router.push(Route::ServerOverview { server_id: id });
                        }
                        Err(err) => {
                            if let Ok(state) = crate::launcher::state() {
                                state
                                    .services
                                    .events
                                    .notify(t("oneclient.servers.install_failed"))
                                    .body(err.to_string())
                                    .error()
                                    .send();
                            }
                        }
                    }
                });
            })
            .into(),
        })
    }
}

fn type_step(form: Form) -> Element {
    let cards = [
        (
            ServerKind::FromInstance,
            IconType::IconLogo,
            Some(t("oneclient.clusters.instance.steps.recommended").to_string()),
            t("oneclient.servers.create.type.instance.blurb"),
        ),
        (
            ServerKind::Scratch,
            IconType::Sliders04,
            None,
            t("oneclient.servers.create.type.scratch.blurb"),
        ),
        (
            ServerKind::Modpack,
            IconType::DownloadCloud02,
            None,
            t("oneclient.servers.create.type.modpack.blurb"),
        ),
        (
            ServerKind::Import,
            IconType::FolderDownload,
            None,
            t("oneclient.servers.create.type.import.blurb"),
        ),
    ];
    let selected = *form.kind.read();

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .children(cards.into_iter().map(|(kind, icon, badge, blurb)| {
            let mut kind_state = form.kind;
            let mut source = form.source;
            wide_card(WideCard {
                icon,
                title: kind_label(kind).to_string(),
                badge,
                blurb: blurb.to_string(),
                meta: String::new(),
                selected: selected == kind,
                on_press: (move |()| {
                    if *kind_state.peek() != kind {
                        kind_state.set(kind);
                        source.set(None);
                    }
                })
                .into(),
                trailing: None,
            })
        }))
        .into_element()
}

fn instance_step(form: Form, clusters: &[oneclient_core::clusters::Cluster]) -> Element {
    if clusters.is_empty() {
        return crate::ui::centered_note(t("oneclient.servers.create.instance.empty"));
    }
    let chosen = *form.cluster.read();
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(4.)
        .children(clusters.iter().map(|cluster| {
            let supported = servers::supports_loader(cluster.mc_loader);
            let row = VersionRow {
                id: cluster.name.clone(),
                meta: format!("{} {}", cluster.mc_loader, cluster.mc_version),
                badge: (!supported)
                    .then(|| t("oneclient.servers.create.instance.unsupported").to_string()),
                date: String::new(),
                loader: Some(cluster.mc_loader),
            };
            let picked = Source {
                mc_version: cluster.mc_version.clone(),
                loader: cluster.mc_loader,
                loader_version: cluster.mc_loader_version.clone(),
                name: cluster.name.clone(),
                game_dir: cluster.game_dir().ok(),
            };
            let id = cluster.id;
            let (mut cluster_state, mut source, mut name) = (form.cluster, form.source, form.name);
            version_row(
                &row,
                chosen == Some(id),
                (move |()| {
                    if !supported {
                        return;
                    }
                    cluster_state.set(Some(id));
                    if name.peek().trim().is_empty() {
                        name.set(format!("{} Server", picked.name));
                    }
                    source.set(Some(picked.clone()));
                })
                .into(),
                None,
            )
        }))
        .into_element()
}

fn pick_step(form: Form, folder: bool) -> Element {
    let chosen = if folder {
        form.folder.read().clone()
    } else {
        form.pack.read().clone()
    };
    let (mut pack, mut folder_state, mut source, mut name) =
        (form.pack, form.folder, form.source, form.name);

    let pick = move |_| {
        spawn(async move {
            let dialog = rfd::AsyncFileDialog::new();
            if folder {
                if let Some(handle) = dialog.pick_folder().await {
                    folder_state.set(Some(handle.path().to_path_buf()));
                }
                return;
            }
            let Some(handle) = dialog
                .add_filter(t("oneclient.servers.create.pack.filter"), &["mrpack"])
                .pick_file()
                .await
            else {
                return;
            };
            let path = handle.path().to_path_buf();
            pack.set(Some(path.clone()));
            let read = path.clone();
            match off_ui(async move { servers::read_mrpack(&read).await }).await {
                Ok(info) => {
                    if name.peek().trim().is_empty() {
                        name.set(info.name.clone());
                    }
                    source.set(Some(Source {
                        mc_version: info.mc_version,
                        loader: info.loader,
                        loader_version: info.loader_version,
                        name: info.name,
                        game_dir: None,
                    }));
                }
                Err(err) => {
                    source.set(None);
                    if let Ok(state) = crate::launcher::state() {
                        state
                            .services
                            .events
                            .notify(t("oneclient.servers.create.pack.invalid"))
                            .body(err.to_string())
                            .error()
                            .send();
                    }
                }
            }
        });
    };

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(14.)
        .child(
            Button::new()
                .secondary()
                .on_press(pick)
                .child(
                    Icon::new(if folder {
                        IconType::Folder
                    } else {
                        IconType::File02
                    })
                    .size(16.),
                )
                .text(if folder {
                    t("oneclient.servers.create.folder.choose")
                } else {
                    t("oneclient.servers.create.pack.choose")
                }),
        )
        .maybe_child(chosen.map(|path| {
            label()
                .text(path.display().to_string())
                .font_size(text_size(13.))
                .color(colors::fg_secondary())
                .into_element()
        }))
        .into_element()
}

#[derive(PartialEq)]
struct VersionStep {
    form: Form,
}

impl Component for VersionStep {
    fn render(&self) -> impl IntoElement {
        let form = self.form;
        let versions = game_versions(&use_game_versions());
        let needle = form.query.read().trim().to_lowercase();
        let chosen = form.version.read().clone();

        let Some(versions) = versions else {
            return centered_spinner(t("oneclient.servers.create.version.loading"));
        };
        let rows: Vec<Element> = versions
            .iter()
            .filter(|v| v.kind == GameVersionKind::Release && v.id.to_lowercase().contains(&needle))
            .map(|v| {
                let row = VersionRow {
                    id: v.id.clone(),
                    meta: String::new(),
                    badge: None,
                    date: v.released.clone(),
                    loader: None,
                };
                let id = v.id.clone();
                let mut version = form.version;
                version_row(
                    &row,
                    chosen.as_deref() == Some(v.id.as_str()),
                    (move |()| version.set(Some(id.clone()))).into(),
                    None,
                )
            })
            .collect();

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .spacing(10.)
            .child(
                TextInput::new(form.query)
                    .width(Size::fill())
                    .placeholder(t("oneclient.servers.create.version.search"))
                    .leading(
                        Icon::new(IconType::SearchMd)
                            .size(16.)
                            .color(colors::fg_secondary())
                            .into_element(),
                    ),
            )
            .child(
                ScrollArea::new()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .scrollbar_gutter(true)
                    .spacing(4.)
                    .children(rows),
            )
            .into_element()
    }
}

#[derive(PartialEq)]
struct LoaderStep {
    form: Form,
    version: String,
}

impl Component for LoaderStep {
    fn render(&self) -> impl IntoElement {
        let form = self.form;
        let available =
            settled_or_loading(&use_version_loaders(self.version.clone())).unwrap_or_default();
        let chosen = *form.loader.read();

        let cards = LoaderChoice::ALL
            .into_iter()
            .map(|choice| {
                let usable =
                    choice == LoaderChoice::Vanilla || choice.resolve(&available).is_some();
                let mut loader = form.loader;
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .opacity(if usable { 1. } else { 0.4 })
                    .child(cell_card(CellCard {
                        id: choice.name(),
                        title: choice.name(),
                        blurb: choice.blurb().to_string(),
                        meta: (!usable)
                            .then(|| t("oneclient.servers.create.loader.unavailable").to_string()),
                        corner: loader_mark(choice, chosen == choice, 22.),
                        selected: chosen == choice,
                        checkbox: false,
                        on_press: (move |()| {
                            if usable {
                                loader.set(choice);
                            }
                        })
                        .into(),
                    }))
                    .into_element()
            })
            .collect();

        fixed_grid(cards, 2, 124., 10.)
    }
}

fn details_step(form: Form) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(18.)
        .child(field(
            t("oneclient.servers.create.details.name"),
            TextInput::new(form.name)
                .width(Size::fill())
                .placeholder(t("oneclient.servers.create.untitled"))
                .into_element(),
        ))
        .child(field(
            t("oneclient.servers.create.details.memory"),
            memory_field(
                form.memory,
                t("oneclient.servers.create.details.memory_default"),
                DEFAULT_MEMORY_MB,
            )
            .into_element(),
        ))
        .child(field(
            t("oneclient.servers.create.details.port"),
            TextInput::new(form.port)
                .width(Size::px(140.))
                .on_validate(validate_number)
                .into_element(),
        ))
        .child(
            checkbox_labeled(form.eula, t("oneclient.servers.create.details.eula")).into_element(),
        )
        .child(
            label()
                .text(tf(
                    "oneclient.servers.create.details.eula_link",
                    &[&"https://aka.ms/MinecraftEULA"],
                ))
                .font_size(text_size(12.))
                .color(colors::fg_secondary()),
        )
        .into_element()
}
