use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::error::{EventError, EventResult};
use crate::event::{
	ChatEvent, Event, GameEvent, LaunchStage, Level, Message, Notification, ProgressEvent, Signal,
};
use crate::prompt::{Answer, Chosen, Prompt, PromptRequest};

/// Cheap to clone every clone feeds the same channel
/// Hand clones to subsystems rather than wrapping this in an `Arc`
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

	#[must_use]
	pub fn channel() -> (Self, EventReceiver) {
		let (tx, rx) = mpsc::unbounded_channel();
		(Self::new(tx), rx)
	}

	/// A closed bus is logged rather than returned use [`EventBus::ask`] when
	/// you need to know whether the other end is alive
	pub fn emit(&self, event: impl Into<Event>) {
		if let Err(err) = self.tx.send(event.into()) {
			tracing::debug!("dropping event, bus is closed: {err}");
		}
	}

	#[must_use]
	pub fn is_open(&self) -> bool {
		!self.tx.is_closed()
	}

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
			},
		}
	}

	/// Repeated calls with the same `id` replace one another
	pub fn progress(&self, id: Uuid, label: impl Into<String>, current: u64, total: u64) {
		self.emit(ProgressEvent::Update {
			id,
			label: label.into(),
			current,
			total,
		});
	}

	pub fn finish_progress(&self, id: Uuid, title: impl Into<String>, body: impl Into<String>) {
		self.emit(ProgressEvent::Complete {
			id,
			title: title.into(),
			body: body.into(),
		});
	}

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

	pub fn chat(&self, event: ChatEvent) {
		self.emit(event);
	}

	pub fn signal(&self, signal: Signal) {
		self.emit(signal);
	}

	/// Returns `Ok(None)` when the prompt was dismissed
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

		// A dropped sender means the UI went away without answering
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

	pub fn send(self) {
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
	}
}
