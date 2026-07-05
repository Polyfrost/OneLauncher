use std::path::PathBuf;

use tokio::sync::oneshot;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum GroupedProgressEvent {
    Start {
        session_id: Uuid,
        title: String,
    },
    AddChild {
        session_id: Uuid,
        child_id: Uuid,
        label: String,
        total: u64,
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
