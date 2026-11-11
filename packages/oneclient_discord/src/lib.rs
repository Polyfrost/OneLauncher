use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use discord_rich_presence::activity::{Activity, Assets, Button, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

use oneclient_common::constants::DISCORD_CLIENT_ID;

const RECONNECT_BACKOFF: Duration = Duration::from_secs(60);

const IDLE_POLL: Duration = Duration::from_secs(3600);

/// Optional Rich Presence art asset. Without it Discord shows the application
/// icon uploaded in the Developer Portal, so no asset has to exist on day one.
const LARGE_IMAGE: Option<&str> = option_env!("REMIXCLIENT_DISCORD_LARGE_IMAGE");

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Presence {
    #[default]
    Idle,
    Playing {
        cluster: String,
        mc_version: String,
    },
}

impl Presence {
    fn details(&self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Playing { .. } => "Playing Minecraft",
        }
    }

    fn state(&self) -> String {
        match self {
            Self::Idle => "In the launcher".to_owned(),
            Self::Playing {
                cluster,
                mc_version,
            } => format!("{cluster} ({mc_version})"),
        }
    }
}

enum Command {
    SetPresence(Presence),
    SetEnabled(bool),
    Shutdown,
}

#[derive(Clone)]
pub struct DiscordRpc {
    tx: Sender<Command>,
}

impl DiscordRpc {
    pub fn spawn(enabled: bool) -> Self {
        let (tx, rx) = mpsc::channel();

        // A fork without its own Discord application cannot present upstream's
        // app identity, so Rich Presence stays off until a client id is supplied.
        let enabled = enabled && !DISCORD_CLIENT_ID.is_empty();

        std::thread::Builder::new()
            .name("discord-rpc".to_owned())
            .spawn(move || Worker::new(enabled).run(&rx))
            .expect("failed to spawn the Discord RPC thread");

        Self { tx }
    }

    pub fn set_presence(&self, presence: Presence) {
        self.send(Command::SetPresence(presence));
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.send(Command::SetEnabled(enabled));
    }

    pub fn shutdown(&self) {
        self.send(Command::Shutdown);
    }

    fn send(&self, command: Command) {
        if self.tx.send(command).is_err() {
            tracing::debug!("Discord RPC worker is gone; dropping command");
        }
    }
}

struct Worker {
    client: DiscordIpcClient,
    enabled: bool,
    connected: bool,
    ever_connected: bool,
    presence: Presence,
    presence_since: i64,
    next_attempt: Instant,
}

impl Worker {
    fn new(enabled: bool) -> Self {
        Self {
            client: DiscordIpcClient::new(DISCORD_CLIENT_ID),
            enabled,
            connected: false,
            ever_connected: false,
            presence: Presence::Idle,
            presence_since: chrono::Utc::now().timestamp(),
            next_attempt: Instant::now(),
        }
    }

    fn run(mut self, rx: &mpsc::Receiver<Command>) {
        self.publish();

        loop {
            match rx.recv_timeout(self.poll_interval()) {
                Ok(Command::SetPresence(presence)) => self.set_presence(presence),
                Ok(Command::SetEnabled(enabled)) => self.set_enabled(enabled),
                Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => self.publish(),
            }
        }

        self.disconnect();
    }

    fn poll_interval(&self) -> Duration {
        if !self.enabled || self.connected {
            return IDLE_POLL;
        }

        self.next_attempt
            .saturating_duration_since(Instant::now())
            .max(Duration::from_millis(100))
    }

    fn set_presence(&mut self, presence: Presence) {
        if self.presence == presence {
            return;
        }

        self.presence = presence;
        self.presence_since = chrono::Utc::now().timestamp();
        self.publish();
    }

    fn set_enabled(&mut self, enabled: bool) {
        let enabled = enabled && !DISCORD_CLIENT_ID.is_empty();
        if self.enabled == enabled {
            return;
        }

        self.enabled = enabled;

        if enabled {
            self.next_attempt = Instant::now();
            self.publish();
        } else {
            self.disconnect();
        }
    }

    fn publish(&mut self) {
        if !self.enabled || !self.ensure_connected() {
            return;
        }

        let state = self.presence.state();
        let mut assets = Assets::new();
        if let Some(key) = LARGE_IMAGE {
            assets = assets.large_image(key).large_text("RemixClient");
        }
        let activity = Activity::new()
            .details(self.presence.details())
            .state(&state)
            .timestamps(Timestamps::new().start(self.presence_since))
            .assets(assets)
            .buttons(vec![Button::new(
                "GitHub",
                "https://github.com/linkure8888-lang/OneLauncher",
            )]);

        if let Err(err) = self.client.set_activity(activity) {
            tracing::warn!("Discord RPC set_activity failed: {err}");
            self.drop_connection();
        }
    }

    fn ensure_connected(&mut self) -> bool {
        if self.connected {
            return true;
        }

        if Instant::now() < self.next_attempt {
            return false;
        }
        self.next_attempt = Instant::now() + RECONNECT_BACKOFF;

        let result = if self.ever_connected {
            self.client.reconnect()
        } else {
            self.client.connect()
        };

        match result {
            Ok(()) => {
                tracing::info!("Discord RPC connected");
                self.connected = true;
                self.ever_connected = true;
                true
            }
            Err(err) => {
                tracing::debug!("Discord RPC connect failed (is Discord running?): {err}");
                false
            }
        }
    }

    fn drop_connection(&mut self) {
        self.connected = false;
        self.next_attempt = Instant::now() + RECONNECT_BACKOFF;
    }

    fn disconnect(&mut self) {
        if !self.connected {
            return;
        }

        let _ = self.client.clear_activity();
        let _ = self.client.close();
        self.connected = false;
    }
}
