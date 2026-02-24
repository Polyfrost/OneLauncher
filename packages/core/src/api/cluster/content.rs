use std::path::Path;

use onelauncher_entity::clusters;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::error::LauncherResult;
use crate::store::Dirs;
use crate::utils::io;

const PROCESS_LOG_FILE_NAME: &str = "latest.log";
const TAIL_READ_CHUNK_SIZE: u64 = 64 * 1024;

#[onelauncher_macro::specta]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProcessLogTail {
	pub content: String,
	pub truncated: bool,
}

/// Returns a list of screenshot file names
pub async fn get_screenshots(cluster: &clusters::Model) -> LauncherResult<Vec<String>> {
	let dir = cluster.folder_name.clone();
	let path = Dirs::get_clusters_dir()
		.await?
		.join(dir)
		.join("screenshots");

	if !path.exists() {
		io::create_dir(&path).await?;
		return Ok(Vec::new());
	}

	let mut list = vec![];
	let mut files = io::read_dir(path).await?;
	while let Ok(Some(entry)) = files.next_entry().await {
		list.push(entry.file_name().to_string_lossy().to_string());
	}

	Ok(list)
}

/// Returns a list of world filenames
pub async fn get_worlds(cluster: &clusters::Model) -> LauncherResult<Vec<String>> {
	let dir = cluster.folder_name.clone();
	let path = Dirs::get_clusters_dir().await?.join(dir).join("saves");

	if !path.exists() {
		io::create_dir(&path).await?;
		return Ok(Vec::new());
	}

	let mut list = vec![];
	let mut files = io::read_dir(path).await?;
	while let Ok(Some(entry)) = files.next_entry().await {
		list.push(entry.file_name().to_string_lossy().to_string());
	}

	Ok(list)
}

/// Returns a list of log file names
pub async fn get_logs(cluster: &clusters::Model) -> LauncherResult<Vec<String>> {
	let dir = cluster.folder_name.clone();
	let path = Dirs::get_clusters_dir().await?.join(dir).join("logs");

	if !path.exists() {
		io::create_dir(&path).await?;
		return Ok(Vec::new());
	}

	let mut list = vec![];
	let mut files = io::read_dir(path).await?;
	while let Ok(Some(entry)) = files.next_entry().await {
		if entry.file_type().await.is_ok_and(|ft| ft.is_file()) {
			list.push(entry.file_name().to_string_lossy().to_string());
		}
	}

	list.sort_by(|a, b| {
		if a == "latest.log" {
			std::cmp::Ordering::Less
		} else if b == "latest.log" {
			std::cmp::Ordering::Greater
		} else {
			a.cmp(b)
		}
	});

	Ok(list)
}

/// returns a log from a cluster and file name
pub async fn get_log_by_name(
	cluster: &clusters::Model,
	file_name: &str,
) -> LauncherResult<Option<String>> {
	let dir = cluster.folder_name.clone();
	let path = Dirs::get_clusters_dir()
		.await?
		.join(dir)
		.join("logs")
		.join(file_name);

	if !path.exists() {
		return Ok(None);
	}

	let content = if file_name.ends_with(".gz") {
		io::read_gz_to_string(&path).await?
	} else {
		io::read_to_string(&path).await?
	};

	Ok(Some(content))
}

/// Returns a tail window from a running cluster's `latest.log`.
/// `max_lines = 0` means no line limit (return full content).
pub async fn get_process_log_tail(
	cluster: &clusters::Model,
	max_lines: usize,
) -> LauncherResult<Option<ProcessLogTail>> {
	let dir = cluster.folder_name.clone();
	let path = Dirs::get_clusters_dir()
		.await?
		.join(dir)
		.join("logs")
		.join(PROCESS_LOG_FILE_NAME);

	if !path.exists() {
		return Ok(None);
	}

	let tail = read_process_log_tail(&path, max_lines).await?;

	Ok(Some(tail))
}

async fn read_process_log_tail(
	path: &Path,
	max_lines: usize,
) -> Result<ProcessLogTail, io::IOError> {
	if max_lines == 0 {
		let bytes = tokio::fs::read(path).await?;
		let content = String::from_utf8_lossy(&bytes).to_string();
		return Ok(ProcessLogTail {
			content,
			truncated: false,
		});
	}

	let max_lines = max_lines.max(1);
	let mut file = tokio::fs::File::open(path).await?;
	let file_size = file.metadata().await?.len();

	if file_size == 0 {
		return Ok(ProcessLogTail {
			content: String::new(),
			truncated: false,
		});
	}

	let mut cursor = file_size;
	let mut buffer = Vec::new();
	let mut newline_count = 0usize;

	while cursor > 0 && newline_count <= max_lines {
		let bytes_to_read = TAIL_READ_CHUNK_SIZE.min(cursor) as usize;
		cursor -= bytes_to_read as u64;

		file.seek(std::io::SeekFrom::Start(cursor)).await?;

		let mut chunk = vec![0u8; bytes_to_read];
		file.read_exact(&mut chunk).await?;
		newline_count += chunk.iter().filter(|byte| **byte == b'\n').count();

		if buffer.is_empty() {
			buffer = chunk;
		} else {
			chunk.extend_from_slice(&buffer);
			buffer = chunk;
		}
	}

	let raw = String::from_utf8_lossy(&buffer);
	let (content, trimmed_by_line_limit) = trim_to_last_lines(&raw, max_lines);

	Ok(ProcessLogTail {
		content,
		truncated: cursor > 0 || trimmed_by_line_limit,
	})
}

fn trim_to_last_lines(content: &str, max_lines: usize) -> (String, bool) {
	let lines: Vec<&str> = content.lines().collect();
	if lines.len() <= max_lines {
		return (content.to_string(), false);
	}

	let start_index = lines.len() - max_lines;
	(lines[start_index..].join("\n"), true)
}

#[cfg(test)]
mod tests {
	use std::path::PathBuf;
	use std::time::{SystemTime, UNIX_EPOCH};

	use super::read_process_log_tail;

	fn temp_file_path(name: &str) -> PathBuf {
		let nonce = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.expect("clock went backwards")
			.as_nanos();
		std::env::temp_dir().join(format!("onelauncher_{name}_{nonce}.log"))
	}

	#[tokio::test]
	async fn process_log_tail_returns_full_content_when_small() {
		let path = temp_file_path("small_tail");
		tokio::fs::write(&path, "a\nb\nc\n").await.unwrap();

		let tail = read_process_log_tail(&path, 10_000).await.unwrap();

		assert_eq!(tail.content, "a\nb\nc\n");
		assert!(!tail.truncated);

		let _ = tokio::fs::remove_file(path).await;
	}

	#[tokio::test]
	async fn process_log_tail_returns_only_last_requested_lines() {
		let path = temp_file_path("large_tail");
		let content = (0..20_000)
			.map(|n| format!("line-{n}"))
			.collect::<Vec<_>>()
			.join("\n");
		tokio::fs::write(&path, format!("{content}\n"))
			.await
			.unwrap();

		let tail = read_process_log_tail(&path, 10_000).await.unwrap();

		let lines: Vec<&str> = tail.content.lines().collect();
		assert_eq!(lines.len(), 10_000);
		assert_eq!(lines.first(), Some(&"line-10000"));
		assert_eq!(lines.last(), Some(&"line-19999"));
		assert!(tail.truncated);

		let _ = tokio::fs::remove_file(path).await;
	}

	#[tokio::test]
	async fn process_log_tail_handles_no_trailing_newline() {
		let path = temp_file_path("no_trailing_newline");
		let content = (0..5)
			.map(|n| format!("line-{n}"))
			.collect::<Vec<_>>()
			.join("\n");
		tokio::fs::write(&path, content).await.unwrap();

		let tail = read_process_log_tail(&path, 3).await.unwrap();

		let lines: Vec<&str> = tail.content.lines().collect();
		assert_eq!(lines, vec!["line-2", "line-3", "line-4"]);
		assert!(tail.truncated);

		let _ = tokio::fs::remove_file(path).await;
	}

	#[tokio::test]
	async fn process_log_tail_handles_crlf_input() {
		let path = temp_file_path("crlf_tail");
		tokio::fs::write(&path, "a\r\nb\r\nc\r\nd\r\n")
			.await
			.unwrap();

		let tail = read_process_log_tail(&path, 2).await.unwrap();

		let lines: Vec<&str> = tail.content.lines().collect();
		assert_eq!(lines, vec!["c", "d"]);
		assert!(tail.truncated);

		let _ = tokio::fs::remove_file(path).await;
	}
}
