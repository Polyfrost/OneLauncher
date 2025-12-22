use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;
use tracing::{error, warn};
use uuid::Uuid;

use crate::LauncherResult;
use crate::api::ingress::IngressPayload;
use crate::api::proxy::LauncherProxy;
use crate::api::proxy::event::LauncherEvent;
use crate::api::proxy::message::MessageLevel;
use crate::api::tauri::LauncherEventEmitter;

#[derive(Debug)]
pub struct ProxyTauri {
	emitter: Arc<LauncherEventEmitter<tauri::Wry>>,
	handle: AppHandle,

	ingress_queue: Arc<IngressQueue>,
}

impl ProxyTauri {
	#[must_use]
	pub fn new(handle: AppHandle) -> Self {
		tracing::debug!("using tauri bridge");
		tracing::debug!(
			"webview version: {}",
			tauri::webview_version().unwrap_or_else(|_| "unknown".into())
		);

		let emitter = Arc::new(LauncherEventEmitter::new(handle.clone()));

		Self {
			emitter: Arc::clone(&emitter),
			handle,
			ingress_queue: IngressQueue::new(Arc::clone(&emitter), Duration::from_millis(100)),
		}
	}
}

#[async_trait::async_trait]
impl LauncherProxy for ProxyTauri {
	async fn send_event(&self, event: LauncherEvent) -> LauncherResult<()> {
		if let LauncherEvent::Message(message) = &event {
			match message.level {
				MessageLevel::Info => {}
				MessageLevel::Warn => warn!("{}", message.message),
				MessageLevel::Error => error!("{}", message.message),
			}
		}

		Ok(match event {
			LauncherEvent::Ingress(ingress) => {
				self.ingress_queue.push(ingress);

				()
			}
			LauncherEvent::Message(message) => self.emitter.message(message)?,
			LauncherEvent::Process(process) => self.emitter.process(process)?,
		})
	}

	#[tracing::instrument]
	fn hide_main_window(&self) -> LauncherResult<()> {
		if let Some(window) = self.handle.get_webview_window("main") {
			Ok(window.hide()?)
		} else {
			warn!("main window not found");
			Ok(())
		}
	}

	#[tracing::instrument]
	fn show_main_window(&self) -> LauncherResult<()> {
		if let Some(window) = self.handle.get_webview_window("main") {
			Ok(window.show()?)
		} else {
			warn!("main window not found");
			Ok(())
		}
	}
}

#[derive(Debug)]
pub struct IngressQueue {
	tx: mpsc::UnboundedSender<IngressPayload>,
}

impl IngressQueue {
	pub fn new(
		emitter: Arc<LauncherEventEmitter<tauri::Wry>>,
		debounce_duration: Duration,
	) -> Arc<Self> {
		let (tx, mut rx) = mpsc::unbounded_channel::<IngressPayload>();

		tokio::spawn(async move {
			let mut pending: HashMap<Uuid, IngressPayload> = HashMap::new();

			let mut tick = tokio::time::interval(debounce_duration);
			tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

			loop {
				tokio::select! {
					Some(payload) = rx.recv() => {
						// last write wins
						pending.insert(payload.id, payload);
					}

					_ = tick.tick() => {
						if pending.is_empty() {
							continue;
						}

						for (_, payload) in pending.drain() {
							let emitter = Arc::clone(&emitter);
							tokio::spawn(async move {
								let _ = emitter.ingress(payload);
							});
						}
					}
				}
			}
		});

		Arc::new(Self { tx })
	}

	#[inline]
	pub fn push(&self, payload: IngressPayload) {
		let _ = self.tx.send(payload);
	}
}
