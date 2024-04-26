#![cfg_attr(
	all(not(debug_assertions), target_os = "windows"),
	windows_subsystem = "windows"
)]

use onelauncher::prelude::*;

#[tokio::main]
async fn main() -> onelauncher::Result<()> {
	let _logger = onelauncher::start_logger();
	let state = State::get().await?;

	if minecraft::users().await?.is_empty() {
		println!("logging in");
		let login = minecraft::begin().await?;

		println!("{}", login.redirect_uri.as_str());
	
		let mut input = String::new();
		std::io::stdin()
			.read_line(&mut input)
			.expect("unable to read input");
		println!("{}", input.trim());
		let credentials = minecraft::finish(&input, login).await?;
		println!("{}", credentials.username);
	}

	state.settings.write().await.max_async_fetches = 100;
	state.settings.write().await.init_hooks.post =
		Some("echo this should run after minecraft as a global hook".to_string());
	// test changing fetch settings and resetting the semaphore
	state.reset_fetch_semaphore().await;

	{
		let c = cluster::list(None).await?;
		for (id, _) in c.into_iter() {
			cluster::remove(&id).await?;
		}
	}

	let name = "examplecluster".to_string();
	let game = "1.20.4".to_string();
	let loader = Loader::Vanilla;
	let loader_version = "stable".to_string();

	State::sync().await?;

	Ok(())
}
