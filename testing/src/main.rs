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

	println!("clearing clusters");
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

	let cluster = create::create_cluster(
		name.clone(),
		game,
		loader,
		Some(loader_version),
		None,
		None,
		None,
		None,
		None,
	)
	.await?;

	State::sync().await?;

	println!("running minecraft");
	let c_lock = cluster::run(&cluster).await?;
	let uuid = c_lock.read().await.uuid;
	let pid = c_lock.read().await.current_child.read().await.id();

	println!("mc_uuid: {}", uuid);
	println!("mc_pid: {:?}", pid);

	println!("proc_uuid: {:?}", processor::get_running().await?);
	println!(
		"proc_path: {:?}",
		processor::get_running_cluster_paths().await?
	);

	let mut proc = c_lock.write().await;
	processor::wait_for(&mut proc).await?;

	Ok(())
}
