use onelauncher::data::PackageType;
use onelauncher::package::content::Providers;
use onelauncher::{Result, State};

#[tokio::main]
async fn main() -> Result<()> {
	let _ = State::get().await?;

	search(Providers::Modrinth).await?;
	search(Providers::Curseforge).await?;

	Ok(())
}

async fn search(provider: Providers) -> Result<()> {
	println!("Searching for packages from {}...", provider.name());

	let results = provider
		.search(
			Some("Evergreen HUD".to_string()),
			Some(5),
			Some(0),
			None,
			None,
			None,
			Some(vec![PackageType::Mod]),
			None,
		)
		.await?;

	println!("{results:#?}");

	Ok(())
}
