use freya::query::{
    Mutation, MutationCapability, QueriesStorage, Query, QueryCapability, UseMutation, UseQuery,
    use_mutation, use_query,
};
use oneclient_core::{BisectAnswer, BisectStatus, LauncherError};

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
        broken: bool,
    },
    Skip {
        cluster_id: i64,
    },
    Undo {
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
                    oneclient_core::start_bisect(&state, cluster_id)
                        .await
                        .map(|_| ())
                }
                BisectAction::Answer { cluster_id, broken } => {
                    let answer = if broken {
                        BisectAnswer::Broken
                    } else {
                        BisectAnswer::Works
                    };
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
                BisectAction::Finish {
                    cluster_id,
                    disable,
                } => oneclient_core::finish_bisect(&state, cluster_id, &disable).await,
                BisectAction::KeepOn { cluster_id, hashes } => {
                    oneclient_core::keep_bisect_mods_on(&state, cluster_id, &hashes)
                        .await
                        .map(|_| ())
                }
            }
        })
        .await
        .map_err(|e| e.to_string())
    }

    async fn on_settled(&self, _keys: &BisectAction, result: &Result<(), String>) {
        if let Err(err) = result
            && let Ok(state) = crate::launcher::state()
        {
            state
                .services
                .events
                .notify("Problem mod search")
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
