use std::collections::{HashMap, VecDeque};
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tokio::sync::{Mutex, Notify};
use tokio::time::Instant;
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

		Self {
			emitter: Arc::new(LauncherEventEmitter::new(handle.clone())),
			handle,
			ingress_queue: IngressQueue::new(),
		}
		.init()
	}

	fn init(self) -> Self {
		let ingress_queue = Arc::clone(&self.ingress_queue);
		let emitter = Arc::clone(&self.emitter);

		// spawn the processor on another thread
		tokio::spawn(async move {
			ingress_queue
				.process_queue(move |payload| {
					let _ = emitter.ingress(payload);
				})
				.await;
		});

		self
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
				self.ingress_queue.push(ingress).await;

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

pub struct IngressQueue {
	queue: Mutex<VecDeque<IngressPayload>>,
	last_sent: Mutex<HashMap<Uuid, Instant>>,
	notify: Notify,
	debounce: Duration,
}

impl Debug for IngressQueue {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("IngressQueue")
			.field("debounce", &self.debounce)
			.finish()
	}
}

impl IngressQueue {
	pub fn new() -> Arc<Self> {
		Arc::new(Self {
			queue: Mutex::new(VecDeque::new()),
			last_sent: Mutex::new(HashMap::new()),
			notify: Notify::new(),
			debounce: Duration::from_millis(100),
		})
	}

	pub async fn push(&self, payload: IngressPayload) {
		let now = Instant::now();

		// check last sent time for debouncing
		let mut last_sent = self.last_sent.lock().await;
		if let Some(&instant) = last_sent.get(&payload.id) {
			if now.duration_since(instant) < self.debounce {
				// skip update or merge with existing queued message
				let mut queue = self.queue.lock().await;
				if let Some(existing) = queue.iter_mut().find(|q| q.id == payload.id) {
					existing.percent = payload.percent.or(existing.percent);
					existing.message = payload.message;
					return;
				}
			}
		}

		last_sent.insert(payload.id, now);

		let mut queue = self.queue.lock().await;
		queue.push_back(payload);
		self.notify.notify_one(); // wake processor
	}

	pub async fn process_queue<F>(self: Arc<Self>, mut sender: F)
	where
		F: FnMut(IngressPayload) + Send + 'static,
	{
		loop {
			let queued = {
				let mut queue = self.queue.lock().await;
				queue.pop_front()
			};

			if let Some(payload) = queued {
				sender(payload);
			} else {
				// wait until next push, no CPU spin
				self.notify.notified().await;
			}
		}
	}
}
