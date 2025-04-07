use std::collections::HashMap;

use indicatif::ProgressBar;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{constants::CLI_TOTAL_INGRESS, store::ingress::IngressPayload, LauncherResult};

use super::LauncherProxy;

#[derive(Default, Debug)]
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
		let mut feeds = self.ingress_feeds.write().await;

		let completed = ingress.percent.is_none();

		if let Some(progress) = feeds.get(&ingress.id) {
			if completed {
				progress.finish();
			} else {
				#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
				let new_pos = (ingress.percent.unwrap() * CLI_TOTAL_INGRESS as f64).round() as u64;
				progress.set_position(new_pos);
			}
		} else if !completed {
			let progress = ProgressBar::new(CLI_TOTAL_INGRESS);
			progress.set_message(ingress.message.clone());
			progress.set_position(0);
			progress.set_style(indicatif::ProgressStyle::default_bar().template(
                "{spinner:.green}, [{elapsed_precise}] [{bar:.lime/green}] {pos}/{len} {msg}"
            ).unwrap().progress_chars("#>-"));

			feeds.insert(ingress.id, progress);
		}
		Ok(())
	}

	async fn send_message(&self, message: crate::api::proxy::message::MessagePayload) -> LauncherResult<()> {
		println!("[{:?}] {}", message.level, message.message);
		Ok(())
	}

}
