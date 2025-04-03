use std::sync::{atomic::AtomicBool, Arc};

use discord_rich_presence::{activity::{Activity, Assets, Button, Timestamps}, DiscordIpc, DiscordIpcClient};
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
		let client_id = Core::get().discord_client_id.clone()
			.ok_or(DiscordError::MissingClientId)?;

		let mut client = DiscordIpcClient::new(client_id.as_str())
			.map_err(|_| DiscordError::ConnectError)?;

		let connected = client.connect().is_ok();

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
				.buttons(
					vec![Button::new("Website", "https://polyfrost.org/")]
				)
				.timestamps(
					timestamp.unwrap_or_else(|| Timestamps::new().start(self.started_at))
				)
				.assets(
					Assets::new()
						.large_image("onelauncher_logo_512")
						.large_text("OneLauncher"),
				)
		).await
	}

	pub async fn set_activity(&self, activity: Activity<'_>) -> bool {
		if !self.is_connected() {
			return false;
		}

		let mut client = self.client.write().await;

		if client.set_activity(activity).is_err() {
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
			return false;
		}

		true
	}

	pub async fn reconnect(&self) -> bool {
		let mut client = self.client.write().await;

		let connected = client.reconnect().is_ok();

		self.set_connected(connected);
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
		self.connected.store(connected, std::sync::atomic::Ordering::Relaxed);
	}
}

#[derive(Debug, thiserror::Error)]
pub enum DiscordError {
	#[error("Discord client ID is missing")]
	MissingClientId,
	#[error("couldn't connect to Discord IPC")]
	ConnectError,
}