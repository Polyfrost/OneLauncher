//! `OneLauncher` log management

use crate::data::{Credentials, Directories, MinecraftCredentials};
use crate::prelude::ClusterPath;
use crate::utils::http;
use futures::TryFutureExt;
use onelauncher_utils::io::{self, IOError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

// TODO: put this in the global store
/// Core logging state and reader for `OneLauncher`.
#[derive(Serialize, Debug)]
pub struct LogManager {
	/// Type log type associated with this log file.
	pub log_type: LogType,
	/// The age of this log as a [`u64`] in seconds.
	pub age: u64,
	/// The log file to read as a [`String`].
	pub log_file: String,
	/// The parsed and censored output of the logfile.
	pub output: Option<LogOutput>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, PartialEq)]
pub enum LogType {
	Info,
	Crash,
}

impl LogManager {
	/// Initialize a new [`LogManager`].
	async fn initialize(
		log_type: LogType,
		age: SystemTime,
		cluster: &ClusterPath,
		log_file: String,
		clear: Option<bool>,
	) -> crate::Result<Self> {
		Ok(Self {
			log_type,
			age: age
				.duration_since(SystemTime::UNIX_EPOCH)
				.unwrap_or_else(|_| std::time::Duration::from_secs(0))
				.as_secs(),
			output: if clear.unwrap_or(false) {
				None
			} else {
				Some(get_output_by_file(cluster, log_type, &log_file).await?)
			},
			log_file,
		})
	}
}

/// Verify all [`LogManager`]s of a certain [`LogType`]
#[tracing::instrument]
pub async fn get_logs_by_type(
	cluster_path: &ClusterPath,
	log_type: LogType,
	clear: Option<bool>,
	logs: &mut Vec<crate::Result<LogManager>>,
) -> crate::Result<()> {
	let logs_folder = match log_type {
		LogType::Info => Directories::cluster_logs_dir(cluster_path).await?,
		LogType::Crash => Directories::crash_reports_dir(cluster_path).await?,
	};

	if logs_folder.exists() {
		for entry in
			std::fs::read_dir(&logs_folder).map_err(|e| IOError::with_path(e, &logs_folder))?
		{
			let entry: std::fs::DirEntry =
				entry.map_err(|e| IOError::with_path(e, &logs_folder))?;
			let age = entry
				.metadata()?
				.created()
				.unwrap_or(SystemTime::UNIX_EPOCH);
			let path = entry.path();
			if !path.is_file() {
				continue;
			}
			if let Some(name) = path.file_name() {
				let name = name.to_string_lossy().to_string();

				logs.push(LogManager::initialize(log_type, age, cluster_path, name, clear).await);
			}
		}
	}

	Ok(())
}

/// Get all [`LogManager`]s in the global [`State`].
pub async fn get_logs(
	cluster_path: &ClusterPath,
	clear: Option<bool>,
) -> crate::Result<Vec<LogManager>> {
	let dir = Directories::cluster_logs_dir(cluster_path).await?;
	if !dir.exists() {
		io::create_dir(dir).await?;
		return Ok(vec![]);
	}

	let mut logs = Vec::new();
	get_logs_by_type(cluster_path, LogType::Info, clear, &mut logs).await?;
	get_logs_by_type(cluster_path, LogType::Crash, clear, &mut logs).await?;

	let mut logs = logs
		.into_iter()
		.collect::<crate::Result<Vec<LogManager>>>()?;
	logs.sort_by(|a, b| match (a.log_file.as_str(), b.log_file.as_str()) {
		("latest.log", _) => std::cmp::Ordering::Less,
		(_, "latest.log") => std::cmp::Ordering::Greater,
		_ => b.age.cmp(&a.age).then(b.log_file.cmp(&a.log_file)),
	});
	Ok(logs)
}

/// Delete all stored logs from a specific [`ClusterPath`].
#[tracing::instrument]
pub async fn delete_logs(cluster_path: ClusterPath) -> crate::Result<()> {
	let cluster_path = cluster_path.cluster_path().await?;
	let logs_folder = Directories::cluster_logs_dir(&cluster_path).await?;
	for entry in std::fs::read_dir(&logs_folder).map_err(|e| IOError::with_path(e, &logs_folder))? {
		let entry = entry.map_err(|e| IOError::with_path(e, &logs_folder))?;
		let path = entry.path();
		if path.is_dir() {
			io::remove_dir_all(&path).await?;
		}
	}

	Ok(())
}

/// Get the [`LogManager`] for a specific [`ClusterPath`] log file.
#[tracing::instrument]
pub async fn get_logs_by_file(
	cluster_path: ClusterPath,
	log_type: LogType,
	log_file: String,
) -> crate::Result<LogManager> {
	let cluster_path = cluster_path.cluster_path().await?;

	let path = match log_type {
		LogType::Info => Directories::cluster_logs_dir(&cluster_path).await,
		LogType::Crash => Directories::crash_reports_dir(&cluster_path).await,
	}?
	.join(&log_file);

	let metadata = std::fs::metadata(&path)?;
	let age = metadata.created().unwrap_or(SystemTime::UNIX_EPOCH);

	LogManager::initialize(log_type, age, &cluster_path, log_file, Some(true)).await
}

/// Get the default [`LogCursor`] for a [`ClusterPath`].
#[tracing::instrument]
pub async fn get_log_cursor(cluster_path: ClusterPath, cursor: u64) -> crate::Result<LogCursor> {
	get_live_log_cursor(cluster_path, "latest.log", cursor).await
}

/// Get a live [`LogCursor`] for a [`ClusterPath`]'s log file.
#[tracing::instrument]
pub async fn get_live_log_cursor(
	cluster_path: ClusterPath,
	log_file: &str,
	mut cursor: u64,
) -> crate::Result<LogCursor> {
	let cluster_path = cluster_path.cluster_path().await?;
	let logs_folder = Directories::cluster_logs_dir(&cluster_path).await?;
	let path = logs_folder.join(log_file);
	if !path.exists() {
		return Ok(LogCursor {
			cursor: 0,
			new: false,
			output: LogOutput(String::new()),
		});
	}

	let mut file = tokio::fs::File::open(&path)
		.await
		.map_err(|e| IOError::with_path(e, &path))?;
	let metadata = file
		.metadata()
		.await
		.map_err(|e| IOError::with_path(e, &path))?;

	let new = if cursor > metadata.len() {
		cursor = 0;
		true
	} else {
		false
	};

	let mut buf = Vec::new();
	file.seek(std::io::SeekFrom::Start(cursor))
		.map_err(|e| IOError::with_path(e, &path))
		.await?;
	let bytes_read = file
		.read_to_end(&mut buf)
		.map_err(|e| IOError::with_path(e, &path))
		.await?;
	let output = String::from_utf8_lossy(&buf).to_string();
	let cursor = cursor + bytes_read as u64;
	let creds: Vec<MinecraftCredentials> = crate::State::get()
		.await?
		.users
		.read()
		.await
		.users
		.clone()
		.into_values()
		.collect();
	let output = LogOutput::censor_secrets(output, &creds, &None);
	Ok(LogCursor {
		cursor,
		output,
		new,
	})
}

/// Delete a specific minecraft log file.
#[tracing::instrument]
pub async fn delete_logs_by_file(
	cluster_path: ClusterPath,
	log_type: LogType,
	log_file: &str,
) -> crate::Result<()> {
	let cluster_path = cluster_path.cluster_path().await?;
	let logs_folder = match log_type {
		LogType::Info => Directories::cluster_logs_dir(&cluster_path).await?,
		LogType::Crash => Directories::crash_reports_dir(&cluster_path).await?,
	};

	let path = logs_folder.join(log_file);
	io::remove_file(&path).await?;
	Ok(())
}

pub async fn read_log_to_string(path: &std::path::PathBuf) -> crate::Result<String> {
	if let Some(ext) = path.extension() {
		let mut result = String::new();

		if ext == "gz" {
			result = io::read_gz_to_string(path).await?;
		} else if ext == "log" || ext == "txt" {
			// On Java 17 and older, UTF-8 is not the default charset (https://openjdk.org/jeps/400)
			// Minecraft on Windows on older versions sometimes likes to output the log
			// in an encoding other than UTF-8, which would make OneLauncher throw an error
			// when attempting to read it into a String (Rust Strings are UTF-8)
			// TODO: Potentially explore a better solution for non UTF-8 log reading?
			let bytes = io::read(path).await?;
			result = String::from_utf8_lossy(&bytes).to_string();
		}

		return Ok(result);
	}

	Err(anyhow::anyhow!("log file extension {} not supported", path.display()).into())
}

/// Get a specific [`ClusterPath`] log file's [`LogOutput`].
#[tracing::instrument]
pub async fn get_output_by_file(
	cluster_path: &ClusterPath,
	log_type: LogType,
	log_file: &str,
) -> crate::Result<LogOutput> {
	let logs_folder = match log_type {
		LogType::Info => Directories::cluster_logs_dir(cluster_path).await?,
		LogType::Crash => Directories::crash_reports_dir(cluster_path).await?,
	};
	let path = logs_folder.join(log_file);
	let credentials: Vec<MinecraftCredentials> = crate::State::get()
		.await?
		.users
		.read()
		.await
		.users
		.clone()
		.into_values()
		.collect();

	let result = read_log_to_string(&path).await?;

	return Ok(LogOutput::censor_secrets(result, &credentials, &None));
}

/// The log cursor used to parse logs passed into [`LogManager`].
#[derive(Serialize, Debug)]
pub struct LogCursor {
	/// The cursor ID.
	pub cursor: u64,
	/// The [`LogOutput`] associated with this cursor.
	pub output: LogOutput,
	/// Whether or not this corresponds is a new log file.
	pub new: bool,
}

/// The log output, a wrapper around [`String`], with utilities for parsing and censoring log contents.
#[derive(Serialize, Debug)]
#[serde(transparent)]
pub struct LogOutput(pub String);

impl LogOutput {
	/// Censor user secrets because sometimes mclogs misses them, including username, realname and minecraft credentials.
	pub fn censor_secrets(
		mut output: String,
		credentials: &Vec<MinecraftCredentials>,
		_credentials_store: &Option<Credentials>,
	) -> Self {
		let username = whoami::username();
		let realname = whoami::realname();
		output = output.replace(&format!("/{username}/"), "/{ENV_USERNAME}/");
		output = output.replace(&format!("\\{username}\\"), "\\{ENV_USERNAME}\\");
		output = output.replace(&format!("/{realname}/"), "/{ENV_REALNAME}/");
		output = output.replace(&format!("\\{realname}\\"), "\\{ENV_REALNAME}\\");

		for cred in credentials {
			output = output.replace(&cred.access_token, "{MC_ACCESS_TOKEN}");
			output = output.replace(&cred.username, "{MC_USERNAME}");
			output = output.replace(&cred.id.as_simple().to_string(), "{MC_UUID}");
			output = output.replace(&cred.id.as_hyphenated().to_string(), "{MC_UUID}");
		}

		Self(output)
	}
}

#[derive(serde::Deserialize)]
struct McLogsResponse {
	pub success: bool,
	#[serde(default)]
	pub id: Option<String>,
	#[serde(default)]
	pub error: Option<String>,
}

/// Upload a log to https://mclo.gs/. If successful, it returns the log id
#[tracing::instrument]
pub async fn upload_log(path: &ClusterPath, log: LogOutput) -> crate::Result<String> {
	let log = log.0;
	let mut form = HashMap::new();
	form.insert("content", log);

	let parsed = serde_json::from_slice::<McLogsResponse>(
		&http::REQWEST_CLIENT
			.post(format!("{}/log", crate::constants::MCLOGS_API_URL))
			.header("Content-Type", "application/x-www-form-urlencoded")
			.form(&form)
			.send()
			.await?
			.bytes()
			.await?,
	)?;

	if parsed.success {
		parsed.id.map_or_else(
			|| Err(anyhow::anyhow!("failed to get log id from mclo.gs").into()),
			Ok,
		)
	} else {
		Err(anyhow::anyhow!(
			"failed to upload log to mclo.gs: {}",
			parsed.error.unwrap_or_default()
		)
		.into())
	}
}
