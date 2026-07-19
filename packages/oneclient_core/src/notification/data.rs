use std::path::PathBuf;

use tokio::sync::oneshot;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum GroupedProgressEvent {
    Start {
        session_id: Uuid,
        title: String,
    },
    /// Pre-announce how many children (and total bytes) a category will have,
    /// so the aggregate total is known up-front instead of climbing as each
    /// child is added during a `buffer_unordered` fan-out.
    Expect {
        session_id: Uuid,
        category: TaskCategory,
        count: u64,
        total: u64,
    },
    AddChild {
        session_id: Uuid,
        child_id: Uuid,
        label: String,
        total: u64,
        category: TaskCategory,
    },
    UpdateChild {
        session_id: Uuid,
        child_id: Uuid,
        current: u64,
        total: u64,
    },
    SetChildPhase {
        session_id: Uuid,
        child_id: Uuid,
        phase: TaskPhase,
    },
    FinishChild {
        session_id: Uuid,
        child_id: Uuid,
    },
    End {
        session_id: Uuid,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TaskPhase {
    #[default]
    Downloading,
    Verifying,
    Extracting,
    Installing,
}

impl TaskPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Downloading => "Downloading",
            Self::Verifying => "Verifying",
            Self::Extracting => "Extracting",
            Self::Installing => "Installing",
        }
    }
}

/// Coarse grouping of a grouped-progress child. Drives the notification body text
/// ("Downloading Minecraft" vs "Downloading Packages") and the per-category task rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TaskCategory {
    Client,
    Metadata,
    Libraries,
    Natives,
    Assets,
    #[default]
    Packages,
}

impl TaskCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::Client => "Client",
            Self::Metadata => "Metadata",
            Self::Libraries => "Libraries",
            Self::Natives => "Natives",
            Self::Assets => "Assets",
            Self::Packages => "Packages",
        }
    }

    /// True for everything that is part of the Minecraft install (not user packages).
    pub fn is_minecraft(self) -> bool {
        !matches!(self, Self::Packages)
    }
}

#[derive(Debug)]
pub enum Notification {
	Message {
		title: String,
		body: String,
		level: NotificationLevel,
	},

	Progress {
		id: Uuid,
		label: String,
		current: u64,
		total: u64,
	},

	GroupedProgress(GroupedProgressEvent),

	GameStage {
		cluster_id: i64,
		stage: LaunchStage,
	},

	GameLog {
		cluster_id: i64,
		line: String,
	},

	GameFailed {
		cluster_id: i64,
		message: String,
	},

	Prompt {
		title: String,
		question: String,
		kind: PromptKind,
		reply_tx: oneshot::Sender<UserChoice>,
	},

	InvalidateClusters,

	InvalidateJava,

	SyncComplete,

	/// Live status of Microsoft Auth
	/// `None` clears the status
	MicrosoftLoginStatus(Option<MicrosoftLoginStatus>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MicrosoftLoginStatus {
	pub label: String,
	pub current: u64,
	pub total: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchStage {
	Checking,
	Downloading,
	Launching,
	Running,
	Exited,
}

impl LaunchStage {
	pub fn is_busy(self) -> bool {
		matches!(self, Self::Checking | Self::Downloading | Self::Launching)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
	JavaInstall { major: u32 },
	Update,
}

#[derive(Debug)]
pub enum UserChoice {
	Accept,
	Cancel,
	Folder(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NotificationLevel {
	Info,
	Error,
}
