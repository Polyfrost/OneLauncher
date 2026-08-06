use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::error::{EventError, EventResult};
use crate::event::{
	Event, GameEvent, LaunchStage, Level, Message, Notification, Persistence, ProgressEvent, Signal,
};
use crate::prompt::{Answer, Chosen, Prompt, PromptRequest};

/// The sending half of the event bus.
///
/// Cheap to clone, since `UnboundedSender` is refcounted internally and every
/// clone feeds the same channel. Hand clones to subsystems rather than wrapping
/// this in an `Arc`.
#[derive(Clone, Debug)]
pub struct EventBus {
	tx: mpsc::UnboundedSender<Event>,
}

pub type EventReceiver = mpsc::UnboundedReceiver<Event>;

impl EventBus {
	#[must_use]
	pub fn new(tx: mpsc::UnboundedSender<Event>) -> Self {
		Self { tx }
	}

	/// Creates a bus and its receiver. The owner of the receiver is the UI.
	#[must_use]
	pub fn channel() -> (Self, EventReceiver) {
		let (tx, rx) = mpsc::unbounded_channel();
		(Self::new(tx), rx)
	}

	/// The one primitive every other method goes through.
	///
	/// A closed bus is logged rather than returned: emitting is called from
	/// deep inside download loops where there is nothing useful to do about a
	/// UI that has already shut down. Use [`EventBus::ask`] when you need to
	/// know whether the other end is alive.
	pub fn emit(&self, event: impl Into<Event>) {
		if let Err(err) = self.tx.send(event.into()) {
			tracing::debug!("dropping event, bus is closed: {err}");
		}
	}

	/// Whether the receiving end is still up.
	#[must_use]
	pub fn is_open(&self) -> bool {
		!self.tx.is_closed()
	}

	// --- notifications -----------------------------------------------------

	/// Starts building a notification. Nothing is emitted until `.send()`.
	///
	/// ```ignore
	/// events.notify("Install failed").body(err.to_string()).error().send();
	/// ```
	pub fn notify(&self, title: impl Into<String>) -> NotificationBuilder<'_> {
		NotificationBuilder {
			bus: self,
			message: Message {
				title: title.into(),
				body: String::new(),
				level: Level::Info,
				persistence: Persistence::Transient,
			},
			persistence: None,
		}
	}

	// --- progress ----------------------------------------------------------

	/// Reports progress for a single task. Repeated calls with the same `id`
	/// replace one another.
	///
	/// Not a builder: this runs tens of thousands of times during a download,
	/// and the argument list is short enough that a builder would only add
	/// ceremony to a hot path.
	pub fn progress(&self, id: Uuid, label: impl Into<String>, current: u64, total: u64) {
		self.emit(ProgressEvent::Update {
			id,
			label: label.into(),
			current,
			total,
		});
	}

	/// Converts the in-flight progress entry `id` into a finished message.
	///
	/// `persistence` is spelled out rather than defaulted: a finished download
	/// is usually nothing, and occasionally the "restart to apply" the whole
	/// download was for, and only the caller knows which.
	pub fn finish_progress(
		&self,
		id: Uuid,
		title: impl Into<String>,
		body: impl Into<String>,
		persistence: Persistence,
	) {
		self.emit(ProgressEvent::Complete {
			id,
			title: title.into(),
			body: body.into(),
			persistence,
		});
	}

	// --- game --------------------------------------------------------------

	pub fn game_stage(&self, cluster_id: i64, stage: LaunchStage) {
		self.emit(GameEvent::Stage { cluster_id, stage });
	}

	pub fn game_log(&self, cluster_id: i64, line: impl Into<String>) {
		self.emit(GameEvent::Log {
			cluster_id,
			line: line.into(),
		});
	}

	pub fn game_failed(&self, cluster_id: i64, message: impl Into<String>) {
		self.emit(GameEvent::Failed {
			cluster_id,
			message: message.into(),
		});
	}

	// --- signals -----------------------------------------------------------

	pub fn signal(&self, signal: Signal) {
		self.emit(signal);
	}

	// --- prompts -----------------------------------------------------------

	/// Asks the user a question and waits for the answer.
	///
	/// Returns `Ok(None)` when the prompt was dismissed. The caller's own value
	/// for the chosen option comes back in [`Chosen::value`]; the untyped choice
	/// id never escapes this function.
	#[tracing::instrument(level = "debug", skip_all, fields(title = %prompt.title))]
	pub async fn ask<T>(&self, prompt: Prompt<T>) -> EventResult<Option<Chosen<T>>> {
		let Prompt {
			title,
			body,
			options,
			dismiss,
		} = prompt;

		let (choices, values): (Vec<_>, Vec<_>) = options.into_iter().unzip();
		let (reply, reply_rx) = oneshot::channel();

		self.tx
			.send(Event::Notification(Notification::Prompt(PromptRequest {
				title,
				body,
				choices: choices.clone(),
				dismiss,
				reply,
			})))
			.map_err(|_| EventError::BusClosed)?;

		// A dropped sender means the UI went away without answering, which is
		// indistinguishable from a dead bus as far as the caller is concerned.
		let Some(answer) = reply_rx.await.map_err(|_| EventError::BusClosed)? else {
			return Ok(None);
		};

		let Answer { id, input } = answer;
		let index = choices
			.iter()
			.position(|choice| choice.id == id)
			.ok_or(EventError::UnknownChoice(id))?;

		let value = values
			.into_iter()
			.nth(index)
			.ok_or(EventError::UnknownChoice(id))?;

		Ok(Some(Chosen { value, input }))
	}
}

#[must_use = "the notification is not emitted until `.send()` is called"]
pub struct NotificationBuilder<'a> {
	bus: &'a EventBus,
	message: Message,
	/// The caller's explicit choice, resolved against the level in `send`.
	/// Kept out of `message` so `.error().transient()` and
	/// `.transient().error()` mean the same thing; a builder where the call
	/// order silently changes where a notification lands is a trap.
	persistence: Option<Persistence>,
}

impl NotificationBuilder<'_> {
	pub fn body(mut self, body: impl Into<String>) -> Self {
		self.message.body = body.into();
		self
	}

	pub fn level(mut self, level: Level) -> Self {
		self.message.level = level;
		self
	}

	pub fn info(self) -> Self {
		self.level(Level::Info)
	}

	pub fn error(self) -> Self {
		self.level(Level::Error)
	}

	/// Files this notification in the notification center.
	pub fn persistent(mut self) -> Self {
		self.persistence = Some(Persistence::Persistent);
		self
	}

	/// Shows this notification and forgets it. Use on an error the user is
	/// already looking at and can simply retry.
	pub fn transient(mut self) -> Self {
		self.persistence = Some(Persistence::Transient);
		self
	}

	pub fn send(mut self) {
		self.message.persistence = self
			.persistence
			.unwrap_or_else(|| Persistence::for_level(self.message.level));
		self.bus.emit(self.message);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::prompt::{Choice, InputValue};

	#[derive(Debug, PartialEq, Eq)]
	enum Answered {
		Download,
		Folder,
	}

	fn prompt() -> Prompt<Answered> {
		Prompt::new("Java required", "No Java 21 runtime was found.")
			.option(
				Choice::primary("download", "Download").picks_selection("java-vendor"),
				Answered::Download,
			)
			.option(
				Choice::new("folder", "Choose folder").picks_folder("Pick a folder"),
				Answered::Folder,
			)
			.dismiss("Cancel")
	}

	/// The whole point of the redesign: the caller gets its own enum back, not
	/// a string it has to re-match.
	#[tokio::test]
	async fn ask_maps_the_answer_back_to_the_callers_value() {
		let (bus, mut rx) = EventBus::channel();

		let task = tokio::spawn(async move { bus.ask(prompt()).await });

		let Some(Event::Notification(Notification::Prompt(request))) = rx.recv().await else {
			panic!("expected a prompt");
		};
		assert_eq!(request.choices.len(), 2);
		assert_eq!(request.dismiss.as_deref(), Some("Cancel"));
		request
			.reply
			.send(Some(Answer::new("folder").with_folder("/opt/jdk21")))
			.unwrap();

		let chosen = task.await.unwrap().unwrap().unwrap();
		assert_eq!(chosen.value, Answered::Folder);
		assert_eq!(chosen.folder(), Some(std::path::Path::new("/opt/jdk21")));
	}

	#[tokio::test]
	async fn selection_input_round_trips() {
		let (bus, mut rx) = EventBus::channel();
		let task = tokio::spawn(async move { bus.ask(prompt()).await });

		let Some(Event::Notification(Notification::Prompt(request))) = rx.recv().await else {
			panic!("expected a prompt");
		};
		request
			.reply
			.send(Some(Answer::new("download").with_selection("adoptium")))
			.unwrap();

		let chosen = task.await.unwrap().unwrap().unwrap();
		assert_eq!(chosen.value, Answered::Download);
		assert_eq!(chosen.selection(), Some("adoptium"));
		assert_eq!(
			chosen.input,
			Some(InputValue::Selection("adoptium".to_string()))
		);
	}

	#[tokio::test]
	async fn dismissing_yields_none() {
		let (bus, mut rx) = EventBus::channel();
		let task = tokio::spawn(async move { bus.ask(prompt()).await });

		let Some(Event::Notification(Notification::Prompt(request))) = rx.recv().await else {
			panic!("expected a prompt");
		};
		request.reply.send(None).unwrap();

		assert!(task.await.unwrap().unwrap().is_none());
	}

	/// A front-end answering with a choice that was never offered is a bug, and
	/// must not be silently folded into "the user cancelled".
	#[tokio::test]
	async fn unknown_choice_is_an_error_not_a_dismissal() {
		let (bus, mut rx) = EventBus::channel();
		let task = tokio::spawn(async move { bus.ask(prompt()).await });

		let Some(Event::Notification(Notification::Prompt(request))) = rx.recv().await else {
			panic!("expected a prompt");
		};
		request.reply.send(Some(Answer::new("nonsense"))).unwrap();

		assert_eq!(
			task.await.unwrap().unwrap_err(),
			EventError::UnknownChoice("nonsense")
		);
	}

	/// Dropping the UI mid-prompt must unblock the waiting task rather than
	/// hanging it forever.
	#[tokio::test]
	async fn dropping_the_receiver_unblocks_the_asker() {
		let (bus, rx) = EventBus::channel();
		let task = tokio::spawn(async move { bus.ask(prompt()).await });
		drop(rx);

		assert_eq!(task.await.unwrap().unwrap_err(), EventError::BusClosed);
	}

	#[test]
	fn emitting_on_a_closed_bus_does_not_panic() {
		let (bus, rx) = EventBus::channel();
		drop(rx);

		assert!(!bus.is_open());
		bus.notify("gone").body("nobody is listening").error().send();
		bus.game_log(1, "still fine");
	}

	#[tokio::test]
	async fn notification_builder_defaults_to_info_with_an_empty_body() {
		let (bus, mut rx) = EventBus::channel();
		bus.notify("Done").send();

		let Some(Event::Notification(Notification::Message(message))) = rx.recv().await else {
			panic!("expected a message");
		};
		assert_eq!(message.title, "Done");
		assert_eq!(message.body, "");
		assert_eq!(message.level, Level::Info);
		assert_eq!(message.persistence, Persistence::Transient);
	}

	async fn persistence_of(build: impl FnOnce(&EventBus)) -> Persistence {
		let (bus, mut rx) = EventBus::channel();
		build(&bus);

		let Some(Event::Notification(Notification::Message(message))) = rx.recv().await else {
			panic!("expected a message");
		};
		message.persistence
	}

	/// The default that keeps the center worth opening: an ordinary "done"
	/// notice is seen and forgotten, a failure is filed.
	#[tokio::test]
	async fn the_level_decides_when_the_caller_does_not() {
		assert_eq!(
			persistence_of(|bus| bus.notify("Copied").send()).await,
			Persistence::Transient
		);
		assert_eq!(
			persistence_of(|bus| bus.notify("Install failed").error().send()).await,
			Persistence::Persistent
		);
	}

	/// Both overrides exist because both defaults are wrong somewhere: a failed
	/// clipboard copy is an error worth no follow-up, and "Update available" is
	/// an info the user will want to find again.
	#[tokio::test]
	async fn an_explicit_choice_wins_whichever_order_it_is_made_in() {
		assert_eq!(
			persistence_of(|bus| bus.notify("Copy failed").error().transient().send()).await,
			Persistence::Transient
		);
		assert_eq!(
			persistence_of(|bus| bus.notify("Copy failed").transient().error().send()).await,
			Persistence::Transient
		);
		assert_eq!(
			persistence_of(|bus| bus.notify("Update available").persistent().send()).await,
			Persistence::Persistent
		);
	}
}
