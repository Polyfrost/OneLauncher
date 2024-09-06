use onelauncher::{data::PackageType, package::content, Result, State};

#[tokio::main]
async fn main() -> Result<()> {
	let _ = State::get().await?;

	let provider = content::Providers::Modrinth;
	let results = provider.search(
		Some("chatting".to_string()),
		Some(5),
		Some(0),
		None,
		None,
		None,
		Some(vec![PackageType::Mod]),
		None
	).await?;

	println!("{:#?}", results);

	Ok(())
}
