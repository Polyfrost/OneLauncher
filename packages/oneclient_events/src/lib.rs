//! The channel the core talks to the UI over.
//!
//! Every subsystem holds an [`EventBus`] clone and emits [`Event`]s into it;
//! the front-end owns the single receiver and decides what to render. The bus
//! is mostly fire-and-forget. The one exception is [`EventBus::ask`], which
//! blocks the emitting task until the user answers.
//!
//! This crate is a leaf. It must not learn what a Java vendor, a package
//! provider or a cluster is: prompts carry caller-defined values through
//! [`Prompt<T>`], and [`TaskCategory`] holds display labels rather than types.

mod bus;
mod error;
mod event;

pub mod progress;
pub mod prompt;

pub use bus::{EventBus, EventReceiver, NotificationBuilder};
pub use error::{EventError, EventResult};
pub use event::{
	Event, GameEvent, LaunchStage, Level, Message, Notification, Persistence, ProgressEvent, Signal,
};
pub use progress::{
	GroupedProgressChild, GroupedProgressEvent, GroupedProgressSession, TaskCategory, TaskPhase,
};
pub use prompt::{Answer, Choice, ChoiceInput, ChoiceStyle, Chosen, InputValue, Prompt, PromptRequest};
