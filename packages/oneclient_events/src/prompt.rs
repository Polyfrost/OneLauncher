//! Asking the user a question and getting a typed answer back.
//!
//! The previous design had one closed `UserChoice` enum listing every answer
//! any prompt could produce, including `Install { vendor: JavaVendor }`, which
//! meant the event layer depended on the Java subsystem that depended on it.
//!
//! Here a prompt carries its own [`Choice`] list. The wire format is untyped
//! (a choice id plus optional collected input), but [`Prompt<T>`] pairs each
//! choice with a caller-defined value and hands that value back, so call sites
//! match on their own enum rather than on strings.

use std::path::{Path, PathBuf};

use tokio::sync::oneshot;

/// How prominently the UI should render a choice. Only a hint; the event layer
/// has no business knowing what a button looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChoiceStyle {
	/// The recommended action.
	Primary,
	#[default]
	Secondary,
	/// Destructive; render with a warning treatment.
	Danger,
}

/// Something the UI must collect from the user before the choice can be
/// answered. Without this a prompt could only return "which button", and
/// answers like "…and here is the folder they picked" would have no home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChoiceInput {
	/// Open a directory picker. `title` is the dialog's title.
	Folder { title: String },
	/// Collect a caller-defined selection, e.g. a Java vendor chosen from a
	/// list the UI fetches live. `hint` names the kind of thing being picked so
	/// the front-end can route to the right picker; its meaning belongs to
	/// whoever built the prompt.
	Selection { hint: String },
}

/// What the UI collected for a [`ChoiceInput`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputValue {
	Folder(PathBuf),
	Selection(String),
}

/// One answer the user can give.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
	/// Stable identifier, matched when the answer comes back. A `&'static str`
	/// rather than a `String` because every id in practice is a literal, and it
	/// keeps `Choice` cheap to clone onto the wire.
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

	/// Require a directory before this choice can be answered.
	#[must_use]
	pub fn picks_folder(mut self, title: impl Into<String>) -> Self {
		self.input = Some(ChoiceInput::Folder {
			title: title.into(),
		});
		self
	}

	/// Require a caller-defined selection before this choice can be answered.
	#[must_use]
	pub fn picks_selection(mut self, hint: impl Into<String>) -> Self {
		self.input = Some(ChoiceInput::Selection { hint: hint.into() });
		self
	}
}

/// A question, its answers, and what each answer means to the caller.
///
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

	/// Add a choice and the value returned if the user picks it.
	pub fn option(mut self, choice: Choice, value: T) -> Self {
		self.options.push((choice, value));
		self
	}

	/// Offer an explicit way out, labelled `label`. Dismissing resolves the
	/// prompt to `Ok(None)`; a prompt without this can still be dismissed by
	/// closing it, so callers must always handle `None`.
	pub fn dismiss(mut self, label: impl Into<String>) -> Self {
		self.dismiss = Some(label.into());
		self
	}
}

/// What the user picked: the caller's own value, plus anything the UI collected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chosen<T> {
	pub value: T,
	pub input: Option<InputValue>,
}

impl<T> Chosen<T> {
	/// The directory picked for a [`ChoiceInput::Folder`] choice.
	pub fn folder(&self) -> Option<&Path> {
		match &self.input {
			Some(InputValue::Folder(path)) => Some(path),
			_ => None,
		}
	}

	/// The selection made for a [`ChoiceInput::Selection`] choice.
	pub fn selection(&self) -> Option<&str> {
		match &self.input {
			Some(InputValue::Selection(value)) => Some(value),
			_ => None,
		}
	}
}

/// The untyped prompt as it crosses the bus. The UI renders `choices` and
/// replies with the id of the one picked; `T` never leaves the caller's crate.
#[derive(Debug)]
pub struct PromptRequest {
	pub title: String,
	pub body: String,
	pub choices: Vec<Choice>,
	pub dismiss: Option<String>,
	pub reply: oneshot::Sender<Option<Answer>>,
}

/// The UI's reply. `None` on the wire means dismissed.
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
