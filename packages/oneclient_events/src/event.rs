use uuid::Uuid;

use crate::progress::GroupedProgressEvent;
use crate::prompt::PromptRequest;

/// One channel carries all of it so ordering between a progress update and the
/// notification that follows it is preserved
/// Nothing here names a screen or a surface rendering is entirely the
/// front-end's call
#[derive(Debug)]
pub enum Event {
    Notification(Notification),
    Progress(ProgressEvent),
    Game(GameEvent),
    /// State changed elsewhere whoever caches it should refetch
    Signal(Signal),
}

#[derive(Debug)]
pub enum Notification {
    Message(Message),
    Prompt(PromptRequest),
}

impl From<Notification> for Event {
    fn from(value: Notification) -> Self {
        Self::Notification(value)
    }
}

impl From<Message> for Event {
    fn from(value: Message) -> Self {
        Self::Notification(Notification::Message(value))
    }
}

impl From<PromptRequest> for Event {
    fn from(value: PromptRequest) -> Self {
        Self::Notification(Notification::Prompt(value))
    }
}

impl From<ProgressEvent> for Event {
    fn from(value: ProgressEvent) -> Self {
        Self::Progress(value)
    }
}

impl From<GameEvent> for Event {
    fn from(value: GameEvent) -> Self {
        Self::Game(value)
    }
}

impl From<Signal> for Event {
    fn from(value: Signal) -> Self {
        Self::Signal(value)
    }
}

impl From<GroupedProgressEvent> for Event {
    fn from(value: GroupedProgressEvent) -> Self {
        Self::Progress(ProgressEvent::Grouped(value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub title: String,
    pub body: String,
    pub level: Level,
    /// Offered as an "open link" action on the toast and inbox entry
    pub link: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Level {
    #[default]
    Info,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressEvent {
    /// Keyed by `id` so repeated updates replace one another rather than stack
    Update {
        id: Uuid,
        label: String,
        current: u64,
        total: u64,
    },

    /// Turns the in-flight [`ProgressEvent::Update`] with this `id` into a
    /// finished message in place rather than emitting a second card
    Complete {
        id: Uuid,
        title: String,
        body: String,
        level: Level,
    },

    Grouped(GroupedProgressEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameEvent {
    Stage { cluster_id: i64, stage: LaunchStage },
    Log { cluster_id: i64, line: String },
    Failed { cluster_id: i64, message: String },
    Crashed(Box<GameCrash>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashRemedy {
    VerifyFiles,
    RaiseMemory,
    OpenJavaSettings,
    OpenMods,
}

impl CrashRemedy {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::VerifyFiles => "Verify & repair",
            Self::RaiseMemory => "Memory settings",
            Self::OpenJavaSettings => "Java settings",
            Self::OpenMods => "Open mods",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashFix {
    pub text: String,
    pub kind: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VersionRange {
    pub min: Option<String>,
    pub min_inclusive: bool,
    pub max: Option<String>,
    pub max_inclusive: bool,
}

impl VersionRange {
    #[must_use]
    pub fn at_least(version: &str) -> Self {
        Self {
            min: Some(version.to_string()),
            min_inclusive: true,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn exactly(version: &str) -> Self {
        Self {
            min: Some(version.to_string()),
            min_inclusive: true,
            max: Some(version.to_string()),
            max_inclusive: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrashModAction {
    Enable {
        hash: String,
    },
    Install {
        mod_id: String,
        range: Option<VersionRange>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashMod {
    pub name: String,
    pub id: Option<String>,
    pub version: Option<String>,
    pub note: String,
    pub hash: Option<String>,
    pub bundled_in: Option<String>,
    pub action: Option<CrashModAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashModIssueKind {
    Missing,
    WrongVersion,
    Incompatible,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashModIssue {
    pub kind: CrashModIssueKind,
    pub target: CrashMod,
    pub sources: Vec<CrashMod>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameCrash {
    pub cluster_id: i64,
    pub cluster_name: String,
    pub title: String,
    pub exit: String,
    pub played_secs: u64,
    pub cause: Option<String>,
    pub suspects: Vec<String>,
    pub remedy: Option<CrashRemedy>,
    pub fixes: Vec<CrashFix>,
    pub excerpt: Vec<String>,
    pub game_dir: Option<String>,
    pub mod_issues: Vec<CrashModIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchStage {
    Checking,
    /// Queued behind another cluster's launch
    Waiting,
    Downloading,
    Launching,
    Running,
    Exited,
}

impl LaunchStage {
    #[must_use]
    pub fn is_busy(self) -> bool {
        matches!(
            self,
            Self::Checking | Self::Waiting | Self::Downloading | Self::Launching
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    ClustersChanged,
    JavaChanged,
    /// Initial background sync finished
    SyncComplete,
}
