use freya::prelude::*;
use oneclient_common::domain::GameLoader;
use oneclient_content::packages::ContentType;
use oneclient_core::GameVersionKind;

use super::sidebar::CategoryRow;
use crate::components::{Button, Icon, IconType, ScrollArea};
use crate::hooks::{SearchOverride, game_versions, use_game_versions};
use crate::i18n::t;
use crate::theme::{colors, text_size};
use crate::view::app::cluster::dialog;

const LOADERS: [GameLoader; 3] = [GameLoader::Fabric, GameLoader::Forge, GameLoader::NeoForge];
const LIST_H: f32 = 260.;

pub(crate) fn summary(search_override: &SearchOverride, uses_loader: bool) -> String {
    let version = search_override
        .version
        .clone()
        .unwrap_or_else(|| t("oneclient.browser.override.any_version").to_string());
    if !uses_loader {
        return version;
    }
    let loader = search_override.loader.map_or_else(
        || t("oneclient.browser.index.loader.any").to_string(),
        |l| l.to_string(),
    );
    format!("{version} · {loader}")
}

#[derive(PartialEq)]
pub(super) struct OverrideButton {
    pub(super) content_type: ContentType,
    pub(super) search_override: State<Option<SearchOverride>>,
    pub(super) cluster: Option<(GameLoader, String)>,
}

impl Component for OverrideButton {
    fn render(&self) -> impl IntoElement {
        let mut open = use_state(|| false);
        let current = self.search_override.read().clone();
        let uses_loader = self.content_type == ContentType::Mod;

        let button = Button::new()
            .small()
            .height(Size::px(28.))
            .padding(Gaps::new_symmetric(0., 10.))
            .on_press(move |_| open.set(true))
            .child(
                Icon::new(IconType::Sliders04)
                    .size(12.)
                    .color(colors::fg_primary()),
            );
        let button = match &current {
            Some(o) => button.primary().text(summary(o, uses_loader)),
            None => button
                .secondary()
                .text(t("oneclient.browser.override.button")),
        };

        let initial = current.unwrap_or_else(|| SearchOverride {
            loader: self
                .cluster
                .as_ref()
                .map(|(loader, _)| *loader)
                .filter(|l| LOADERS.contains(l)),
            version: self.cluster.as_ref().map(|(_, version)| version.clone()),
        });

        rect()
            .child(button)
            .maybe_child(open.read().then(|| OverrideModal {
                uses_loader,
                search_override: self.search_override,
                initial,
                open,
            }))
    }
}

#[derive(PartialEq)]
struct OverrideModal {
    uses_loader: bool,
    search_override: State<Option<SearchOverride>>,
    initial: SearchOverride,
    open: State<bool>,
}

impl Component for OverrideModal {
    fn render(&self) -> impl IntoElement {
        let initial = self.initial.clone();
        let draft = use_state(move || initial);
        let mut open = self.open;
        let mut search_override = self.search_override;

        let versions: Vec<String> = game_versions(&use_game_versions())
            .map(|all| {
                all.iter()
                    .filter(|v| v.kind == GameVersionKind::Release)
                    .map(|v| v.id.clone())
                    .collect()
            })
            .unwrap_or_default();

        let loader_rows =
            std::iter::once((None, t("oneclient.browser.index.loader.any").to_string()))
                .chain(LOADERS.iter().map(|l| (Some(*l), l.to_string())))
                .map(move |(value, name)| {
                    let mut draft = draft;
                    CategoryRow {
                        name: name.clone(),
                        selected: draft.read().loader == value,
                        on_toggle: (move |()| draft.write().loader = value).into(),
                        key: DiffKey::None,
                    }
                    .key(name)
                    .into_element()
                });

        let version_rows = std::iter::once((
            None,
            t("oneclient.browser.override.any_version").to_string(),
        ))
        .chain(versions.into_iter().map(|v| (Some(v.clone()), v)))
        .map(move |(value, name)| {
            let mut draft = draft;
            let selected = draft.read().version == value;
            CategoryRow {
                name: name.clone(),
                selected,
                on_toggle: (move |()| draft.write().version = value.clone()).into(),
                key: DiffKey::None,
            }
            .key(name)
            .into_element()
        });

        let control = rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .spacing(12.)
            .maybe_child(self.uses_loader.then(|| {
                column(
                    t("oneclient.browser.override.loaders"),
                    loader_rows.collect(),
                )
            }))
            .child(column(
                t("oneclient.browser.override.versions"),
                version_rows.collect(),
            ))
            .into_element();

        dialog(
            t("oneclient.browser.override.title").to_string(),
            t("oneclient.browser.override.description").to_string(),
            Some(control),
            move || open.set(false),
            [
                Button::new()
                    .secondary()
                    .on_press(move |_| {
                        search_override.set(None);
                        open.set(false);
                    })
                    .text(t("oneclient.common.reset"))
                    .into_element(),
                Button::new()
                    .primary()
                    .on_press(move |_| {
                        search_override.set(Some(draft.read().clone()));
                        open.set(false);
                    })
                    .text(t("oneclient.common.apply"))
                    .into_element(),
            ],
        )
    }
}

fn column(title: &'static str, rows: Vec<Element>) -> Element {
    rect()
        .vertical()
        .width(Size::flex(1.0))
        .spacing(8.)
        .child(
            label()
                .text(title)
                .font_size(text_size(11.))
                .font_weight(FontWeight::MEDIUM)
                .letter_spacing(1.4)
                .color(colors::fg_secondary()),
        )
        .child(
            ScrollArea::new()
                .width(Size::fill())
                .height(Size::px(LIST_H))
                .children([rect()
                    .vertical()
                    .width(Size::fill())
                    .spacing(1.)
                    .children(rows)
                    .into_element()]),
        )
        .into_element()
}
