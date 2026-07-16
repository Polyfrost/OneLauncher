use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader};

use crate::game::session::SessionRecorder;
use crate::notification::NotificationService;

const POLL_INTERVAL: Duration = Duration::from_millis(200);

pub(crate) struct LogTail {
	stop: Arc<AtomicBool>,
	handle: tokio::task::JoinHandle<()>,
}

impl LogTail {
	/// Stop tailing, but only after one last pass: the lines explaining *why*
	/// the game exited are written moments before it does, and would be lost if
	/// the tail stopped the instant the process did.
	pub(crate) async fn stop(self) {
		self.stop.store(true, Ordering::Relaxed);
		let _ = self.handle.await;
	}
}

pub(crate) fn spawn_log_tail(
	cluster_id: i64,
	path: PathBuf,
	notifier: NotificationService,
	recorder: Option<SessionRecorder>,
) -> LogTail {
	let stop = Arc::new(AtomicBool::new(false));
	let flag = Arc::clone(&stop);

	let handle = tokio::spawn(async move {
		if let Err(err) = tail(cluster_id, &path, &notifier, recorder.as_ref(), &flag).await {
			tracing::warn!(cluster_id, path = %path.display(), error = %err, "game log tail stopped");
		}
	});

	LogTail { stop, handle }
}

async fn tail(
	cluster_id: i64,
	path: &PathBuf,
	notifier: &NotificationService,
	recorder: Option<&SessionRecorder>,
	stop: &AtomicBool,
) -> std::io::Result<()> {
	// The game may not have created the file yet on a re-adopt, and on launch it
	// exists but is empty. Either way, wait for it rather than giving up.
	let file = loop {
		match tokio::fs::File::open(path).await {
			Ok(file) => break file,
			Err(_) if !stop.load(Ordering::Relaxed) => {
				tokio::time::sleep(POLL_INTERVAL).await;
			}
			Err(err) => return Err(err),
		}
	};

	let mut reader = BufReader::new(file);
	let mut line = String::new();
	let mut offset = 0u64;

	loop {
		line.clear();
		let read = reader.read_line(&mut line).await?;

		if read == 0 {
			// A final pass has already run by the time the flag is seen here,
			// since the read above returned EOF on a fully drained file.
			if stop.load(Ordering::Relaxed) {
				return Ok(());
			}
			// The log is truncated when a cluster relaunches; if the file
			// shrank, our offset is past the end and we would never read again.
			let len = polyio::stat(path).await.map(|m| m.len()).unwrap_or(offset);
			if len < offset {
				reader.seek(std::io::SeekFrom::Start(0)).await?;
				offset = 0;
			}
			tokio::time::sleep(POLL_INTERVAL).await;
			continue;
		}

		offset += read as u64;

		// A partial line means the game is mid-write; wait for the newline
		// rather than emitting half a line and re-emitting the rest.
		if !line.ends_with('\n') {
			// Unless the game is already gone — a process killed mid-write
			// leaves a newline that will never arrive, and waiting for it would
			// hang the caller of `stop()` forever.
			if stop.load(Ordering::Relaxed) {
				emit(cluster_id, &line, notifier, recorder).await;
				return Ok(());
			}
			reader.seek(std::io::SeekFrom::Start(offset - read as u64)).await?;
			offset -= read as u64;
			tokio::time::sleep(POLL_INTERVAL).await;
			continue;
		}

		emit(cluster_id, &line, notifier, recorder).await;
	}
}

async fn emit(
	cluster_id: i64,
	line: &str,
	notifier: &NotificationService,
	recorder: Option<&SessionRecorder>,
) {
	// Blank lines are kept: the game prints them, and the console should read
	// the way the log does.
	let text = line.trim_end_matches(['\n', '\r']).to_string();
	if let Some(recorder) = recorder {
		recorder.observe(&text).await;
	}
	notifier.game_log(cluster_id, text);
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::notification::Notification;
	use tokio::io::AsyncWriteExt;
	use tokio::sync::mpsc;

	struct Harness {
		dir: async_tempfile::TempDir,
		rx: mpsc::UnboundedReceiver<Notification>,
		notifier: NotificationService,
	}

	impl Harness {
		/// The temp dir removes itself on drop, so nothing here needs cleaning up.
		async fn new() -> Self {
			let (tx, rx) = mpsc::unbounded_channel();
			let dir = polyio::tempdir().await.expect("temp dir");
			Self {
				dir,
				rx,
				notifier: NotificationService::new(tx),
			}
		}

		fn path(&self) -> PathBuf {
			self.dir.dir_path().join("cluster-output.log")
		}

		/// Drain whatever the tail has emitted so far.
		fn drain(&mut self) -> Vec<String> {
			let mut out = Vec::new();
			while let Ok(Notification::GameLog { line, .. }) = self.rx.try_recv() {
				out.push(line);
			}
			out
		}
	}

	/// Give the tail a few poll cycles to notice what we wrote.
	async fn settle() {
		tokio::time::sleep(POLL_INTERVAL * 4).await;
	}

	#[tokio::test]
	async fn follows_lines_appended_after_start() {
		let mut h = Harness::new().await;
		let path = h.path();
		polyio::write(&path, "[12:00:00] first\n").await.unwrap();

		let tail = spawn_log_tail(7, path.clone(), h.notifier.clone(), None);
		settle().await;

		let mut file = tokio::fs::OpenOptions::new()
			.append(true)
			.open(&path)
			.await
			.unwrap();
		file.write_all(b"[12:00:01] second\n").await.unwrap();
		file.flush().await.unwrap();
		settle().await;
		tail.stop().await;

		assert_eq!(h.drain(), vec!["[12:00:00] first", "[12:00:01] second"]);
	}

	#[tokio::test]
	async fn waits_for_a_file_that_does_not_exist_yet() {
		let mut h = Harness::new().await;
		let path = h.path();

		let tail = spawn_log_tail(7, path.clone(), h.notifier.clone(), None);
		settle().await;
		polyio::write(&path, "[12:00:00] late\n").await.unwrap();
		settle().await;
		tail.stop().await;

		assert_eq!(h.drain(), vec!["[12:00:00] late"]);
	}

	#[tokio::test]
	async fn holds_back_a_partial_line_until_its_newline_lands() {
		let mut h = Harness::new().await;
		let path = h.path();
		polyio::write(&path, "[12:00:00] half").await.unwrap();

		let tail = spawn_log_tail(7, path.clone(), h.notifier.clone(), None);
		settle().await;
		// A line mid-write must not be emitted, or it would arrive twice.
		assert!(h.drain().is_empty());

		let mut file = tokio::fs::OpenOptions::new()
			.append(true)
			.open(&path)
			.await
			.unwrap();
		file.write_all(b" and half\n").await.unwrap();
		file.flush().await.unwrap();
		settle().await;
		tail.stop().await;

		assert_eq!(h.drain(), vec!["[12:00:00] half and half"]);
	}

	#[tokio::test]
	async fn recovers_when_the_log_is_truncated() {
		let mut h = Harness::new().await;
		let path = h.path();
		polyio::write(&path, "[12:00:00] old session line\n").await.unwrap();

		let tail = spawn_log_tail(7, path.clone(), h.notifier.clone(), None);
		settle().await;
		assert_eq!(h.drain(), vec!["[12:00:00] old session line"]);

		// A relaunch truncates the log; the tail would otherwise sit forever at
		// an offset past the new end and go silent.
		polyio::write(&path, "[13:00:00] new\n").await.unwrap();
		settle().await;
		tail.stop().await;

		assert_eq!(h.drain(), vec!["[13:00:00] new"]);
	}

	#[tokio::test]
	async fn stops_even_when_the_last_line_has_no_newline() {
		let mut h = Harness::new().await;
		let path = h.path();
		// A process killed mid-write leaves a newline that never arrives.
		polyio::write(&path, "[12:00:00] cut off mid-writ").await.unwrap();

		let tail = spawn_log_tail(7, path.clone(), h.notifier.clone(), None);
		settle().await;

		// Waiting for that newline would hang the exit path forever.
		let stopped = tokio::time::timeout(Duration::from_secs(5), tail.stop()).await;
		assert!(stopped.is_ok(), "tail.stop() hung on an unterminated final line");
		assert_eq!(h.drain(), vec!["[12:00:00] cut off mid-writ"]);
	}

	#[tokio::test]
	async fn emits_the_final_lines_written_just_before_exit() {
		let mut h = Harness::new().await;
		let path = h.path();
		polyio::write(&path, "[12:00:00] Stopping!\n").await.unwrap();

		// Stopping immediately, as the exit path does, must still flush the
		// reason the game exited rather than race past it.
		let tail = spawn_log_tail(7, path.clone(), h.notifier.clone(), None);
		tail.stop().await;

		assert_eq!(h.drain(), vec!["[12:00:00] Stopping!"]);
	}
}
