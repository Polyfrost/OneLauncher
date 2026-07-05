use oneclient_core::auth::{
    offline_account, offline_uuid, AccountKind, CredentialsStore,
};
use uuid::Uuid;

fn fake_microsoft_account(username: &str) -> oneclient_core::auth::MinecraftAccount {
    let mut account = offline_account(username.to_string());
    account.kind = AccountKind::Microsoft;
    account.access_token = "access".into();
    account.refresh_token = "refresh".into();
    account
}

#[test]
fn offline_uuid_matches_vanilla_algorithm() {
    let id = offline_uuid("Notch");
    
    assert_eq!(
        id,
        Uuid::parse_str("b50ad385-829d-3141-a216-7e7d7539ba7f").unwrap()
    );
    assert_ne!(id, offline_uuid("Steve"));
}

#[tokio::test]
async fn offline_account_blocked_without_microsoft() {
    let mut store = CredentialsStore::default();
    let err = store.add_offline_account("Steve".into()).unwrap_err();

    assert!(err.to_string().contains("Microsoft"));
}

#[tokio::test]
async fn offline_account_allowed_when_microsoft_exists() {
    let mut store = CredentialsStore::default();
    let msa = fake_microsoft_account("MsaUser");
    store.users.insert(msa.id, msa);

    let offline = store.add_offline_account("Steve".into()).unwrap();
    assert_eq!(offline.kind, AccountKind::Offline);
    assert_eq!(offline.username, "Steve");
}

#[tokio::test]
async fn default_offline_fails_when_last_microsoft_removed() {
    let mut store = CredentialsStore::default();
    let msa = fake_microsoft_account("MsaUser");
    let msa_id = msa.id;
    store.users.insert(msa_id, msa);

    let offline = store.add_offline_account("Steve".into()).unwrap();
    store.default_user = Some(offline.id);
    store.users.remove(&msa_id);

    let services = oneclient_core::dev::ephemeral_services().await.unwrap();
    let err = store.default_account(&services).await.unwrap_err();
    assert!(err.to_string().contains("Microsoft"));
}
