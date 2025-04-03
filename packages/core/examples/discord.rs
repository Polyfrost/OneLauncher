use discord_rich_presence::activity::Timestamps;
use onelauncher_core::{api::proxy::proxy_empty::ProxyEmpty, error::LauncherResult, initialize_core, store::{CoreOptions, State}};

#[tokio::main]
async fn main() -> LauncherResult<()> {
	initialize_core(CoreOptions {
		discord_client_id: Some(String::from("1274084193193955408")),
		..Default::default()
	}, ProxyEmpty::new()).await?;

	let state = State::get().await?;
	let Some(rpc) = &state.rpc else {
		println!("RPC is not initialized");
		return Ok(());
	};

	rpc.set_message("Hello, Discord!", None).await;
	println!("Set Discord message");

	tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
	rpc.set_message("New timestamp.", Some(Timestamps::new())).await;
	println!("Set Discord message");

	tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
	rpc.set_message("Reverted back to the old timestamp!", None).await;
	println!("Set Discord message");

	tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
	rpc.clear_activity().await;
	println!("Cleared Discord message");

	Ok(())
}