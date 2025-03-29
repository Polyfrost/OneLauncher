use std::collections::HashMap;

use indicatif::ProgressBar;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{store::{ingress::IngressPayload, State}, LauncherResult};

use super::LauncherProxy;

#[derive(Default)]
pub struct ProxyCli {
	ingress_feeds: RwLock<HashMap<Uuid, ProgressBar>>,
}

impl ProxyCli {
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}
}

#[async_trait::async_trait]
impl LauncherProxy for ProxyCli {

	async fn send_ingress(&self, ingress: IngressPayload) -> LauncherResult<()> {
		let feeds = self.ingress_feeds.write().await;

		if let Some(progress) = feeds.get(&ingress.id) {
			// progress.set_position(ingress.percent as u64);
			// TODO: customize progress bar
		} else {
			let progress = ProgressBar::new(ingress.total as u64);
			progress.set_message(ingress.message.clone());
			progress.set_position(0);
		}
		Ok(())
	}

	async fn send_message(&self, message: crate::api::proxy::message::MessagePayload) -> LauncherResult<()> {
		println!("[{:?}] {:?}", message.level, message.message);
		Ok(())
	}

}
