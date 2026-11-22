use std::time::Duration;

use freya::prelude::*;
use oneclient_common::GameLoader;
use oneclient_core::GameVersionKind;
use oneclient_core::servers::{LocalServer, Platform};

use crate::components::{Button, Dropdown, IconType, TextInput, validate_number};
use crate::hooks::{
    game_versions, settled_or_loading, use_debounced_save, use_dispatch, use_game_versions,
    use_platform_available, use_server_hub, use_version_loaders,
};
use crate::i18n::{t, tf};
use crate::servers::{self, ServerStatus};
use crate::view::app::settings::settings_row;

fn row_key(server: &LocalServer, row: &str) -> DiffKey {
    let mut hasher = std::hash::DefaultHasher::new();
    std::hash::Hash::hash(&(&server.id, row), &mut hasher);
    DiffKey::U64(std::hash::Hasher::finish(&hasher))
}

#[derive(PartialEq)]
pub(super) struct NameRow {
    pub server: LocalServer,
}

impl Component for NameRow {
    fn render_key(&self) -> DiffKey {
        row_key(&self.server, "name")
    }

    fn render(&self) -> impl IntoElement {
        let initial = self.server.name.clone();
        let name = use_state(move || initial);
        let id = self.server.id.clone();
        use_debounced_save(
            name.read().clone(),
            Duration::from_millis(600),
            move |next: String| {
                let next = next.trim().to_string();
                if !next.is_empty() {
                    servers::update(id.clone(), move |s| s.name = next);
                }
            },
        );
        settings_row(
            IconType::Pencil01,
            t("oneclient.servers.settings.name.title"),
            t("oneclient.servers.settings.name.description"),
            TextInput::new(name).width(Size::px(220.)),
        )
    }
}

#[derive(PartialEq)]
pub(super) struct VersionRow {
    pub server: LocalServer,
}

impl Component for VersionRow {
    fn render_key(&self) -> DiffKey {
        row_key(&self.server, "version")
    }

    fn render(&self) -> impl IntoElement {
        let server = &self.server;
        let station = use_dispatch().station();
        let stopped = use_server_hub().status(&server.id) == ServerStatus::Stopped;
        let mut picked = use_state(|| None::<String>);
        let mut armed = use_state(|| false);

        let all = game_versions(&use_game_versions()).unwrap_or_default();
        let current = all
            .iter()
            .find(|v| v.id == server.mc_version)
            .map(|v| v.released.clone());
        let mut releases: Vec<(String, String)> = all
            .iter()
            .filter(|v| v.kind == GameVersionKind::Release)
            .map(|v| (v.id.clone(), v.released.clone()))
            .collect();
        releases.sort_by(|a, b| b.1.cmp(&a.1));
        let newer: Vec<String> = releases
            .into_iter()
            .filter(|(_, released)| current.as_ref().is_some_and(|c| released >= c))
            .map(|(id, _)| id)
            .collect();

        let target = picked
            .read()
            .clone()
            .unwrap_or_else(|| server.mc_version.clone());
        let paper = settled_or_loading(&use_platform_available(Platform::Paper, target.clone()));
        let loaders = settled_or_loading(&use_version_loaders(target.clone())).unwrap_or_default();
        let available = match server.platform {
            Platform::Paper => paper == Some(true),
            Platform::Custom => true,
            Platform::Loader => {
                server.loader == GameLoader::Vanilla || loaders.contains(&server.loader)
            }
        };
        let changed = target != server.mc_version;

        let description = if changed && !available {
            tf(
                "oneclient.servers.settings.version.unavailable",
                &[&server.software_name(), &target],
            )
        } else {
            t("oneclient.servers.settings.version.description").to_string()
        };
        let options = newer.clone();
        let (id, chosen) = (server.id.clone(), target.clone());

        settings_row(
            IconType::RefreshCw01,
            t("oneclient.servers.settings.version.title"),
            description,
            rect()
                .horizontal()
                .spacing(8.)
                .cross_align(Alignment::Center)
                .child(
                    Dropdown::new(target, newer)
                        .width(Size::px(130.))
                        .height(Size::px(34.))
                        .on_select(move |index: usize| {
                            if let Some(version) = options.get(index) {
                                picked.set(Some(version.clone()));
                                armed.set(false);
                            }
                        }),
                )
                .child(
                    Button::new()
                        .small()
                        .enabled(stopped && changed && available)
                        .maybe(*armed.read(), |b| b.danger())
                        .maybe(!*armed.read(), |b| b.secondary())
                        .on_press(move |_| {
                            if !*armed.peek() {
                                armed.set(true);
                                return;
                            }
                            armed.set(false);
                            picked.set(None);
                            servers::update_software(station, id.clone(), Some(chosen.clone()));
                        })
                        .text(if *armed.read() {
                            t("oneclient.servers.settings.confirm")
                        } else {
                            t("oneclient.servers.settings.version.change")
                        }),
                ),
        )
    }
}

pub(super) fn update_row(server: &LocalServer, stopped: bool) -> Option<Element> {
    if !server.takes_content() {
        return None;
    }
    let station = use_dispatch().station();
    let id = server.id.clone();
    let installed = server
        .loader_version
        .clone()
        .unwrap_or_else(|| super::DASH.to_string());
    Some(
        settings_row(
            IconType::Download01,
            t("oneclient.servers.settings.update.title"),
            tf(
                "oneclient.servers.settings.update.description",
                &[&server.software_name(), &server.mc_version, &installed],
            ),
            Button::new()
                .small()
                .secondary()
                .enabled(stopped && server.installed())
                .on_press(move |_| servers::update_software(station, id.clone(), None))
                .text(t("oneclient.servers.settings.update.button")),
        )
        .into_element(),
    )
}

pub(super) fn jar_row(server: &LocalServer, stopped: bool) -> Element {
    let station = use_dispatch().station();
    let id = server.id.clone();
    let description = match &server.custom_jar {
        Some(jar) if server.platform == Platform::Custom => {
            tf("oneclient.servers.settings.jar.current", &[jar])
        }
        _ => t("oneclient.servers.settings.jar.description").to_string(),
    };
    settings_row(
        IconType::File02,
        t("oneclient.servers.settings.jar.title"),
        description,
        Button::new()
            .small()
            .secondary()
            .enabled(stopped)
            .on_press(move |_| {
                let id = id.clone();
                spawn(async move {
                    if let Some(file) = rfd::AsyncFileDialog::new()
                        .add_filter(t("oneclient.servers.create.loader.jar.filter"), &["jar"])
                        .pick_file()
                        .await
                    {
                        servers::use_custom_jar(station, id, file.path().to_path_buf());
                    }
                });
            })
            .text(t("oneclient.servers.settings.jar.choose")),
    )
    .into_element()
}

pub(super) fn java_row(server: &LocalServer) -> Element {
    let current = match &server.java_path {
        Some(path) => path.display().to_string(),
        None => tf(
            "oneclient.servers.settings.java.automatic",
            &[&server
                .java_major
                .map_or_else(|| super::DASH.to_string(), |major| major.to_string())],
        ),
    };
    let (pick_id, reset_id, custom) = (
        server.id.clone(),
        server.id.clone(),
        server.java_path.is_some(),
    );
    settings_row(
        IconType::CodeSnippet02,
        t("oneclient.servers.settings.java.title"),
        current,
        rect()
            .horizontal()
            .spacing(8.)
            .child(
                Button::new()
                    .small()
                    .secondary()
                    .on_press(move |_| {
                        let id = pick_id.clone();
                        spawn(async move {
                            let Some(file) = rfd::AsyncFileDialog::new()
                                .set_title(t("oneclient.servers.settings.java.pick"))
                                .pick_file()
                                .await
                            else {
                                return;
                            };
                            let path = file.path().to_path_buf();
                            servers::update(id, move |s| s.java_path = Some(path));
                        });
                    })
                    .text(t("oneclient.servers.settings.java.choose")),
            )
            .child(
                Button::new()
                    .small()
                    .ghost()
                    .enabled(custom)
                    .on_press(move |_| servers::update(reset_id.clone(), |s| s.java_path = None))
                    .text(t("oneclient.servers.settings.java.reset")),
            ),
    )
    .into_element()
}

#[derive(PartialEq)]
pub(super) struct BackupsKeptRow {
    pub server: LocalServer,
}

impl Component for BackupsKeptRow {
    fn render_key(&self) -> DiffKey {
        row_key(&self.server, "backups-kept")
    }

    fn render(&self) -> impl IntoElement {
        let initial = self.server.backups_kept.to_string();
        let kept = use_state(move || initial);
        let id = self.server.id.clone();
        use_debounced_save(
            kept.read().clone(),
            Duration::from_millis(600),
            move |next: String| {
                if let Ok(count) = next.trim().parse::<u32>() {
                    servers::update(id.clone(), move |s| s.backups_kept = count.max(1));
                }
            },
        );
        settings_row(
            IconType::ClockRewind,
            t("oneclient.servers.settings.backups_kept.title"),
            t("oneclient.servers.settings.backups_kept.description"),
            TextInput::new(kept)
                .width(Size::px(90.))
                .on_validate(validate_number),
        )
    }
}
