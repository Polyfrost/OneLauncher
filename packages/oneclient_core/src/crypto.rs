use std::fmt::Write;
use std::io::Read;

use digest::Digest;

use crate::LauncherResult;

/// Largest read chunk used when hashing a file. Files smaller than this get a
/// buffer sized to fit, so hashing a 10 KiB asset doesn't allocate (and zero)
/// a quarter of a megabyte.
const MAX_HASH_BUFFER: usize = 256 * 1024;

/// Incremental SHA-1, so a download can be verified from the bytes as they
/// stream past instead of re-reading the finished file back off disk.
pub struct Sha1Stream(sha1::Sha1);

impl Sha1Stream {
	#[must_use]
	pub fn new() -> Self {
		Self(sha1::Sha1::new())
	}

	pub fn update(&mut self, data: &[u8]) {
		Digest::update(&mut self.0, data);
	}

	#[must_use]
	pub fn finish(self) -> String {
		to_hex(&self.0.finalize())
	}
}

impl Default for Sha1Stream {
	fn default() -> Self {
		Self::new()
	}
}

/// Hashing is CPU- and syscall-bound, so it runs on the blocking pool: one
/// dispatch for the whole file rather than one per read, and the digest itself
/// stays off the async workers that are driving concurrent downloads.
pub async fn sha1_file(path: impl AsRef<std::path::Path>) -> LauncherResult<String> {
	let path = path.as_ref().to_path_buf();
	tokio::task::spawn_blocking(move || sha1_file_sync(&path))
		.await
		.map_err(std::io::Error::other)?
}

pub fn sha1_file_sync(path: &std::path::Path) -> LauncherResult<String> {
	let mut file = std::fs::File::open(path)?;
	let mut hasher = Sha1Stream::new();

	let size = file
		.metadata()
		.map(|meta| meta.len())
		.unwrap_or(MAX_HASH_BUFFER as u64);
	let capacity = (size.max(1).min(MAX_HASH_BUFFER as u64)) as usize;
	let mut buffer = vec![0u8; capacity];

	loop {
		let n = file.read(&mut buffer)?;
		if n == 0 {
			break;
		}
		hasher.update(&buffer[..n]);
	}

	Ok(hasher.finish())
}

pub fn sha1_bytes(data: &[u8]) -> String {
	let mut hasher = Sha1Stream::new();
	hasher.update(data);
	hasher.finish()
}

pub fn normalize_hash(hash: &str) -> String {
	hash.trim().to_ascii_lowercase()
}

pub(crate) fn to_hex(data: &[u8]) -> String {
	data.iter().fold(String::new(), |mut out, b| {
		let _ = write!(out, "{b:02x}");
		out
	})
}
