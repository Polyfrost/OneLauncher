use onelauncher_core::{api::proxy::proxy_empty::ProxyEmpty, error::LauncherResult, initialize_core, store::State};

#[tokio::main]
async fn main() -> LauncherResult<()> {
	initialize_core(ProxyEmpty::new()).await?;

	let state = State::get().await?;
	let Some(rpc) = &state.rpc else {
		println!("RPC is not initialized");
		return Ok(());
	};

	rpc.set_message("Hello, Discord!").await;
	println!("Set Discord message");

	tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
	rpc.set_message("This user is testing Discord RPC!").await;
	println!("Set Discord message");

	tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
	rpc.clear_activity().await;
	println!("Cleared Discord message");

	Ok(())
}