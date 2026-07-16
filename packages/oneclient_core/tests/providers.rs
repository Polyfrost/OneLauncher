
use oneclient_core::dev;
use oneclient_core::packages::domain::{ContentType, ProviderId};
use oneclient_core::packages::types::{PackageBody, SearchFilters};

#[test]
fn registry_get_unknown_provider_errors() {
    let registry = oneclient_core::packages::PackageProviderRegistry::new();

    let err = match registry.get(ProviderId::Local) {
        Err(e) => e,
        Ok(_) => panic!("expected ProviderId::Local to be unregistered"),
    };

    assert!(err.to_string().contains("not registered"));
}

#[tokio::test]
async fn registry_providers_match_ids() {
    let registry = oneclient_core::packages::PackageProviderRegistry::new();

    for id in registry.remote_ids() {
        assert_eq!(registry.get(id).unwrap().id(), id);
    }
}

#[tokio::test]
#[ignore = "requires network"]
async fn modrinth_search_returns_hits() {
    let env = dev::ephemeral_services().await.unwrap();
    let provider = env.packages.get(ProviderId::Modrinth).unwrap();

    let page = provider
        .search(
            &SearchFilters {
                query: Some("sodium".into()),
                limit: Some(5),
                content_type: Some(ContentType::Mod),
                ..Default::default()
            },
            &env,
        )
        .await
        .unwrap();

    assert!(!page.items.is_empty());
    assert!(
        page.items
            .iter()
            .any(|p| p.name.to_lowercase().contains("sodium"))
    );
}

#[tokio::test]
#[ignore = "requires network"]
async fn modrinth_get_project_and_versions() {
    let env = dev::ephemeral_services().await.unwrap();
    let provider = env.packages.get(ProviderId::Modrinth).unwrap();

    let project = provider.get_project("AANobbMI", &env).await.unwrap();
    assert_eq!(project.slug, "sodium");
    assert_eq!(project.provider, ProviderId::Modrinth);

    let versions = provider
        .list_versions(&project.id, Some("1.20.1"), None, 0, 5, &env)
        .await
        .unwrap();
    assert!(!versions.items.is_empty());
}

#[tokio::test]
#[ignore = "requires network"]
async fn curseforge_search_returns_hits() {
    let env = dev::ephemeral_services().await.unwrap();
    let provider = env.packages.get(ProviderId::CurseForge).unwrap();

    let page = provider
        .search(
            &SearchFilters {
                query: Some("sodium".into()),
                limit: Some(5),
                content_type: Some(ContentType::Mod),
                ..Default::default()
            },
            &env,
        )
        .await
        .unwrap();

    assert!(!page.items.is_empty());
}

#[tokio::test]
#[ignore = "requires network"]
async fn modrinth_lookup_version_by_sha1() {
    let env = dev::ephemeral_services().await.unwrap();
    let provider = env.packages.get(ProviderId::Modrinth).unwrap();

    let versions = provider
        .list_versions("AANobbMI", Some("1.20.1"), None, 0, 1, &env)
        .await
        .unwrap();
    let version = provider
        .get_version("AANobbMI", &versions.items[0].version_id, &env)
        .await
        .unwrap();
    let sha1 = version
        .primary_file()
        .expect("version has a primary file")
        .sha1
        .clone();

    let found = env.packages.lookup_version(&sha1, &env).await.unwrap();
    assert!(found.is_some());
    
    let (provider_id, _) = found.unwrap();
    assert_eq!(provider_id, ProviderId::Modrinth);
}

#[tokio::test]
#[ignore = "requires network"]
async fn curseforge_get_project_fetches_markdown_body() {
    let env = dev::ephemeral_services().await.unwrap();
    let provider = env.packages.get(ProviderId::CurseForge).unwrap();

    // 238222 = Just Enough Items (JEI)
    let project = provider
        .get_project_with_body("238222", &env)
        .await
        .unwrap();
    assert_eq!(project.provider, ProviderId::CurseForge);

    let body = match &project.body {
        PackageBody::Raw(body) => body,
        PackageBody::Url(url) => panic!("expected a fetched body, got a link to {url}"),
    };

    assert!(!body.is_empty(), "body should not be empty");
    assert!(
        !body.contains("<p>") && !body.contains("<br"),
        "body should be markdown, not html: {body:.200}"
    );
    assert!(
        !body.contains("](/"),
        "body should not contain site-relative links: {body:.200}"
    );
}

#[tokio::test]
#[ignore = "requires network"]
async fn modrinth_get_project_with_body_has_markdown() {
    let env = dev::ephemeral_services().await.unwrap();
    let provider = env.packages.get(ProviderId::Modrinth).unwrap();

    let project = provider
        .get_project_with_body("AANobbMI", &env)
        .await
        .unwrap();

    match &project.body {
        PackageBody::Raw(body) => assert!(!body.is_empty(), "body should not be empty"),
        PackageBody::Url(url) => panic!("expected a fetched body, got a link to {url}"),
    }
}
