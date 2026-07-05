use std::fmt::Write;

use digest::Digest;
use tokio::io::AsyncReadExt;

use crate::LauncherResult;

pub async fn sha1_file(path: impl AsRef<std::path::Path>) -> LauncherResult<String> {
	let path = path.as_ref();
	let mut hasher = sha1::Sha1::new();
	let file = tokio::fs::File::open(path).await?;
	let mut reader = tokio::io::BufReader::new(file);
	let mut buffer = vec![0u8; 256 * 1024];

	loop {
		let n = reader.read(&mut buffer).await?;
		if n == 0 {
			break;
		}
		Digest::update(&mut hasher, &buffer[..n]);
	}

	Ok(to_hex(&hasher.finalize()))
}

pub fn sha1_bytes(data: &[u8]) -> String {
	let mut hasher = sha1::Sha1::new();
	Digest::update(&mut hasher, data);
	to_hex(&hasher.finalize())
}

pub fn normalize_hash(hash: &str) -> String {
	hash.trim().to_ascii_lowercase()
}

fn to_hex(data: &[u8]) -> String {
	data.iter().fold(String::new(), |mut out, b| {
		let _ = write!(out, "{b:02x}");
		out
	})
}
