
mod commands;
mod runtime;
mod snapshot;

pub use commands::BridgeCommand;
pub use snapshot::{
    AsyncStatus, BridgeSnapshot, ClustersSnapshot, GameSnapshot, JavaSnapshot, LauncherInit,
    ProfilesSnapshot, SettingsSnapshot,
};

use runtime::CoreBridgeRuntime;
use tokio::sync::{mpsc, watch};

#[derive(Clone)]
pub struct OneClientBridge {
    pub snapshots: watch::Receiver<BridgeSnapshot>,
    commands: mpsc::UnboundedSender<BridgeCommand>,
}

impl OneClientBridge {
    pub fn new() -> (Self, CoreBridgeHandle) {
        let (snapshots_tx, snapshots_rx) = watch::channel(BridgeSnapshot::default());
        let (commands_tx, commands_rx) = mpsc::unbounded_channel();

        (
            Self {
                snapshots: snapshots_rx,
                commands: commands_tx.clone(),
            },
            CoreBridgeHandle {
                snapshots_tx,
                commands_rx,
                commands_tx: commands_tx.clone(),
            },
        )
    }

    pub fn send(&self, command: BridgeCommand) {
        let _ = self.commands.send(command);
    }
}

pub struct CoreBridgeHandle {
    snapshots_tx: watch::Sender<BridgeSnapshot>,
    commands_rx: mpsc::UnboundedReceiver<BridgeCommand>,
    commands_tx: mpsc::UnboundedSender<BridgeCommand>,
}

impl CoreBridgeHandle {
    pub fn spawn_runtime(self) {
        let runtime = CoreBridgeRuntime {
            snapshots_tx: self.snapshots_tx,
            commands_rx: self.commands_rx,
            commands_tx: self.commands_tx,
        };

        tokio::spawn(runtime.run());
    }
}

pub fn use_bridge_snapshot(bridge: &OneClientBridge) -> BridgeSnapshot {
    freya::sdk::use_track_watcher(&bridge.snapshots);
    bridge.snapshots.borrow().clone()
}
