use std::collections::HashMap;

use indicatif::ProgressBar;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::LauncherResult;
use crate::api::processes::ProcessPayloadKind;
use crate::api::proxy::LauncherProxy;
use crate::api::proxy::event::LauncherEvent;
use crate::constants::CLI_TOTAL_INGRESS;

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
	async fn send_event(&self, event: LauncherEvent) -> LauncherResult<()> {
		match event {
			LauncherEvent::Ingress(ingress) => {
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
					progress.set_style(
						indicatif::ProgressStyle::default_bar()
							.template(
								"{spinner:.green}, [{elapsed_precise}] [{bar:.lime/green}] {pos}/{len} {msg}",
							)
							.unwrap()
							.progress_chars("#>-"),
					);

					feeds.insert(ingress.id, progress);
				}
			}
			LauncherEvent::Message(message) => {
				println!("[{:?}] {}", message.level, message.message);
			}
			LauncherEvent::Process(process) => match process.kind {
				ProcessPayloadKind::Starting { command } => {
					println!("Starting process: {command}");
				}
				ProcessPayloadKind::Started { pid, .. } => {
					println!("Process started: {pid}");
				}
				ProcessPayloadKind::Stopped { pid, exit_code } => {
					println!("Process {pid} exited with code {exit_code}");
				}
				ProcessPayloadKind::Output { pid, output } => {
					println!("Process {pid}: {output}");
				}
			},
		}

		Ok(())
	}

	fn hide_main_window(&self) -> LauncherResult<()> {
		Ok(())
	}

	fn show_main_window(&self) -> LauncherResult<()> {
		Ok(())
	}
}
