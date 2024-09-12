//! `OneConfig` state using [`async_tungstenite`] to communicate to the `OneConfig` server

use super::ClusterPath;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Wrapper over an [`async_tungstenite::WebSocketStream`] to connect to the `OneConfig` server.
pub struct OneConfig {
	/// The public tokio socket connection.
	pub socket: async_tungstenite::WebSocketStream<async_tungstenite::tokio::ConnectStream>,
}

/// A WebSocket packet while updating a mod to display a GUI in game.
#[derive(Serialize, Deserialize, Debug)]
pub struct ModUpdatePacket {
	/// The ID of the packet.
	id: Uuid,
	/// The ID of the mod being updated.
	mod_id: Uuid,
	/// The [`ClusterPath`] associated with this event.
	cluster_path: ClusterPath,
}

impl OneConfig {
	/// Initializes a `OneConfig` socket connection on localhost port `4023`.
	pub async fn initialize(cluster: ClusterPath) -> crate::Result<Self> {
		let (socket, _) = async_tungstenite::tokio::connect_async(format!(
			"wss://localhost:4023/oneconfig/ws?cluster={cluster}"
		))
		.await?;
		Ok(Self { socket })
	}

	pub async fn close(&mut self) -> crate::Result<()> {
		self.socket.close(None).await?;

		Ok(())
	}
}
