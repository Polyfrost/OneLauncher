use uuid::Uuid;

use crate::progress::GroupedProgressEvent;
use crate::prompt::PromptRequest;

/// Everything the core tells the UI about.
///
/// One channel carries all of it, so ordering between a progress update and the
/// notification that follows it is preserved.
///
/// The split between [`Event::Notification`] and [`Event::Progress`] is the
/// difference between an opinion and a fact. A notification is the core saying
/// "tell the user this", so it is user-facing by definition. Progress is the
/// core stating that some work is 40% done; whether that becomes a toast, a bar
/// in a modal, or nothing at all is the front-end's call.
///
/// That is also why nothing here names a screen or a surface. Progress that
/// should not become a toast simply never reaches the global bus: whoever
/// starts the work hands it a bus they own (see [`crate::EventBus::channel`]),
/// and the core never learns who is listening.
#[derive(Debug)]
pub enum Event {
	/// Something to put in front of the user: a message, or a question.
	Notification(Notification),
	/// Work happening, with a completion fraction. Carries no display opinion.
	Progress(ProgressEvent),
	/// A running Minecraft instance.
	Game(GameEvent),
	/// State changed elsewhere; whoever caches it should refetch.
	Signal(Signal),
}

/// Something the notification layer owns: a message to show, or a question that
/// blocks the emitting task until answered.
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Level {
	#[default]
	Info,
	Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressEvent {
	/// A single task's progress, keyed by `id` so repeated updates replace one
	/// another rather than stacking up.
	Update {
		id: Uuid,
		label: String,
		current: u64,
		total: u64,
	},

	/// Turn the in-flight [`ProgressEvent::Update`] with this `id` into a
	/// finished message in place, so a download and its completion notice are
	/// one card instead of two.
	Complete {
		id: Uuid,
		title: String,
		body: String,
	},

	/// A tree of related tasks. See [`crate::progress`].
	Grouped(GroupedProgressEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameEvent {
	Stage { cluster_id: i64, stage: LaunchStage },
	Log { cluster_id: i64, line: String },
	Failed { cluster_id: i64, message: String },
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
	#[must_use]
	pub fn is_busy(self) -> bool {
		matches!(self, Self::Checking | Self::Downloading | Self::Launching)
	}
}

/// Fire-and-forget notices that something changed. Carry no payload beyond
/// what the consumer needs to decide what to invalidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
	ClustersChanged,
	JavaChanged,
	/// Initial background sync finished.
	SyncComplete,
}
