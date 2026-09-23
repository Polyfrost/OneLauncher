use std::collections::HashSet;

use freya::prelude::*;
use oneclient_core::clusters::ClusterKind;

use super::copy::{footer_note, heading, is_ready, step_value};
use super::data::{Picks, resolve};
use super::details::DetailsState;
use super::model::*;
use super::rail::{Rail, RowState, rail, steps_card, version_art};
use super::shell::{Shell, shell};
use super::steps;
use crate::hooks::{ClusterAction, use_cluster_mutation};

fn create_action(wizard: Wizard, picks: &Picks) -> Option<ClusterAction> {
    let mc_version = picks.versions.chosen.clone()?;
    let mc_loader = picks.loader.chosen?;

    Some(ClusterAction::CreateInstance {
        kind: picks.kind,
        name: picks.name.trim().to_string(),
        mc_version,
        mc_loader,
        mc_loader_version: picks.loader.version.clone(),
        description: wizard.details.description_value(),
        tags: wizard.details.tags.read().clone(),
        cover_source: wizard.details.cover.read().clone(),
        bundles: (picks.kind == ClusterKind::OneClient && picks.bundles.loaded)
            .then(|| picks.bundles.taken()),
    })
}

fn wizard_rail(wizard: Wizard, picks: &Picks) -> Element {
    let description = wizard.details.description.read().trim().to_string();
    let tags = wizard.details.tags.read().clone();

    let title = if picks.name.trim().is_empty() {
        "New instance".to_string()
    } else {
        picks.name.clone()
    };

    let subtitle = if description.is_empty() {
        match picks.choice {
            None => "Pick a type to get started".to_string(),
            Some(TypeChoice::OneClient) => match &picks.versions.chosen {
                Some(version) => format!("OneClient · {version}"),
                None => "OneClient".to_string(),
            },
            Some(TypeChoice::Scratch) => match &picks.versions.chosen {
                Some(version) => format!("{version} · {}", picks.loader_label()),
                None => picks.loader_label(),
            },
        }
    } else {
        description
    };

    let rows = picks
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            let state = if index < picks.index {
                RowState::Done
            } else if index == picks.index {
                RowState::Current
            } else {
                RowState::Pending
            };
            let value = if state == RowState::Pending {
                String::new()
            } else {
                step_value(wizard, picks, *step)
            };
            (step.label(), value, state)
        })
        .collect();

    let art = version_art(picks.versions.chosen.as_deref(), picks.loader.chosen)
        .picked_cover(wizard.details.cover.read().clone());

    rail(Rail {
        art,
        title,
        subtitle,
        card: steps_card(picks.progress(), rows),
        tags: if picks.step == Step::Customize {
            tags
        } else {
            Vec::new()
        },
    })
}

#[derive(PartialEq)]
pub struct CreateInstanceModal {
    on_close: EventHandler<()>,
}

impl CreateInstanceModal {
    pub fn new(on_close: impl Into<EventHandler<()>>) -> Self {
        Self {
            on_close: on_close.into(),
        }
    }
}

impl Component for CreateInstanceModal {
    fn render(&self) -> impl IntoElement {
        let mutation = use_cluster_mutation();
        let wizard = Wizard {
            step: use_state(|| 0usize),
            choice: use_state(|| Some(TypeChoice::OneClient)),
            version: use_state(|| None::<String>),
            filter: use_state(|| 0usize),
            query: use_state(String::new),
            loader: use_state(|| LoaderChoice::Fabric),
            loader_version: use_state(|| None::<String>),
            declined: use_state(|| None::<HashSet<String>>),
            details: DetailsState::blank(),
        };

        let picks = resolve(wizard);
        let action = create_action(wizard, &picks);
        let (title, subtitle) = heading(&picks);

        let first = picks.index == 0;
        let last = picks.step == Step::Customize;
        let index = picks.index;
        let mut step = wizard.step;
        let mut query = wizard.query;

        let close_x = self.on_close.clone();
        let close_cancel = self.on_close.clone();
        let close_created = self.on_close.clone();

        shell(Shell {
            rail: wizard_rail(wizard, &picks),
            eyebrow: picks.progress(),
            title: title.to_string(),
            subtitle,
            body: steps::body(wizard, &picks),
            scrolls_itself: picks.step == Step::Version,
            note: footer_note(&picks),
            secondary_label: if first { "Cancel" } else { "Back" }.to_string(),
            primary_label: if last { "Create instance" } else { "Next" }.to_string(),
            primary_enabled: is_ready(&picks),
            on_close: (move |()| close_x.call(())).into(),
            on_secondary: (move |()| {
                if first {
                    close_cancel.call(());
                } else {
                    step.set(index.saturating_sub(1));
                }
            })
            .into(),
            on_primary: (move |()| {
                if last {
                    if let Some(action) = action.clone() {
                        mutation.mutate(action);
                        close_created.call(());
                    }
                } else {
                    query.set(String::new());
                    step.set(index + 1);
                }
            })
            .into(),
        })
    }
}
