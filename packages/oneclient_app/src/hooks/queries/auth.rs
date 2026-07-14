
use std::sync::Mutex;

use freya::query::{
    Mutation, MutationCapability, MutationStateData, QueriesStorage, Query, QueryCapability,
    QueryStateData, UseMutation, UseQuery, use_mutation, use_query,
};
use oneclient_core::LauncherError;
use oneclient_core::auth::{self, AccountKind, MicrosoftLoginSession, MinecraftAccount};
use uuid::Uuid;

static HANDLED_LOGIN_CODE: Mutex<Option<String>> = Mutex::new(None);

pub fn login_code_already_handled(user_code: &str) -> bool {
    let mut handled = HANDLED_LOGIN_CODE.lock().unwrap();
    if handled.as_deref() == Some(user_code) {
        return true;
    }
    *handled = Some(user_code.to_string());
    false
}

pub fn reset_login_code_dedup() {
    *HANDLED_LOGIN_CODE.lock().unwrap() = None;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ListAccountsKeys;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefaultAccountKeys {
    pub fallback: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AccountKeys {
    pub id: Uuid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ListAccountsQuery;

impl QueryCapability for ListAccountsQuery {
    type Ok = Vec<MinecraftAccount>;
    type Err = LauncherError;
    type Keys = ListAccountsKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        auth::list_accounts().await
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefaultAccountQuery;

impl QueryCapability for DefaultAccountQuery {
    type Ok = Option<MinecraftAccount>;
    type Err = LauncherError;
    type Keys = DefaultAccountKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let account = auth::get_default_account().await?;
        if account.is_some() || !keys.fallback {
            return Ok(account);
        }

        let accounts = auth::list_accounts().await?;
        Ok(accounts.into_iter().next())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AccountQuery;

impl QueryCapability for AccountQuery {
    type Ok = Option<MinecraftAccount>;
    type Err = LauncherError;
    type Keys = AccountKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        auth::get_account(keys.id).await
    }
}

pub fn use_accounts() -> UseQuery<ListAccountsQuery> {
    use_query(Query::new(ListAccountsKeys, ListAccountsQuery))
}

pub fn use_default_account(fallback: bool) -> UseQuery<DefaultAccountQuery> {
    use_query(Query::new(
        DefaultAccountKeys { fallback },
        DefaultAccountQuery,
    ))
}

pub fn use_current_account() -> UseQuery<DefaultAccountQuery> {
    use_default_account(true)
}

pub fn use_account(id: Uuid) -> UseQuery<AccountQuery> {
    use_query(Query::new(AccountKeys { id }, AccountQuery))
}

pub fn try_accounts(query: &UseQuery<ListAccountsQuery>) -> Option<Vec<MinecraftAccount>> {
    settled_ok(query)
}

pub fn try_default_account(query: &UseQuery<DefaultAccountQuery>) -> Option<MinecraftAccount> {
    settled_ok(query).flatten()
}

pub fn try_account(query: &UseQuery<AccountQuery>) -> Option<MinecraftAccount> {
    settled_ok(query).flatten()
}

fn settled_ok<Q>(query: &UseQuery<Q>) -> Option<Q::Ok>
where
    Q: QueryCapability,
    Q::Ok: Clone,
{
    let reader = query.read();
    match &*reader.state() {
        QueryStateData::Settled { res: Ok(value), .. } => Some(value.clone()),
        QueryStateData::Loading {
            res: Some(Ok(value)),
        } => Some(value.clone()),
        _ => None,
    }
}

async fn invalidate_auth_queries(account_id: Option<Uuid>) {
    QueriesStorage::<ListAccountsQuery>::try_invalidate_matching(ListAccountsKeys).await;
    for fallback in [false, true] {
        QueriesStorage::<DefaultAccountQuery>::try_invalidate_matching(DefaultAccountKeys {
            fallback,
        })
        .await;
    }
    if let Some(id) = account_id {
        QueriesStorage::<AccountQuery>::try_invalidate_matching(AccountKeys { id }).await;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BeginMicrosoftLoginMutation;

impl MutationCapability for BeginMicrosoftLoginMutation {
    type Ok = MicrosoftLoginSession;
    type Err = LauncherError;
    type Keys = ();

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        auth::begin_microsoft_login().await
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FinishMicrosoftLoginMutation;

impl MutationCapability for FinishMicrosoftLoginMutation {
    type Ok = MinecraftAccount;
    type Err = LauncherError;
    type Keys = MicrosoftLoginSession;

    async fn run(&self, flow: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        auth::finish_microsoft_login(flow.clone()).await
    }

    async fn on_settled(&self, _keys: &Self::Keys, result: &Result<Self::Ok, Self::Err>) {
        if let Ok(account) = result {
            invalidate_auth_queries(Some(account.id)).await;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CancelMicrosoftLoginMutation;

/// Drops a pending browser login server-side when the user cancels before the
/// flow reaches the point of no return. Keyed by the session's CSRF state token.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CancelMicrosoftLoginKeys {
    pub state_token: String,
}

impl MutationCapability for CancelMicrosoftLoginMutation {
    type Ok = ();
    type Err = LauncherError;
    type Keys = CancelMicrosoftLoginKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        auth::cancel_microsoft_login(&keys.state_token).await;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AddMicrosoftAccountMutation;

impl MutationCapability for AddMicrosoftAccountMutation {
    type Ok = MinecraftAccount;
    type Err = LauncherError;
    type Keys = ();

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let flow = auth::begin_microsoft_login().await?;
        auth::finish_microsoft_login(flow).await
    }

    async fn on_settled(&self, _keys: &Self::Keys, result: &Result<Self::Ok, Self::Err>) {
        if let Ok(account) = result {
            invalidate_auth_queries(Some(account.id)).await;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AddOfflineAccountMutation;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AddOfflineAccountKeys {
    pub username: String,
}

impl MutationCapability for AddOfflineAccountMutation {
    type Ok = MinecraftAccount;
    type Err = LauncherError;
    type Keys = AddOfflineAccountKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        auth::add_offline_account(keys.username.clone()).await
    }

    async fn on_settled(&self, _keys: &Self::Keys, result: &Result<Self::Ok, Self::Err>) {
        if let Ok(account) = result {
            invalidate_auth_queries(Some(account.id)).await;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RemoveAccountMutation;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RemoveAccountKeys {
    pub id: Uuid,
}

impl MutationCapability for RemoveAccountMutation {
    type Ok = ();
    type Err = LauncherError;
    type Keys = RemoveAccountKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        auth::remove_account(keys.id).await
    }

    async fn on_settled(&self, keys: &Self::Keys, result: &Result<Self::Ok, Self::Err>) {
        if result.is_ok() {
            invalidate_auth_queries(Some(keys.id)).await;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SetDefaultAccountMutation;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SetDefaultAccountKeys {
    pub id: Option<Uuid>,
}

impl MutationCapability for SetDefaultAccountMutation {
    type Ok = ();
    type Err = LauncherError;
    type Keys = SetDefaultAccountKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        auth::set_default_account(keys.id).await
    }

    async fn on_settled(&self, keys: &Self::Keys, result: &Result<Self::Ok, Self::Err>) {
        if result.is_ok() {
            invalidate_auth_queries(keys.id).await;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RefreshAccountMutation;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RefreshAccountKeys {
    pub id: Uuid,
}

impl MutationCapability for RefreshAccountMutation {
    type Ok = MinecraftAccount;
    type Err = LauncherError;
    type Keys = RefreshAccountKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        auth::refresh_account(keys.id).await
    }

    async fn on_settled(&self, keys: &Self::Keys, result: &Result<Self::Ok, Self::Err>) {
        if result.is_ok() {
            invalidate_auth_queries(Some(keys.id)).await;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RefreshAllAccountsMutation;

impl MutationCapability for RefreshAllAccountsMutation {
    type Ok = Vec<MinecraftAccount>;
    type Err = LauncherError;
    type Keys = ();

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        auth::refresh_all_accounts().await
    }

    async fn on_settled(&self, _keys: &Self::Keys, result: &Result<Self::Ok, Self::Err>) {
        if result.is_ok() {
            invalidate_auth_queries(None).await;
        }
    }
}

pub type UseSetDefaultAccount = UseMutation<SetDefaultAccountMutation>;
pub type UseRemoveAccount = UseMutation<RemoveAccountMutation>;
pub type UseRefreshAccount = UseMutation<RefreshAccountMutation>;

pub fn use_begin_microsoft_login() -> UseMutation<BeginMicrosoftLoginMutation> {
    use_mutation(Mutation::new(BeginMicrosoftLoginMutation))
}

pub fn use_finish_microsoft_login() -> UseMutation<FinishMicrosoftLoginMutation> {
    use_mutation(Mutation::new(FinishMicrosoftLoginMutation))
}

pub fn use_cancel_microsoft_login() -> UseMutation<CancelMicrosoftLoginMutation> {
    use_mutation(Mutation::new(CancelMicrosoftLoginMutation))
}

pub fn use_add_microsoft_account() -> UseMutation<AddMicrosoftAccountMutation> {
    use_mutation(Mutation::new(AddMicrosoftAccountMutation))
}

pub fn use_add_offline_account() -> UseMutation<AddOfflineAccountMutation> {
    use_mutation(Mutation::new(AddOfflineAccountMutation))
}

pub fn use_remove_account() -> UseMutation<RemoveAccountMutation> {
    use_mutation(Mutation::new(RemoveAccountMutation))
}

pub fn use_set_default_account() -> UseMutation<SetDefaultAccountMutation> {
    use_mutation(Mutation::new(SetDefaultAccountMutation))
}

pub fn use_refresh_account() -> UseMutation<RefreshAccountMutation> {
    use_mutation(Mutation::new(RefreshAccountMutation))
}

pub fn use_refresh_all_accounts() -> UseMutation<RefreshAllAccountsMutation> {
    use_mutation(Mutation::new(RefreshAllAccountsMutation))
}

pub fn mutation_is_pending<M: MutationCapability>(mutation: &UseMutation<M>) -> bool {
    matches!(
        &*mutation.read().state(),
        MutationStateData::Pending | MutationStateData::Loading { .. }
    )
}

pub fn mutation_error<M>(mutation: &UseMutation<M>) -> Option<M::Err>
where
    M: MutationCapability,
    M::Err: Clone,
{
    let reader = mutation.read();
    match &*reader.state() {
        MutationStateData::Settled { res: Err(err), .. } => Some(err.clone()),
        MutationStateData::Loading {
            res: Some(Err(err)),
        } => Some(err.clone()),
        _ => None,
    }
}

pub fn accounts_have_microsoft(accounts: &[MinecraftAccount]) -> bool {
    accounts.iter().any(|a| a.kind == AccountKind::Microsoft)
}
