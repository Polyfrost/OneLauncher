use onelauncher_core::error::LauncherError;
use serde::{Serialize, Serializer, ser::SerializeStruct};
use specta::Type;

#[derive(thiserror::Error, Debug, Type)]
#[serde(tag = "kind", content = "data")]
pub enum SerializableError {
	#[error(transparent)]
	CoreError(#[serde(skip)] #[from] onelauncher_core::error::LauncherError),
}

pub type SerializableResult<T> = std::result::Result<T, SerializableError>;

macro_rules! impl_serialize {
    ($($variant:ident),* $(,)?) => {
        impl Serialize for SerializableError {
            fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                match self {
                    SerializableError::CoreError(error) => {
						display_tracing_error(error);

                        let mut state = serializer.serialize_struct("CoreError", 2)?;
                        state.serialize_field("field_name", "CoreError")?;
                        state.serialize_field("message", &error.to_string())?;
                        state.end()
                    }
                    $(
                        SerializableError::$variant(message) => {
                            let mut state = serializer.serialize_struct(stringify!($variant), 2)?;
                            state.serialize_field("field_name", stringify!($variant))?;
                            state.serialize_field("message", &message.to_string())?;
                            state.end()
                        },
                    )*
                }
            }
        }
    };
}

impl_serialize! {}

pub fn display_tracing_error(err: &LauncherError) {
    match get_span_trace(err) {
        Some(span_trace) => {
            tracing::error!(error = %err, span_trace = %span_trace);
        }
        None => {
            tracing::error!(error = %err);
        }
    }
}

pub fn get_span_trace<'a>(
    error: &'a (dyn std::error::Error + 'static),
) -> Option<&'a tracing_error::SpanTrace> {
    error.source().and_then(tracing_error::ExtractSpanTrace::span_trace)
}
