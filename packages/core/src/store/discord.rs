use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use discord_rich_presence::activity::{Activity, Assets, Button, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use tokio::sync::RwLock;

use crate::error::LauncherResult;

use super::Core;

pub struct DiscordRPC {
	started_at: i64,
	client: Arc<RwLock<DiscordIpcClient>>,
	connected: Arc<AtomicBool>,
}

impl DiscordRPC {
	pub fn initialize() -> LauncherResult<Self> {
		let client_id = Core::get()
			.discord_client_id
			.clone()
			.ok_or(DiscordError::MissingClientId)?;

		let mut client =
			DiscordIpcClient::new(client_id.as_str()).map_err(|_| DiscordError::ConnectError)?;

		let connected = client.connect().is_ok();

		if connected {
			tracing::info!("Discord RPC connected successfully");
		} else {
			tracing::warn!("Discord RPC failed to connect (Discord may not be running)");
		}

		Ok(Self {
			started_at: chrono::Utc::now().timestamp(),
			client: Arc::new(RwLock::new(client)),
			connected: Arc::new(AtomicBool::new(connected)),
		})
	}

	pub async fn set_message(&self, msg: &str, timestamp: Option<Timestamps>) -> bool {
		self.set_activity(
			Activity::new()
				.state(msg)
				.buttons(vec![Button::new("Website", "https://polyfrost.org/")])
				.timestamps(timestamp.unwrap_or_else(|| Timestamps::new().start(self.started_at)))
				.assets(
					Assets::new()
						.large_image("onelauncher_logo_512")
						.large_text("OneLauncher"),
				),
		)
		.await
	}

	pub async fn set_activity(&self, activity: Activity<'_>) -> bool {
		// If we know we're disconnected, try to reconnect first
		if !self.is_connected() && !self.reconnect().await {
			return false;
		}

		let mut client = self.client.write().await;

		if client.set_activity(activity).is_err() {
			// Connection dropped — mark as disconnected so next call will reconnect
			self.set_connected(false);
			tracing::warn!("Discord RPC set_activity failed; connection may have dropped");
			return false;
		}

		true
	}

	pub async fn clear_activity(&self) -> bool {
		if !self.is_connected() {
			return false;
		}

		let mut client = self.client.write().await;

		if client.clear_activity().is_err() {
			self.set_connected(false);
			return false;
		}

		true
	}

	pub async fn reconnect(&self) -> bool {
		tracing::info!("Attempting Discord RPC reconnect...");
		let mut client = self.client.write().await;

		let connected = client.reconnect().is_ok();

		self.set_connected(connected);

		if connected {
			tracing::info!("Discord RPC reconnected successfully");
		} else {
			tracing::warn!("Discord RPC reconnect failed");
		}

		connected
	}

	pub async fn close(&self) -> bool {
		let mut client = self.client.write().await;

		if client.close().is_err() {
			return false;
		}

		self.set_connected(false);
		true
	}

	#[must_use]
	pub fn is_connected(&self) -> bool {
		self.connected.load(std::sync::atomic::Ordering::Relaxed)
	}

	fn set_connected(&self, connected: bool) {
		self.connected
			.store(connected, std::sync::atomic::Ordering::Relaxed);
	}
}

#[onelauncher_macro::error]
#[derive(Debug, thiserror::Error)]
pub enum DiscordError {
	#[error("Discord client ID is missing")]
	MissingClientId,
	#[error("couldn't connect to Discord IPC")]
	ConnectError,
}
