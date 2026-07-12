pub use data::{
    GroupedProgressEvent, LaunchStage, MicrosoftLoginStatus, Notification, NotificationLevel,
    PromptKind, TaskPhase, UserChoice,
};
pub use grouped::{GroupedProgressChild, GroupedProgressSession};
pub use service::NotificationService;

mod data;
mod grouped;
mod service;

#[derive(Debug, thiserror::Error)]
pub enum NotificationError {
    #[error("Notification service is down")]
    ServiceDown,
    #[error("Notification prompt timed out")]
    PromptTimedOut,
}
