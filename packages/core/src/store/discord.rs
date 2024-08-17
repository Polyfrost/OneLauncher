//! Handles Discord rich prescence integration
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use discord_rich_presence::activity::{Activity, Assets, Button};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use tokio::sync::RwLock;

use crate::constants::DISCORD_RPC_CLIENT_ID;
use crate::State;

/// A structure used to manage a Discord IPC client.
pub struct DiscordRPC {
	/// The managed [`DiscordIpcClient`] used for this IPC client.
	client: Arc<RwLock<DiscordIpcClient>>,
	/// Whether or not we are connected to the IPC server and internet.
	connected: Arc<AtomicBool>,
}

impl DiscordRPC {
	/// Initialize the Discord IPC client and attempt to connect to the server.
	/// If an error is returned, it will be ignored since Discord RPC isnt essential for the launcher to load.
	pub async fn initialize(is_offline: bool) -> crate::Result<Self> {
		let mut discord_ipc = DiscordIpcClient::new(DISCORD_RPC_CLIENT_ID)
			.map_err(|e| anyhow::anyhow!("failed to create discord client {}", e))?;

		let connected = if !is_offline {
			let result = discord_ipc.connect();
			if result.is_ok() {
				Arc::new(AtomicBool::new(true))
			} else {
				Arc::new(AtomicBool::new(false))
			}
		} else {
			Arc::new(AtomicBool::new(false))
		};

		let client = Arc::new(RwLock::new(discord_ipc));
		Ok(DiscordRPC { client, connected })
	}

	/// Set the Discord IPC activity with a message and try to reconnect if the initial connection fails.
	pub async fn set_activity(&self, msg: &str, reconnect_if_fail: bool) -> crate::Result<()> {
		if !self.retry_online().await {
			return Ok(());
		}
		let state = State::get().await?;
		let settings = state.settings.read().await;
		if settings.disable_discord {
			Ok(self.clear_activity(true).await?)
		} else {
			Ok(self.apply_activity(msg, reconnect_if_fail).await?)
		}
	}

	/// Do not use this function, use [`DiscordRPC#set_activity`]
	pub async fn apply_activity(&self, msg: &str, reconnect_if_fail: bool) -> crate::Result<()> {
		if !self.retry_connection().await {
			return Ok(());
		}

		let activity = Activity::new()
			.state(msg)
			.buttons(vec![
				Button::new("Download", "https://polyfrost.org/"),
			])
			.assets(
				Assets::new()
					.large_image("polyfrost_logo_512")
					.large_text("Polyfrost Logo"),
			);

		let mut client: tokio::sync::RwLockWriteGuard<'_, DiscordIpcClient> =
			self.client.write().await;
		let result = client.set_activity(activity.clone());
		let could_not_set = |e: Box<dyn serde::ser::StdError>| {
			anyhow::anyhow!("failed to apply discord activity {}", e)
		};

		if reconnect_if_fail {
			if let Err(_e) = result {
				client
					.reconnect()
					.map_err(|e| anyhow::anyhow!("failed to reconnect to Discord RPC {}", e))?;

				return Ok(client.set_activity(activity).map_err(could_not_set)?);
			}
		} else {
			result.map_err(could_not_set)?;
		}

		Ok(())
	}

	/// Attempt to check if we are still in an offline environment.
	pub async fn retry_online(&self) -> bool {
		let state = match State::get().await {
			Ok(s) => s,
			Err(_) => return false,
		};

		let offline = state.offline.read().await;
		if *offline {
			return false;
		}
		true
	}

	/// Retry the Discord IPC connection.
	pub async fn retry_connection(&self) -> bool {
		let mut client = self.client.write().await;
		if !self.connected.load(std::sync::atomic::Ordering::Relaxed) {
			if client.connect().is_ok() {
				self.connected
					.store(true, std::sync::atomic::Ordering::Relaxed);
				return true;
			}
			return false;
		}
		true
	}

	/// Clear the current Discord IPC activity, reconnecting if requested.
	pub async fn clear_activity(&self, reconnect_if_fail: bool) -> crate::Result<()> {
		if !self.retry_online().await || !self.retry_connection().await {
			return Ok(());
		}

		let mut client = self.client.write().await;
		let result = client.clear_activity();
		let could_not_clear = |e: Box<dyn serde::ser::StdError>| {
			anyhow::anyhow!("failed to clear discord activity {}", e)
		};

		if reconnect_if_fail {
			if result.is_err() {
				client
					.reconnect()
					.map_err(|e| anyhow::anyhow!("failed to reconnect to Discord RPC {}", e))?;

				return Ok(client.clear_activity().map_err(could_not_clear)?);
			}
		} else {
			result.map_err(could_not_clear)?;
		}

		Ok(())
	}

	/// Clear and disable the Discord IPC, reconnecting if requested.
	pub async fn clear(&self, reconnect_if_fail: bool) -> crate::Result<()> {
		let state: Arc<tokio::sync::RwLockReadGuard<'_, State>> = State::get().await?;

		{
			let settings = state.settings.read().await;
			if settings.disable_discord {
				println!("discord is disabled.. clearing");
				return self.clear_activity(true).await;
			}
		}

		self.set_activity("Idling...", reconnect_if_fail).await?;

		Ok(())
	}
}
