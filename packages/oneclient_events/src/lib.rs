//! This crate is a leaf
//! It must not learn what a Java vendor a package provider or a cluster is
//! prompts carry caller-defined values through [`Prompt<T>`] and
//! [`TaskCategory`] holds display labels rather than types

mod bus;
mod error;
mod event;

pub mod progress;
pub mod prompt;

pub use bus::{EventBus, EventReceiver, NotificationBuilder};
pub use error::{EventError, EventResult};
pub use event::{
	Event, GameEvent, LaunchStage, Level, Message, Notification, ProgressEvent, Signal,
};
pub use progress::{
	GroupedProgressChild, GroupedProgressEvent, GroupedProgressSession, TaskCategory, TaskPhase,
};
pub use prompt::{Answer, Choice, ChoiceInput, ChoiceStyle, Chosen, InputValue, Prompt, PromptRequest};
