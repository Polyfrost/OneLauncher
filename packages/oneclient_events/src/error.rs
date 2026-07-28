pub type EventResult<T> = Result<T, EventError>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EventError {
	/// The receiving end went away: the UI shut down, or was never started.
	#[error("the event bus is closed")]
	BusClosed,

	/// The UI answered a prompt with a choice that was not offered. Only
	/// reachable from a buggy front-end, but it is a wrong answer rather than a
	/// missing one, so it must not be silently read as "dismissed".
	#[error("prompt answered with an unknown choice '{0}'")]
	UnknownChoice(&'static str),
}
