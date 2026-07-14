use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::notification::{
	LaunchStage, MicrosoftLoginStatus, Notification, NotificationError, NotificationLevel,
	PromptKind, UserChoice,
};

#[derive(Clone)]
pub struct NotificationService {
	channel: mpsc::UnboundedSender<Notification>,
}

impl NotificationService {
	pub fn new(channel: mpsc::UnboundedSender<Notification>) -> Self {
		Self { channel }
	}

	pub fn channel(&self) -> &mpsc::UnboundedSender<Notification> {
		&self.channel
	}

	pub fn send_message(&self, title: &str, body: &str, level: NotificationLevel) {
		if let Err(err) = self.channel.send(Notification::Message {
			title: title.to_string(),
			body: body.to_string(),
			level,
		}) {
			tracing::error!("{err}");
		}
	}

	pub fn send_progress(&self, id: &Uuid, label: &str, current: u64, total: u64) {
		if let Err(err) = self.channel.send(Notification::Progress {
			id: *id,
			label: label.to_string(),
			current,
			total,
		}) {
			tracing::error!("{err}");
		}
	}

	#[tracing::instrument(level = "debug", skip(self))]
	pub async fn prompt_java_install(&self, major: u32) -> Result<UserChoice, NotificationError> {
		self.prompt(
			"Java required",
			&format!(
				"No Java {major} runtime was found. Download it automatically, choose an existing installation folder, or cancel?"
			),
			PromptKind::JavaInstall { major },
		)
		.await
	}

	#[tracing::instrument(level = "debug", skip(self))]
	pub async fn prompt_update(&self, version: &str) -> Result<UserChoice, NotificationError> {
		self.prompt(
			"Update available",
			&format!("OneClient {version} is ready to install. Download and install it now?"),
			PromptKind::Update,
		)
		.await
	}

	#[tracing::instrument(level = "debug", skip(self, question))]
	pub async fn prompt(
		&self,
		title: &str,
		question: &str,
		kind: PromptKind,
	) -> Result<UserChoice, NotificationError> {
		let (reply_tx, reply_rx) = oneshot::channel();

		self.channel
			.send(Notification::Prompt {
				title: title.to_string(),
				question: question.to_string(),
				kind,
				reply_tx,
			})
			.map_err(|_| NotificationError::ServiceDown)?;

		reply_rx.await.map_err(|_| NotificationError::ServiceDown)
	}

	pub fn microsoft_login_status(&self, status: Option<MicrosoftLoginStatus>) {
		if let Err(err) = self
			.channel
			.send(Notification::MicrosoftLoginStatus(status))
		{
			tracing::error!("{err}");
		}
	}

	pub fn send_info(&self, title: &str, body: &str) {
		self.send_message(title, body, NotificationLevel::Info);
	}

	pub fn send_error(&self, title: &str, body: &str) {
		self.send_message(title, body, NotificationLevel::Error);
	}

	pub fn game_stage(&self, cluster_id: i64, stage: LaunchStage) {
		if let Err(err) = self.channel.send(Notification::GameStage { cluster_id, stage }) {
			tracing::error!("{err}");
		}
	}

	pub fn game_failed(&self, cluster_id: i64, message: impl Into<String>) {
		if let Err(err) = self.channel.send(Notification::GameFailed {
			cluster_id,
			message: message.into(),
		}) {
			tracing::error!("{err}");
		}
	}

	pub fn game_log(&self, cluster_id: i64, line: impl Into<String>) {
		if let Err(err) = self.channel.send(Notification::GameLog {
			cluster_id,
			line: line.into(),
		}) {
			tracing::error!("{err}");
		}
	}

	pub fn send_grouped(&self, event: super::data::GroupedProgressEvent) {
		if let Err(err) = self
			.channel
			.send(super::data::Notification::GroupedProgress(event))
		{
			tracing::error!("failed to send grouped progress: {err}");
		}
	}

	pub fn invalidate_clusters(&self) {
		if let Err(err) = self
			.channel
			.send(Notification::InvalidateClusters)
		{
			tracing::error!("failed to invalidate clusters: {err}");
		}
	}

	pub fn invalidate_java(&self) {
		if let Err(err) = self.channel.send(Notification::InvalidateJava) {
			tracing::error!("failed to invalidate java: {err}");
		}
	}

	pub fn sync_complete(&self) {
		if let Err(err) = self.channel.send(Notification::SyncComplete) {
			tracing::error!("failed to signal sync complete: {err}");
		}
	}
}
