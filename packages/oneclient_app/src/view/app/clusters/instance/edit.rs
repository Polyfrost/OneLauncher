use std::path::PathBuf;

use freya::prelude::*;
use oneclient_common::domain::GameLoader;
use oneclient_core::clusters::ClusterKind;

use super::details::{DetailsState, details_body};
use super::rail::{Rail, facts_card, rail, version_art};
use super::shell::{Shell, shell};
use crate::hooks::{ClusterAction, use_cluster_mutation};

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

fn kind_label(kind: ClusterKind) -> &'static str {
    match kind {
        ClusterKind::OneClient => "OneClient",
        ClusterKind::Vanilla => "Vanilla",
        ClusterKind::Modded => "Modded",
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
        let ready = !typed.is_empty();

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

        let loader = if self.facts.mc_loader == GameLoader::Vanilla {
            "Vanilla".to_string()
        } else {
            self.facts.mc_loader.to_string()
        };

        let close_x = self.on_close.clone();
        let close_cancel = self.on_close.clone();
        let close_save = self.on_close.clone();

        shell(Shell {
            rail: rail(Rail {
                art,
                title,
                subtitle,
                card: facts_card(
                    "Instance".to_string(),
                    vec![
                        ("Type", kind_label(self.facts.kind).to_string()),
                        ("Version", self.facts.mc_version.clone()),
                        ("Loader", loader),
                    ],
                ),
                tags,
            }),
            eyebrow: "Instance details".to_string(),
            title: "Edit the instance".to_string(),
            subtitle: "Only how it is presented changes here. The version, loader and installed content stay as they are."
                .to_string(),
            body: details_body(state, self.facts.name.clone(), existing),
            scrolls_itself: false,
            note: "Changes apply straight away.".to_string(),
            secondary_label: "Cancel".to_string(),
            primary_label: "Save changes".to_string(),
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
