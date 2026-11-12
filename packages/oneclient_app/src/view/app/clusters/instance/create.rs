use std::collections::{HashMap, HashSet};

use freya::prelude::*;
use oneclient_common::GameLoader;
use freya::router::RouterContext;
use oneclient_core::GameVersionKind;
use oneclient_core::clusters::{ClusterKind, ModpackSource, PackChoices};

use super::copy::{footer_note, heading, is_ready, step_value};
use super::data::{Picks, import_jobs, pack_choices, resolve};
use super::details::DetailsState;
use super::model::*;
use super::rail::{Rail, RowState, rail, steps_card, version_art};
use super::shell::{Shell, shell};
use super::steps;
use crate::Route;
use crate::hooks::{
    ClusterAction, ExternalImportJob, settled_or_loading, use_active_cluster_id, use_cluster_mutation, use_clusters,
    use_dispatch,
};
use crate::i18n::{t, tf};
use crate::utils::default_cluster;

const MODPACK_BROWSE_TYPE: &str = "modpack";
const MODPACK_EXTENSIONS: [&str; 2] = ["mrpack", "zip"];
const IMPORT_EXTENSIONS: [&str; 2] = [oneclient_content::modpacks::SHARE_EXTENSION, "zip"];

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
        require_bundles: false,
        dedicated: picks.kind == ClusterKind::OneClient && picks.folder == FolderChoice::Dedicated,
        shared_group: match &picks.folder {
            FolderChoice::Custom(group) if picks.kind == ClusterKind::OneClient => {
                Some(group.clone())
            }
            _ => None,
        },
    })
}

fn wizard_rail(wizard: Wizard, picks: &Picks) -> Element {
    let description = wizard.details.description.read().trim().to_string();
    let tags = wizard.details.tags.read().clone();

    // The import creates one instance per pick, named after each source, so
    // the suggested name of a version nobody chose means nothing here
    let title = if picks.choice == TypeChoice::Import {
        t("oneclient.clusters.instance.create.title.import").to_string()
    } else if picks.name.trim().is_empty() {
        t("oneclient.clusters.instance.create.title.new").to_string()
    } else {
        picks.name.clone()
    };

    let subtitle = if description.is_empty() {
        match picks.choice {
            TypeChoice::OneClient => match &picks.versions.chosen {
                Some(version) => format!("OneClient · {version}"),
                None => "OneClient".to_string(),
            },
            TypeChoice::Scratch => match &picks.versions.chosen {
                Some(version) => format!("{version} · {}", picks.loader_label()),
                None => picks.loader_label(),
            },
            TypeChoice::Modpack => {
                t("oneclient.clusters.instance.create.subtitle.modpack").to_string()
            }
            TypeChoice::Import => match picks.import_source {
                ImportSource::File => {
                    t("oneclient.clusters.instance.create.subtitle.file").to_string()
                }
                ImportSource::Code => {
                    t("oneclient.clusters.instance.create.subtitle.code").to_string()
                }
                ImportSource::Launcher => {
                    t("oneclient.clusters.instance.create.subtitle.launcher").to_string()
                }
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

    let art = if matches!(picks.choice, TypeChoice::Modpack | TypeChoice::Import) {
        version_art(None, None)
    } else {
        version_art(picks.versions.chosen.as_deref(), picks.loader.chosen)
    }
    .picked_cover(wizard.details.cover.read().clone());

    rail(Rail {
        art,
        title,
        subtitle,
        card: steps_card(rows),
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
    prefill: Option<(String, GameLoader)>,
    invite: Option<i32>,
}

impl CreateInstanceModal {
    pub fn new(on_close: impl Into<EventHandler<()>>) -> Self {
        Self {
            on_close: on_close.into(),
            prefill: None,
            invite: None,
        }
    }

    pub fn prefill(mut self, version: String, loader: GameLoader) -> Self {
        self.prefill = Some((version, loader));
        self
    }

    pub fn join_invite(mut self, invite_id: i32) -> Self {
        self.invite = Some(invite_id);
        self
    }
}

impl Component for CreateInstanceModal {
    fn render(&self) -> impl IntoElement {
        let mutation = use_cluster_mutation();
        let prefilled = self.prefill.is_some();
        let prefill_version = self.prefill.as_ref().map(|(version, _)| version.clone());
        let prefill_loader = self
            .prefill
            .as_ref()
            .map_or(LoaderChoice::Fabric, |(_, loader)| LoaderChoice::of(*loader));
        let wizard = Wizard {
            exact_loader: self.prefill.as_ref().map(|(_, loader)| *loader),
            step: use_state(|| 0usize),
            choice: use_state(|| {
                if prefilled {
                    TypeChoice::Scratch
                } else {
                    TypeChoice::OneClient
                }
            }),
            version: use_state(|| prefill_version),
            filter: use_state(|| {
                if prefilled {
                    u8::MAX
                } else {
                    kind_bit(GameVersionKind::Release)
                }
            }),
            query: use_state(String::new),
            loader: use_state(|| prefill_loader),
            loader_version: use_state(|| None::<String>),
            declined: use_state(|| None::<HashSet<String>>),
            modpack_origin: use_state(|| ModpackOrigin::Browse),
            import_source: use_state(|| ImportSource::File),
            share_code: use_state(String::new),
            import_chosen: use_state(Vec::new),
            import_extra: use_state(Vec::new),
            import_mode: use_state(|| ImportMode::Improve),
            bundle_page: use_state(|| 0usize),
            import_declined: use_state(HashMap::new),
            import_bundles: use_state(HashMap::new),
            finishing: use_state(|| false),
            modpack_file: use_state(|| None),
            folder: use_state(|| FolderChoice::Shared),
            details: DetailsState::blank(),
        };
        let dispatch = use_dispatch();
        let browse_cluster = browse_cluster();

        let picks = resolve(wizard);
        let action = create_action(wizard, &picks);
        let (title, subtitle) = heading(&picks);

        let first = picks.index == 0;
        let last = picks.step == Step::Customize;
        let modpack_step = picks.step == Step::Modpack;
        let import_step = picks.step == Step::Import;
        let improving = picks.improving;
        let modpack_flow = picks.choice == TypeChoice::Modpack;
        let pick_pack = modpack_flow && picks.step == Step::ImportMode;
        let bundle_step = improving && picks.step == Step::Bundles;
        let no_targets = picks.improve_targets.as_ref().is_some_and(Vec::is_empty);
        let finishes = (bundle_step && picks.last_bundle_page())
            || (import_step && improving && no_targets);
        let checked = picks.import_screening.is_some();
        let check_failed = picks.import_screening_error.is_some();
        let finishing = picks.finishing;
        let current_bundles = picks
            .bundle_target()
            .map(|target| (target.key.clone(), picks.bundles.taken()));
        let finish = (finishes || finishing).then(|| {
            let mut kept = wizard.import_bundles.read().clone();
            kept.extend(current_bundles.clone());
            if modpack_flow {
                let path = picks.modpack_file.clone()?;
                let choices = pack_choices(
                    picks.import_screening.as_deref().unwrap_or_default(),
                    kept.get(&path).cloned(),
                );
                Some(Finish::Pack(path, choices))
            } else {
                Some(Finish::Import(import_jobs(
                    picks.import_screening.as_deref().unwrap_or_default(),
                    &wizard.import_chosen.read(),
                    &kept,
                )))
            }
        });
        let finish = finish.flatten();
        let mut finishing_state = wizard.finishing;
        if finishing && checked {
            finishing_state.set(false);
            if let Some(finish) = finish.clone() {
                let dispatch = dispatch.clone();
                let on_close = self.on_close.clone();
                spawn(async move { finish.run(&dispatch, &on_close) });
            }
        }
        let import_count = picks.import_count;
        let import_as_is = if import_step && !improving {
            import_jobs(&[], &wizard.import_chosen.read(), &HashMap::new())
        } else {
            Vec::new()
        };
        let friend = picks.step == Step::ImportSource && picks.import_source == ImportSource::File;
        let by_code = picks.step == Step::ImportSource && picks.import_source == ImportSource::Code;
        let share_code = picks.share_code.clone();
        let origin = picks.modpack_origin;
        let index = picks.index;
        let page = picks.bundle_page;
        let mut step = wizard.step;
        let mut query = wizard.query;
        let mut bundle_page = wizard.bundle_page;
        let mut import_bundles = wizard.import_bundles;

        let invite = crate::invite_join::wizard_invite(
            self.invite,
            self.prefill.as_ref(),
            picks.versions.chosen.as_deref(),
            picks.loader.chosen,
        );
        let off_invite = self.invite.is_some()
            && invite.is_none()
            && picks.versions.chosen.is_some()
            && picks.loader.chosen.is_some();
        let close_x = self.on_close.clone();
        let close_cancel = self.on_close.clone();
        let close_created = self.on_close.clone();
        let close_modpack = self.on_close.clone();
        let close_import = self.on_close.clone();
        let close_code = self.on_close.clone();

        shell(Shell {
            rail: wizard_rail(wizard, &picks),
            eyebrow: None,
            title: title.to_string(),
            subtitle,
            body: steps::body(wizard, &picks),
            scrolls_itself: matches!(
                picks.step,
                Step::Folder | Step::Version | Step::Import
            ),
            note: if off_invite {
                t("oneclient.clusters.instance.create.invite_mismatch").to_string()
            } else {
                footer_note(&picks)
            },
            secondary_label: if first {
                t("oneclient.common.cancel")
            } else {
                t("oneclient.common.back")
            }
            .to_string(),
            primary_label: match (modpack_step, origin) {
                _ if finishing && !check_failed => t("oneclient.clusters.instance.create.checking_mods").to_string(),
                _ if finishes && check_failed && modpack_flow => {
                    t("oneclient.clusters.instance.create.install_unchecked").to_string()
                }
                _ if finishes && modpack_flow => {
                    t("oneclient.clusters.instance.create.install_modpack").to_string()
                }
                _ if pick_pack => t("oneclient.clusters.instance.create.choose_file").to_string(),
                _ if finishes && check_failed => {
                    t("oneclient.clusters.instance.create.import_unchecked").to_string()
                }
                _ if (import_step && !improving) || finishes => match import_count {
                    0 => t("oneclient.clusters.instance.create.import.none").to_string(),
                    1 => t("oneclient.clusters.instance.create.import.one").to_string(),
                    n => tf("oneclient.clusters.instance.create.import.other", &[&n]),
                },
                (false, _) if friend => {
                    t("oneclient.clusters.instance.create.choose_file").to_string()
                }
                (false, _) if by_code => {
                    t("oneclient.clusters.instance.create.open_share").to_string()
                }
                (true, ModpackOrigin::Browse) => {
                    t("oneclient.clusters.instance.create.browse_modpacks").to_string()
                }
                (true, ModpackOrigin::File) => t("oneclient.common.next").to_string(),
                (false, _) if last => t("oneclient.clusters.instance.create.create").to_string(),
                (false, _) => t("oneclient.common.next").to_string(),
            },
            primary_enabled: is_ready(&picks) && !(last && off_invite),
            primary_busy: finishing && !check_failed,
            on_close: (move |()| close_x.call(())).into(),
            on_secondary: (move |()| {
                finishing_state.set(false);
                if first {
                    close_cancel.call(());
                } else if bundle_step && page > 0 {
                    bundle_page.set(page - 1);
                } else {
                    step.set(index.saturating_sub(1));
                }
            })
            .into(),
            on_primary: (move |()| {
                if friend {
                    pick_shared_file(dispatch.clone(), close_modpack.clone());
                } else if by_code {
                    dispatch.import_share(share_code.clone());
                    close_code.call(());
                } else if finishes {
                    if checked || check_failed {
                        if let Some(finish) = finish.clone() {
                            finish.run(&dispatch, &close_import);
                        }
                    } else {
                        finishing_state.set(true);
                    }
                } else if bundle_step {
                    if let Some((key, taken)) = current_bundles.clone() {
                        import_bundles.write().insert(key, taken);
                    }
                    bundle_page.set(page + 1);
                } else if import_step && !improving {
                    dispatch.import_external_instances(import_as_is.clone());
                    close_import.call(());
                } else if import_step {
                    bundle_page.set(0);
                    step.set(index + 1);
                } else if pick_pack && improving {
                    bundle_page.set(0);
                    pick_modpack_file(wizard.modpack_file, step, index + 1);
                } else if pick_pack || (modpack_step && origin == ModpackOrigin::Browse) {
                    start_modpack(
                        origin,
                        browse_cluster,
                        dispatch.clone(),
                        close_modpack.clone(),
                    );
                } else if last {
                    if let Some(action) = action.clone() {
                        if let Some(invite_id) = invite {
                            dispatch.create_cluster_and_join(mutation, action, invite_id);
                        } else {
                            mutation.mutate(action);
                        }
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

#[derive(Clone)]
enum Finish {
    Pack(std::path::PathBuf, PackChoices),
    Import(Vec<ExternalImportJob>),
}

impl Finish {
    fn run(self, dispatch: &crate::Actions, on_close: &EventHandler<()>) {
        match self {
            Self::Pack(path, choices) => {
                dispatch.install_modpack_with(ModpackSource::File(path), choices);
            }
            Self::Import(jobs) => dispatch.import_external_instances(jobs),
        }
        on_close.call(());
    }
}

fn pick_shared_file(dispatch: crate::Actions, on_close: EventHandler<()>) {
    spawn(async move {
        let Some(handle) = rfd::AsyncFileDialog::new()
            .set_title(t("oneclient.clusters.instance.create.dialog.import.title"))
            .add_filter(
                t("oneclient.clusters.instance.create.dialog.import.filter"),
                &IMPORT_EXTENSIONS,
            )
            .pick_file()
            .await
        else {
            return;
        };
        dispatch.import_file(handle.path().to_path_buf());
        on_close.call(());
    });
}

fn browse_cluster() -> i64 {
    let clusters = settled_or_loading(&use_clusters()).unwrap_or_default();
    let active = *use_active_cluster_id().read();

    default_cluster(clusters, active)
        .map(|cluster| cluster.id)
        .unwrap_or_default()
}

fn modpack_dialog() -> rfd::AsyncFileDialog {
    rfd::AsyncFileDialog::new()
        .set_title(t("oneclient.clusters.instance.create.dialog.modpack.title"))
        .add_filter(
            t("oneclient.clusters.instance.create.dialog.modpack.filter"),
            &MODPACK_EXTENSIONS,
        )
}

fn pick_modpack_file(
    mut modpack_file: State<Option<std::path::PathBuf>>,
    mut step: State<usize>,
    next: usize,
) {
    spawn(async move {
        let Some(handle) = modpack_dialog().pick_file().await else {
            return;
        };
        modpack_file.set(Some(handle.path().to_path_buf()));
        step.set(next);
    });
}

fn start_modpack(
    origin: ModpackOrigin,
    cluster_id: i64,
    dispatch: crate::Actions,
    on_close: EventHandler<()>,
) {
    match origin {
        ModpackOrigin::Browse => {
            on_close.call(());
            let _ = RouterContext::get().push(Route::Browser {
                cluster_id,
                package_type: MODPACK_BROWSE_TYPE.to_string(),
                pick_cluster: true,
            });
        }
        ModpackOrigin::File => {
            spawn(async move {
                let Some(handle) = modpack_dialog().pick_file().await else {
                    return;
                };
                dispatch.install_modpack(ModpackSource::File(handle.path().to_path_buf()));
                on_close.call(());
            });
        }
    }
}
