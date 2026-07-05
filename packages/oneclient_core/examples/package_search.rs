
use std::env;

use oneclient_core::packages::domain::{ContentType, ProviderId};
use oneclient_core::packages::types::SearchFilters;
use oneclient_core::{LauncherResult, dev, logger};

#[tokio::main]
async fn main() -> LauncherResult<()> {
    logger::init_debug()?;

    let mut args = env::args().skip(1);
    let provider_name = args.next().unwrap_or_else(|| "modrinth".into());
    let query = args.next().unwrap_or_else(|| "sodium".into());

    let provider_id = parse_provider(&provider_name)?;
    let env = dev::ephemeral_services().await?;
    let provider = env.packages.get(provider_id)?;

    let page = provider
        .search(
            &SearchFilters {
                query: Some(query.clone()),
                limit: Some(8),
                content_type: Some(ContentType::Mod),
                ..Default::default()
            },
            &env,
        )
        .await?;

    println!(
        "Search {:?} for {query:?} - {} hit(s) (showing {}):\n",
        provider_id,
        page.total,
        page.items.len()
    );

    for hit in &page.items {
        println!(
            "  {} ({}) - {} downloads\n    {}",
            hit.name, hit.id, hit.downloads, hit.summary
        );
    }

    Ok(())
}

fn parse_provider(name: &str) -> LauncherResult<ProviderId> {
    match name.to_lowercase().as_str() {
        "modrinth" | "mr" => Ok(ProviderId::Modrinth),
        "curseforge" | "cf" => Ok(ProviderId::CurseForge),
        other => Err(oneclient_core::LauncherError::InvalidSettingsProfile {
            reason: format!("unknown provider {other:?}; use modrinth or curseforge"),
        }),
    }
}
