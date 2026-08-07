pub type EventResult<T> = Result<T, EventError>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EventError {
	#[error("the event bus is closed")]
	BusClosed,

	/// A wrong answer rather than a missing one so it must not be read as
	/// "dismissed"
	#[error("prompt answered with an unknown choice '{0}'")]
	UnknownChoice(&'static str),
}
