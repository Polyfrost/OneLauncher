use std::path::PathBuf;

use freya::prelude::*;
use oneclient_cluster::naming::{MAX_NAME_CHARS, validate_name};
use oneclient_common::domain::GameLoader;
use oneclient_core::clusters::ClusterKind;

use super::details::{DetailsState, details_body};
use super::rail::{Rail, facts_card, rail, version_art};
use super::shell::{Shell, shell};
use crate::hooks::{ClusterAction, use_cluster_mutation};
use crate::i18n::t;

#[derive(Clone, PartialEq)]
pub struct InstanceFacts {
    pub cluster_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub cover: Option<PathBuf>,
    pub mc_version: String,
    pub mc_loader: GameLoader,
    pub kind: ClusterKind,
    pub modpack: bool,
}

#[derive(PartialEq)]
pub struct EditInstanceModal {
    facts: InstanceFacts,
    on_close: EventHandler<()>,
}

impl EditInstanceModal {
    pub fn new(facts: InstanceFacts, on_close: impl Into<EventHandler<()>>) -> Self {
        Self {
            facts,
            on_close: on_close.into(),
        }
    }
}

fn name_limit(modpack: bool) -> Option<usize> {
    (!modpack).then_some(MAX_NAME_CHARS)
}

fn kind_label(kind: ClusterKind) -> &'static str {
    match kind {
        ClusterKind::OneClient => "OneClient",
        ClusterKind::Vanilla => "Vanilla",
        ClusterKind::Modded => t("oneclient.clusters.instance.edit.kind.modded"),
    }
}

impl Component for EditInstanceModal {
    fn render(&self) -> impl IntoElement {
        let mutation = use_cluster_mutation();
        let cluster_id = self.facts.cluster_id;

        let state = DetailsState::seeded(
            &self.facts.name,
            self.facts.description.as_deref(),
            &self.facts.tags,
        );

        let existing = self.facts.cover.clone();
        let preview = state.preview_cover(existing.clone());
        let tags = state.tags.read().clone();
        let typed = state.typed_name();
        let name_problem = if typed == self.facts.name {
            None
        } else {
            validate_name(&typed, name_limit(self.facts.modpack)).err()
        };
        let ready = !typed.is_empty() && name_problem.is_none();

        let description = state.description.read().trim().to_string();
        let subtitle = if description.is_empty() {
            format!(
                "{} · {}",
                kind_label(self.facts.kind),
                self.facts.mc_version
            )
        } else {
            description
        };

        let title = if typed.is_empty() {
            self.facts.name.clone()
        } else {
            typed.clone()
        };

        let art = version_art(Some(&self.facts.mc_version), Some(self.facts.mc_loader));
        let art = match &preview {
            Some((path, true)) => art.picked_cover(Some(path.clone())),
            Some((path, false)) => art.cover(Some(path.clone())),
            None => art,
        };

        let loader = self.facts.mc_loader.to_string();

        let close_x = self.on_close.clone();
        let close_cancel = self.on_close.clone();
        let close_save = self.on_close.clone();

        shell(Shell {
            rail: rail(Rail {
                art,
                title,
                subtitle,
                card: facts_card(vec![
                    (
                        t("oneclient.clusters.instance.edit.facts.type"),
                        kind_label(self.facts.kind).to_string(),
                    ),
                    (
                        t("oneclient.clusters.instance.edit.facts.version"),
                        self.facts.mc_version.clone(),
                    ),
                    (t("oneclient.clusters.instance.edit.facts.loader"), loader),
                ]),
                tags,
            }),
            eyebrow: Some(t("oneclient.clusters.instance.edit.eyebrow").to_string()),
            title: t("oneclient.clusters.instance.edit.title").to_string(),
            subtitle: t("oneclient.clusters.instance.edit.subtitle").to_string(),
            body: details_body(
                state,
                self.facts.name.clone(),
                existing,
                version_art(Some(&self.facts.mc_version), Some(self.facts.mc_loader)),
                name_problem,
                name_limit(self.facts.modpack),
            ),
            scrolls_itself: false,
            note: t("oneclient.clusters.instance.edit.note").to_string(),
            secondary_label: t("oneclient.common.cancel").to_string(),
            primary_label: t("oneclient.clusters.instance.edit.save").to_string(),
            primary_enabled: ready,
            on_close: (move |()| close_x.call(())).into(),
            on_secondary: (move |()| close_cancel.call(())).into(),
            on_primary: (move |()| {
                mutation.mutate(ClusterAction::UpdateInstance {
                    cluster_id,
                    name: state.typed_name(),
                    description: state.description_value(),
                    tags: state.tags.read().clone(),
                    cover_source: state.cover.read().clone(),
                    clear_cover: *state.cover_cleared.read(),
                });
                close_save.call(());
            })
            .into(),
        })
    }
}
