use freya::query::{
    Mutation, MutationCapability, QueriesStorage, Query, QueryCapability, UseMutation, UseQuery,
    use_mutation, use_query,
};
use oneclient_core::{BisectAnswer, BisectChoice, BisectStatus, LauncherError};

use crate::i18n::t;
use crate::launcher::off_ui;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BisectKeys {
    pub cluster_id: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BisectQuery;

impl QueryCapability for BisectQuery {
    type Ok = Option<BisectStatus>;
    type Err = LauncherError;
    type Keys = BisectKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        let cluster_id = keys.cluster_id;
        off_ui(async move { oneclient_core::bisect_status(&state, cluster_id).await }).await
    }
}

pub fn use_bisect(cluster_id: i64) -> Option<BisectStatus> {
    let query: UseQuery<BisectQuery> =
        use_query(Query::new(BisectKeys { cluster_id }, BisectQuery));
    super::state::settled_or_loading(&query).flatten()
}

pub async fn bisect_report(cluster_id: i64) -> Result<String, String> {
    let state = crate::launcher::state().map_err(|e| e.to_string())?;
    off_ui(async move { oneclient_core::bisect_report(&state, cluster_id).await })
        .await
        .map_err(|e| e.to_string())
}

pub async fn invalidate_bisect_queries() {
    QueriesStorage::<BisectQuery>::invalidate_all().await;
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BisectAction {
    Start {
        cluster_id: i64,
    },
    Answer {
        cluster_id: i64,
        answer: BisectAnswer,
    },
    Skip {
        cluster_id: i64,
    },
    Undo {
        cluster_id: i64,
    },
    Continue {
        cluster_id: i64,
    },
    Reset {
        cluster_id: i64,
    },
    Finish {
        cluster_id: i64,
        disable: Vec<String>,
    },
    KeepOn {
        cluster_id: i64,
        hashes: Vec<String>,
    },
    AcceptSuggestion {
        cluster_id: i64,
    },
    DeclineSuggestion {
        cluster_id: i64,
    },
    Choose {
        cluster_id: i64,
        hashes: Vec<String>,
        choice: BisectChoice,
    },
    Settle {
        cluster_id: i64,
        hash: String,
        ignore: bool,
    },
    FindHidden {
        cluster_id: i64,
    },
    UseHidden {
        cluster_id: i64,
        accept: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BisectMutation;

impl MutationCapability for BisectMutation {
    type Ok = ();
    type Err = String;
    type Keys = BisectAction;

    async fn run(&self, keys: &BisectAction) -> Result<(), String> {
        let state = crate::launcher::state().map_err(|e| e.to_string())?;
        let action = keys.clone();
        off_ui(async move {
            match action {
                BisectAction::Start { cluster_id } => {
                    let events = &state.services.events;
                    let progress_id = uuid::Uuid::new_v4();
                    events.progress(progress_id, t("oneclient.bisect.starting"), 0, 0);
                    let started =
                        oneclient_core::start_bisect(&state, cluster_id, |label, done, total| {
                            events.progress(progress_id, label, done, total)
                        })
                        .await;
                    match &started {
                        Ok(_) => events.finish_progress(
                            progress_id,
                            t("oneclient.bisect.started.title"),
                            t("oneclient.bisect.started.body"),
                        ),
                        Err(err) => events.fail_progress(
                            progress_id,
                            t("oneclient.bisect.start_failed"),
                            err.to_string(),
                        ),
                    }
                    started.map(|_| ())
                }
                BisectAction::Answer { cluster_id, answer } => {
                    oneclient_core::answer_bisect(&state, cluster_id, answer)
                        .await
                        .map(|_| ())
                }
                BisectAction::Skip { cluster_id } => {
                    oneclient_core::skip_bisect_answer(&state, cluster_id).await
                }
                BisectAction::Undo { cluster_id } => {
                    oneclient_core::undo_bisect_answer(&state, cluster_id)
                        .await
                        .map(|_| ())
                }
                BisectAction::Continue { cluster_id } => {
                    oneclient_core::continue_bisect(&state, cluster_id)
                        .await
                        .map(|_| ())
                }
                BisectAction::Reset { cluster_id } => {
                    oneclient_core::reset_bisect(&state, cluster_id)
                        .await
                        .map(|_| ())
                }
                BisectAction::Finish {
                    cluster_id,
                    disable,
                } => oneclient_core::finish_bisect(&state, cluster_id, &disable).await,
                BisectAction::AcceptSuggestion { cluster_id } => {
                    oneclient_core::accept_bisect_suggestion(&state, cluster_id)
                        .await
                        .map(|_| ())
                }
                BisectAction::DeclineSuggestion { cluster_id } => {
                    oneclient_core::decline_bisect_suggestion(&state, cluster_id).await
                }
                BisectAction::KeepOn { cluster_id, hashes } => {
                    oneclient_core::keep_bisect_mods_on(&state, cluster_id, &hashes)
                        .await
                        .map(|_| ())
                }
                BisectAction::Choose {
                    cluster_id,
                    hashes,
                    choice,
                } => oneclient_core::set_bisect_mod_choice(&state, cluster_id, &hashes, choice)
                    .await
                    .map(|_| ()),
                BisectAction::Settle {
                    cluster_id,
                    hash,
                    ignore,
                } => oneclient_core::settle_unresolvable_mod(&state, cluster_id, &hash, ignore)
                    .await
                    .map(|_| ()),
                BisectAction::FindHidden { cluster_id } => {
                    let events = &state.services.events;
                    let progress_id = uuid::Uuid::new_v4();
                    events.progress(progress_id, t("oneclient.bisect.hidden.progress"), 0, 0);
                    let found = oneclient_core::find_hidden_dependencies(
                        &state,
                        cluster_id,
                        |label, done, total| events.progress(progress_id, label, done, total),
                    )
                    .await;
                    match &found {
                        Ok(_) => events.finish_progress(
                            progress_id,
                            t("oneclient.bisect.hidden.done"),
                            String::new(),
                        ),
                        Err(err) => events.fail_progress(
                            progress_id,
                            t("oneclient.bisect.hidden.failed"),
                            err.to_string(),
                        ),
                    }
                    found.map(|_| ())
                }
                BisectAction::UseHidden { cluster_id, accept } => {
                    oneclient_core::use_hidden_dependencies(&state, cluster_id, accept)
                        .await
                        .map(|_| ())
                }
            }
        })
        .await
        .map_err(|e| e.to_string())
    }

    async fn on_settled(&self, keys: &BisectAction, result: &Result<(), String>) {
        if let Err(err) = result
            && !matches!(
                keys,
                BisectAction::Start { .. } | BisectAction::FindHidden { .. }
            )
            && let Ok(state) = crate::launcher::state()
        {
            state
                .services
                .events
                .notify(t("oneclient.bisect.title"))
                .body(err)
                .error()
                .send();
        }
        super::mutations::invalidate_cluster_queries().await;
    }
}

pub fn use_bisect_mutation() -> UseMutation<BisectMutation> {
    use_mutation(Mutation::new(BisectMutation))
}
