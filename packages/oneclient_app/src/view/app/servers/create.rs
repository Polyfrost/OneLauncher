use std::path::PathBuf;

use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_common::GameLoader;
use oneclient_core::GameVersionKind;
use oneclient_core::clusters::ModpackSource;
use oneclient_core::servers::{
    self, DEFAULT_MEMORY_MB, DEFAULT_PORT, LocalServer, Platform, ServerKind,
};

use crate::Route;
use crate::components::{
    Button, Icon, IconType, ScrollArea, TextInput, centered_spinner, checkbox_labeled,
    memory_field, validate_number,
};
use crate::hooks::{
    game_versions, java_majors, settled_or_loading, use_clusters, use_dispatch, use_game_versions,
    use_java_majors, use_platform_available, use_version_loaders,
};
use crate::i18n::{t, tf};
use crate::launcher::off_ui;
use crate::servers::{self as runtime, Prepare};
use crate::theme::{colors, text_size};
use crate::ui::fixed_grid;
use crate::view::app::clusters::{
    CellCard, LoaderChoice, ModpackOrigin, Rail, Shell, VersionRow, WideCard, asset_mark,
    cell_card, facts_card, field, loader_mark, rail, shell, version_row, wide_card,
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
    platform: State<Platform>,
    cluster: State<Option<i64>>,
    pack: State<Option<PathBuf>>,
    origin: State<ModpackOrigin>,
    loading: State<bool>,
    icon: State<Option<PathBuf>>,
    folder: State<Option<PathBuf>>,
    jar: State<Option<PathBuf>>,
    source: State<Option<Source>>,
    memory: State<String>,
    port: State<String>,
    eula: State<bool>,
    query: State<String>,
}

#[derive(PartialEq)]
pub struct CreateServerModal {
    on_close: EventHandler<()>,
    picked: Option<ModpackSource>,
}

impl CreateServerModal {
    pub fn new(on_close: impl Into<EventHandler<()>>) -> Self {
        Self {
            on_close: on_close.into(),
            picked: None,
        }
    }

    pub fn picked(mut self, picked: Option<ModpackSource>) -> Self {
        self.picked = picked;
        self
    }
}

impl Component for CreateServerModal {
    fn render(&self) -> impl IntoElement {
        let picked = self.picked.is_some();
        let form = Form {
            step: use_state(|| usize::from(picked)),
            kind: use_state(|| {
                if picked {
                    ServerKind::Modpack
                } else {
                    ServerKind::FromInstance
                }
            }),
            name: use_state(String::new),
            version: use_state(|| None),
            loader: use_state(|| LoaderChoice::Fabric),
            platform: use_state(|| Platform::Loader),
            cluster: use_state(|| None),
            pack: use_state(|| None),
            origin: use_state(|| ModpackOrigin::Browse),
            loading: use_state(|| false),
            icon: use_state(|| None),
            folder: use_state(|| None),
            jar: use_state(|| None),
            source: use_state(|| None),
            memory: use_state(|| DEFAULT_MEMORY_MB.to_string()),
            port: use_state(|| DEFAULT_PORT.to_string()),
            eula: use_state(|| false),
            query: use_state(String::new),
        };
        use_hook({
            let picked = self.picked.clone();
            move || {
                if let Some(source) = picked {
                    load_pack(form, source);
                }
            }
        });
        let mut port_state = form.port;
        use_hook(move || {
            spawn(async move {
                let free = off_ui(servers::free_port()).await;
                if *port_state.peek() == DEFAULT_PORT.to_string() {
                    port_state.set(free.to_string());
                }
            })
        });
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

        let platform = if matches!(kind, ServerKind::Scratch | ServerKind::Import) {
            *form.platform.read()
        } else {
            Platform::Loader
        };
        let platform_ok = settled_or_loading(&use_platform_available(
            platform,
            form.version.read().clone().unwrap_or_default(),
        ))
        .unwrap_or(false);

        // A picked instance or pack fixes the version and loader
        let source = form.source.read().clone();
        let (mc_version, loader, loader_version) = match (&source, kind) {
            (Some(src), ServerKind::FromInstance | ServerKind::Modpack) => (
                Some(src.mc_version.clone()),
                Some(src.loader),
                src.loader_version.clone(),
            ),
            _ if platform != Platform::Loader => {
                (form.version.read().clone(), Some(GameLoader::Vanilla), None)
            }
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
            Step::Instance => source.is_some(),
            Step::Pack => !*form.loading.read(),
            Step::Folder => form.folder.read().is_some(),
            Step::Version => form.version.read().is_some(),
            Step::Loader if platform != Platform::Loader => platform_ok,
            Step::Loader => form.loader.read().resolve(&available).is_some(),
            Step::Details => {
                !name.is_empty() && port.is_some() && memory.is_some() && *form.eula.read()
            }
        };

        let dash = super::none();
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
                    } else if platform != Platform::Loader {
                        platform_name(platform).to_string()
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
            (Step::Details, _) if platform == Platform::Custom => {
                t("oneclient.servers.create.note.details.jar")
            }
            (Step::Details, _) => t("oneclient.servers.create.note.details"),
            _ => "",
        };

        let body = match step {
            Step::Type => type_step(form),
            Step::Instance => instance_step(form, &clusters),
            Step::Pack => pack_step(form),
            Step::Folder => folder_step(form),
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
        let close_browse = self.on_close.clone();
        let pack_action = (step == Step::Pack && source.is_none()).then(|| *form.origin.read());
        let mut step_state = form.step;

        shell(Shell {
            rail,
            eyebrow: None,
            title: title.to_string(),
            subtitle: subtitle.to_string(),
            body,
            scrolls_itself: matches!(step, Step::Version | Step::Details),
            note: note.to_string(),
            secondary_label: if index == 0 {
                t("oneclient.common.cancel")
            } else {
                t("oneclient.common.back")
            }
            .to_string(),
            primary_label: match pack_action {
                Some(ModpackOrigin::Browse) => {
                    t("oneclient.clusters.instance.create.browse_modpacks")
                }
                Some(ModpackOrigin::File) => t("oneclient.servers.create.pack.choose"),
                None if last => t("oneclient.servers.create.create"),
                None => t("oneclient.common.next"),
            }
            .to_string(),
            primary_enabled: ready,
            primary_busy: false,
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
                match pack_action {
                    Some(ModpackOrigin::Browse) => {
                        close_browse.call(());
                        let _ = router.push(Route::Browser {
                            cluster_id: crate::view::app::browser::SERVER_PACKS,
                            package_type: "modpack".to_string(),
                            pick_cluster: false,
                        });
                        return;
                    }
                    Some(ModpackOrigin::File) => {
                        spawn(async move {
                            if let Some(handle) = rfd::AsyncFileDialog::new()
                                .add_filter(t("oneclient.servers.create.pack.filter"), &["mrpack", "zip"])
                                .pick_file()
                                .await
                            {
                                load_pack(form, ModpackSource::File(handle.path().to_path_buf()));
                            }
                        });
                        return;
                    }
                    None => {}
                }
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
                server.platform = platform;
                server.memory_mb = memory;
                server.eula = true;
                server.source_cluster = (kind == ServerKind::FromInstance)
                    .then(|| *form.cluster.read())
                    .flatten();
                server.source_path = match kind {
                    ServerKind::FromInstance => {
                        form.source.read().as_ref().and_then(|s| s.game_dir.clone())
                    }
                    ServerKind::Modpack => form.pack.read().clone(),
                    ServerKind::Import => form.folder.read().clone(),
                    ServerKind::Scratch => None,
                };
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
                let icon = (kind == ServerKind::Modpack)
                    .then(|| form.icon.read().clone())
                    .flatten();
                let jar = (platform == Platform::Custom)
                    .then(|| form.jar.read().clone())
                    .flatten();
                close_done.call(());
                spawn_in_window(async move {
                    let name = server.name.clone();
                    match off_ui(async move { servers::allocate_id(&name).await }).await {
                        Ok(id) => {
                            server.id = id.clone();
                            if let Some(icon) = icon {
                                let dir = servers::server_dir(&id);
                                let saved = off_ui(async move { servers::set_icon(&dir?, icon).await })
                                    .await;
                                if let Err(err) = saved {
                                    tracing::warn!(%err, "could not use the modpack icon for the server");
                                }
                            }
                            // A failed copy shows up as a failed install, fixable from the settings
                            if let Some(jar) = jar {
                                let (back, copied) = off_ui(async move {
                                    let copied = servers::use_custom_jar(&mut server, &jar).await;
                                    (server, copied)
                                })
                                .await;
                                server = back;
                                if let Err(err) = copied {
                                    tracing::warn!(%err, "could not copy the custom server jar");
                                }
                            }
                            runtime::install(station, server, prepare, Some(port));
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

fn folder_step(form: Form) -> Element {
    let chosen = form.folder.read().clone();
    let mut folder_state = form.folder;
    let (mut version, mut loader, mut platform, mut port) =
        (form.version, form.loader, form.platform, form.port);

    let pick = move |_| {
        spawn(async move {
            let Some(handle) = rfd::AsyncFileDialog::new().pick_folder().await else {
                return;
            };
            let path = handle.path().to_path_buf();
            folder_state.set(Some(path.clone()));
            let guess = off_ui(async move { servers::detect_import(&path).await }).await;
            if guess.mc_version.is_some() {
                version.set(guess.mc_version);
            }
            if guess.platform != Platform::Loader {
                platform.set(guess.platform);
            } else if let Some(found) = guess.loader {
                platform.set(Platform::Loader);
                loader.set(LoaderChoice::of(found));
            }
            if let Some(found) = guess.port {
                port.set(found.to_string());
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
                .child(Icon::new(IconType::Folder).size(16.))
                .text(t("oneclient.servers.create.folder.choose")),
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

fn pack_step(form: Form) -> Element {
    let cards = [
        (
            ModpackOrigin::Browse,
            IconType::Globe01,
            t("oneclient.clusters.instance.steps.modpack.browse.title"),
            t("oneclient.servers.create.pack.browse.blurb"),
            t("oneclient.clusters.instance.steps.modpack.browse.meta"),
        ),
        (
            ModpackOrigin::File,
            IconType::File02,
            t("oneclient.clusters.instance.steps.modpack.file.title"),
            t("oneclient.servers.create.pack.file.blurb"),
            t("oneclient.servers.create.pack.file.meta"),
        ),
    ];
    let selected = *form.origin.read();
    let status = if *form.loading.read() {
        Some(t("oneclient.servers.create.pack.loading").to_string())
    } else {
        form.source
            .read()
            .as_ref()
            .map(|s| tf("oneclient.servers.create.pack.selected", &[&s.name]))
    };

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .children(cards.into_iter().map(|(origin, icon, title, blurb, meta)| {
            let (mut origin_state, mut pack, mut source) = (form.origin, form.pack, form.source);
            let loading = form.loading;
            wide_card(WideCard {
                icon,
                title: title.to_string(),
                badge: None,
                blurb: blurb.to_string(),
                meta: meta.to_string(),
                selected: selected == origin,
                on_press: (move |()| {
                    if *origin_state.peek() != origin && !*loading.peek() {
                        origin_state.set(origin);
                        pack.set(None);
                        source.set(None);
                    }
                })
                .into(),
                trailing: None,
            })
        }))
        .maybe_child(status.map(|text| {
            label()
                .text(text)
                .font_size(text_size(13.))
                .color(colors::fg_secondary())
                .into_element()
        }))
        .into_element()
}

fn load_pack(form: Form, picked: ModpackSource) {
    let (mut pack, mut source, mut name, mut loading, mut icon) =
        (form.pack, form.source, form.name, form.loading, form.icon);
    loading.set(true);
    icon.set(None);
    spawn(async move {
        let result = off_ui(async move {
            let (path, icon) = match picked {
                ModpackSource::File(path) => (path, None),
                ModpackSource::Provider {
                    provider,
                    project_id,
                    version_id,
                } => {
                    let state = crate::launcher::state()?;
                    let path =
                        servers::download_pack(&state, provider, &project_id, &version_id).await?;
                    let icon = servers::pack_icon(&state, provider, &project_id)
                        .await
                        .inspect_err(|err| tracing::debug!(%err, "no icon for the modpack"))
                        .ok()
                        .flatten();
                    (path, icon)
                }
            };
            let info = servers::read_pack(&path).await?;
            Ok::<_, oneclient_core::LauncherError>((path, info, icon))
        })
        .await;
        loading.set(false);
        match result {
            Ok((path, info, found)) => {
                pack.set(Some(path));
                icon.set(found);
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
                pack.set(None);
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
        let platform = *form.platform.read();
        let paper_ok = settled_or_loading(&use_platform_available(
            Platform::Paper,
            self.version.clone(),
        ));

        let mut cards: Vec<Element> = LoaderChoice::ALL
            .into_iter()
            .map(|choice| {
                let usable =
                    choice == LoaderChoice::Vanilla || choice.resolve(&available).is_some();
                let mut loader = form.loader;
                let mut platform_state = form.platform;
                let selected = platform == Platform::Loader && chosen == choice;
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
                        corner: loader_mark(choice, selected, 22.),
                        selected,
                        checkbox: false,
                        on_press: (move |()| {
                            if usable {
                                loader.set(choice);
                                platform_state.set(Platform::Loader);
                            }
                        })
                        .into(),
                    }))
                    .into_element()
            })
            .collect();

        {
            let (choice, availability) = (Platform::Paper, paper_ok);
            let usable = availability == Some(true);
            let mut platform_state = form.platform;
            let selected = platform == choice;
            cards.push(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .opacity(if usable { 1. } else { 0.4 })
                    .child(cell_card(CellCard {
                        id: platform_name(choice).to_string(),
                        title: platform_name(choice).to_string(),
                        blurb: t("oneclient.servers.create.loader.paper").to_string(),
                        meta: match availability {
                            None => Some(t("oneclient.servers.create.loader.checking").to_string()),
                            Some(false) => {
                                Some(t("oneclient.servers.create.loader.unavailable").to_string())
                            }
                            Some(true) => None,
                        },
                        corner: asset_mark("icons/paper.png", selected, 22.),
                        selected,
                        checkbox: false,
                        on_press: (move |()| {
                            if usable {
                                platform_state.set(choice);
                            }
                        })
                        .into(),
                    }))
                    .into_element(),
            );
        }

        {
            let selected = platform == Platform::Custom;
            let picked = form.jar.read().as_ref().and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().to_string())
            });
            let (mut jar, mut platform_state) = (form.jar, form.platform);
            cards.push(cell_card(CellCard {
                id: "custom-jar".to_string(),
                title: platform_name(Platform::Custom).to_string(),
                blurb: t("oneclient.servers.create.loader.jar.blurb").to_string(),
                meta: picked,
                corner: Some(
                    Icon::new(IconType::File02)
                        .size(22.)
                        .color(if selected {
                            colors::fg_primary()
                        } else {
                            colors::fg_secondary()
                        })
                        .into_element(),
                ),
                selected,
                checkbox: false,
                on_press: (move |()| {
                    spawn(async move {
                        if let Some(handle) = rfd::AsyncFileDialog::new()
                            .add_filter(t("oneclient.servers.create.loader.jar.filter"), &["jar"])
                            .pick_file()
                            .await
                        {
                            jar.set(Some(handle.path().to_path_buf()));
                            platform_state.set(Platform::Custom);
                        }
                    });
                })
                .into(),
            }));
        }

        fixed_grid(cards, 2, 124., 10.)
    }
}

fn platform_name(platform: Platform) -> &'static str {
    match platform {
        Platform::Loader => "",
        Platform::Paper => "Paper",
        Platform::Custom => t("oneclient.servers.create.loader.jar.title"),
    }
}

fn details_step(form: Form) -> Element {
    let fields = rect()
        .vertical()
        .width(Size::fill())
        .spacing(18.)
        .padding(Gaps::new(0., 0., 12., 0.))
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
        ));

    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .content(Content::Flex)
        .spacing(12.)
        .child(
            ScrollArea::new()
                .width(Size::fill())
                .height(Size::flex(1.0))
                .scrollbar_gutter(true)
                .child(fields),
        )
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(6.)
                .child(checkbox_labeled(
                    form.eula,
                    t("oneclient.servers.create.details.eula"),
                ))
                .child(
                    label()
                        .text(tf(
                            "oneclient.servers.create.details.eula_link",
                            &[&"https://aka.ms/MinecraftEULA"],
                        ))
                        .font_size(text_size(12.))
                        .color(colors::fg_secondary()),
                ),
        )
        .into_element()
}
