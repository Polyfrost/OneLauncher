use chrono::{TimeDelta, Utc};
use oneclient_auth::{
    AccountKind, CredentialsStore, offline_account, offline_username_error,
    offline_username_warning, offline_uuid,
};
use uuid::Uuid;

fn isolate_launcher_dir() {
    oneclient_common::paths::set_launcher_dir(
        std::env::temp_dir().join(format!("oneclient-auth-test-{}", Uuid::new_v4())),
    );
}

fn fake_microsoft_account(username: &str) -> oneclient_auth::MinecraftAccount {
    let mut account = offline_account(username.to_string());
    account.kind = AccountKind::Microsoft;
    account.access_token = "access".into();
    account.refresh_token = "refresh".into();
    account
}

#[tokio::test]
async fn offline_appearance_round_trips_through_auth_json() {
    isolate_launcher_dir();

    let mut store = CredentialsStore::default();
    let offline = store.add_offline_account("Steve".into()).unwrap();
    let id = offline.id;

    let appearance = oneclient_auth::AccountAppearance {
        skin: Some(oneclient_auth::CustomTexture {
            source: "https://example.com/steve.png".into(),
            model: Some(oneclient_auth::CustomTexture::SLIM.into()),
        }),
        cape: Some(oneclient_auth::CustomTexture {
            source: "https://example.com/cape.png".into(),
            model: None,
        }),
    };
    store.set_offline_appearance(id, appearance).await.unwrap();

    let reloaded = CredentialsStore::load().await.unwrap();
    let account = reloaded.get_account(id).expect("entry must survive");
    assert_eq!(
        account.custom_skin().map(|t| t.source.as_str()),
        Some("https://example.com/steve.png")
    );
    assert_eq!(
        account.custom_cape().map(|t| t.source.as_str()),
        Some("https://example.com/cape.png")
    );
}

#[tokio::test]
async fn appearance_is_rejected_for_microsoft_accounts() {
    isolate_launcher_dir();

    let mut store = CredentialsStore::default();
    let msa = fake_microsoft_account("MsaUser");
    let id = msa.id;
    store.users.insert(id, msa);

    let err = store
        .set_offline_appearance(id, Default::default())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("offline accounts"));
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
async fn offline_account_allowed_without_microsoft() {
    isolate_launcher_dir();

    let mut store = CredentialsStore::default();
    let offline = store.add_offline_account("Steve".into()).unwrap();

    assert_eq!(offline.kind, AccountKind::Offline);
    assert_eq!(offline.username, "Steve");
    assert_eq!(offline.custom_skin(), None);
}

#[tokio::test]
async fn offline_account_allowed_when_microsoft_exists() {
    isolate_launcher_dir();

    let mut store = CredentialsStore::default();
    let msa = fake_microsoft_account("MsaUser");
    store.users.insert(msa.id, msa);

    let offline = store.add_offline_account("Steve".into()).unwrap();
    assert_eq!(offline.kind, AccountKind::Offline);
    assert_eq!(offline.username, "Steve");
}

#[tokio::test]
async fn blank_offline_username_is_fatal() {
    isolate_launcher_dir();

    let mut store = CredentialsStore::default();
    let err = store.add_offline_account("   ".into()).unwrap_err();

    assert!(err.to_string().contains("invalid offline username"));
}

#[test]
fn unusual_offline_usernames_warn_instead_of_blocking() {
    assert_eq!(offline_username_warning("Steve"), None);
    assert_eq!(offline_username_error("Steve"), None);

    let warning =
        offline_username_warning("Steve!").expect("exclamation mark is not vanilla-legal");
    assert!(warning.contains("outside letters"));

    let warning = offline_username_warning("ab").expect("too short");
    assert!(warning.contains("shorter than 3"));

    let warning = offline_username_warning("aVeryLongUsernameOver16").expect("too long");
    assert!(warning.contains("longer than 16"));

    assert_eq!(
        offline_username_error("Steve!"),
        None,
        "only blank names are fatal"
    );
    assert!(offline_username_error("  ").is_some());
}

#[test]
fn token_expiring_imminently_counts_as_expired() {
    let mut account = fake_microsoft_account("MsaUser");

    account.expires = Utc::now() + TimeDelta::hours(1);
    assert!(!account.is_expired(), "a token with an hour left is usable");

    account.expires = Utc::now() + TimeDelta::seconds(5);
    assert!(
        account.is_expired(),
        "a token about to lapse must be renewed before launching"
    );

    account.expires = Utc::now() - TimeDelta::hours(1);
    assert!(account.is_expired());
}

#[tokio::test]
async fn default_account_returns_expired_token_without_refreshing() {
    isolate_launcher_dir();

    let mut store = CredentialsStore::default();
    let mut msa = fake_microsoft_account("MsaUser");
    msa.expires = Utc::now() - TimeDelta::hours(5);
    let msa_id = msa.id;
    store.users.insert(msa_id, msa);

    store.default_user = Some(msa_id);

    let account = store
        .default_account()
        .await
        .expect("an expired token must not fail the read path")
        .expect("the only account should be the default");

    assert_eq!(account.id, msa_id);
    assert!(account.is_expired());
}

#[tokio::test]
async fn signed_out_account_stays_listed_and_default() {
    isolate_launcher_dir();

    let mut store = CredentialsStore::default();
    let msa = fake_microsoft_account("MsaUser");
    let msa_id = msa.id;
    store.users.insert(msa_id, msa);
    store.default_user = Some(msa_id);

    store.mark_signed_out(msa_id).await.unwrap();

    let reloaded = CredentialsStore::load().await.unwrap();
    let account = reloaded.get_account(msa_id).expect("entry must survive");
    assert!(account.needs_sign_in());
    assert!(
        !account.refresh_token.is_empty(),
        "the refresh token is kept so the next renewal can retry it"
    );
    assert!(account.access_token.is_empty());
    assert_eq!(reloaded.default_user, Some(msa_id));
}

#[test]
fn accounts_saved_before_the_flag_existed_load_as_signed_in() {
    let account: oneclient_auth::MinecraftAccount = serde_json::from_str(&format!(
        r#"{{"id":"{}","username":"MsaUser","access_token":"a","refresh_token":"r","expires":"2026-01-01T00:00:00Z","kind":"microsoft"}}"#,
        Uuid::new_v4()
    ))
    .unwrap();

    assert!(!account.needs_sign_in());
}

#[test]
fn offline_accounts_never_need_sign_in() {
    let mut account = offline_account("Steve".into());
    account.signed_out = true;

    assert!(!account.needs_sign_in());
}
