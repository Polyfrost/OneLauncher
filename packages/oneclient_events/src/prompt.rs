//! A prompt carries its own [`Choice`] list
//! The wire format is untyped (a choice id plus optional collected input) but
//! [`Prompt<T>`] pairs each choice with a caller-defined value keeping this
//! crate free of any dependency on the subsystems that raise prompts

use std::path::{Path, PathBuf};

use tokio::sync::oneshot;

/// Only a hint the event layer has no business knowing what a button looks like
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChoiceStyle {
	Primary,
	#[default]
	Secondary,
	Danger,
}

/// Something the UI must collect before the choice can be answered
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChoiceInput {
	Folder { title: String },
	/// `hint` names the kind of thing being picked so the front-end can route to
	/// the right picker its meaning belongs to whoever built the prompt
	Selection { hint: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputValue {
	Folder(PathBuf),
	Selection(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
	/// Stable identifier matched when the answer comes back
	pub id: &'static str,
	pub label: String,
	pub style: ChoiceStyle,
	pub input: Option<ChoiceInput>,
}

impl Choice {
	pub fn new(id: &'static str, label: impl Into<String>) -> Self {
		Self {
			id,
			label: label.into(),
			style: ChoiceStyle::Secondary,
			input: None,
		}
	}

	#[must_use]
	pub fn primary(id: &'static str, label: impl Into<String>) -> Self {
		Self::new(id, label).style(ChoiceStyle::Primary)
	}

	#[must_use]
	pub fn danger(id: &'static str, label: impl Into<String>) -> Self {
		Self::new(id, label).style(ChoiceStyle::Danger)
	}

	#[must_use]
	pub fn style(mut self, style: ChoiceStyle) -> Self {
		self.style = style;
		self
	}

	#[must_use]
	pub fn picks_folder(mut self, title: impl Into<String>) -> Self {
		self.input = Some(ChoiceInput::Folder {
			title: title.into(),
		});
		self
	}

	#[must_use]
	pub fn picks_selection(mut self, hint: impl Into<String>) -> Self {
		self.input = Some(ChoiceInput::Selection { hint: hint.into() });
		self
	}
}

/// ```ignore
/// enum JavaAnswer { Download, PickFolder }
///
/// let answer = events.ask(
///     Prompt::new("Java required", "No Java 21 runtime was found.")
///         .option(
///             Choice::primary("download", "Download").picks_selection("java-vendor"),
///             JavaAnswer::Download,
///         )
///         .option(
///             Choice::new("folder", "Choose folder").picks_folder("Select a Java 21 folder"),
///             JavaAnswer::PickFolder,
///         )
///         .dismiss("Cancel"),
/// ).await?;
/// ```
#[must_use = "a prompt is not shown until it is passed to `EventBus::ask`"]
pub struct Prompt<T> {
	pub(crate) title: String,
	pub(crate) body: String,
	pub(crate) options: Vec<(Choice, T)>,
	pub(crate) dismiss: Option<String>,
}

impl<T> Prompt<T> {
	pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
		Self {
			title: title.into(),
			body: body.into(),
			options: Vec::new(),
			dismiss: None,
		}
	}

	pub fn option(mut self, choice: Choice, value: T) -> Self {
		self.options.push((choice, value));
		self
	}

	/// A prompt without this can still be dismissed by closing it so callers
	/// must always handle `None`
	pub fn dismiss(mut self, label: impl Into<String>) -> Self {
		self.dismiss = Some(label.into());
		self
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chosen<T> {
	pub value: T,
	pub input: Option<InputValue>,
}

impl<T> Chosen<T> {
	pub fn folder(&self) -> Option<&Path> {
		match &self.input {
			Some(InputValue::Folder(path)) => Some(path),
			_ => None,
		}
	}

	pub fn selection(&self) -> Option<&str> {
		match &self.input {
			Some(InputValue::Selection(value)) => Some(value),
			_ => None,
		}
	}
}

/// The untyped prompt as it crosses the bus `T` never leaves the caller's crate
#[derive(Debug)]
pub struct PromptRequest {
	pub title: String,
	pub body: String,
	pub choices: Vec<Choice>,
	pub dismiss: Option<String>,
	pub reply: oneshot::Sender<Option<Answer>>,
}

/// `None` on the wire means dismissed
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
	pub id: &'static str,
	pub input: Option<InputValue>,
}

impl Answer {
	pub fn new(id: &'static str) -> Self {
		Self { id, input: None }
	}

	#[must_use]
	pub fn with_folder(mut self, path: impl Into<PathBuf>) -> Self {
		self.input = Some(InputValue::Folder(path.into()));
		self
	}

	#[must_use]
	pub fn with_selection(mut self, value: impl Into<String>) -> Self {
		self.input = Some(InputValue::Selection(value.into()));
		self
	}
}
