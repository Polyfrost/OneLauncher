use std::fmt::Write;
use std::io::Read;
use std::path::Path;

use digest::Digest;

use crate::{IOError, PolyIOResult};

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
pub async fn sha1_file(path: impl AsRef<Path>) -> PolyIOResult<String> {
	let path = path.as_ref().to_path_buf();
	tokio::task::spawn_blocking(move || sha1_file_sync(&path))
		.await
		.map_err(std::io::Error::other)?
}

pub fn sha1_file_sync(path: &Path) -> PolyIOResult<String> {
	let mut file = std::fs::File::open(path).map_err(|e| IOError::PathIOError {
		source: e,
		path: path.to_string_lossy().to_string(),
	})?;
	let mut hasher = Sha1Stream::new();

	let size = file
		.metadata()
		.map(|meta| meta.len())
		.unwrap_or(MAX_HASH_BUFFER as u64);
	let capacity = (size.max(1).min(MAX_HASH_BUFFER as u64)) as usize;
	let mut buffer = vec![0u8; capacity];

	loop {
		let n = file.read(&mut buffer).map_err(|e| IOError::PathIOError {
			source: e,
			path: path.to_string_lossy().to_string(),
		})?;
		if n == 0 {
			break;
		}
		hasher.update(&buffer[..n]);
	}

	Ok(hasher.finish())
}

#[must_use]
pub fn sha1_bytes(data: &[u8]) -> String {
	let mut hasher = Sha1Stream::new();
	hasher.update(data);
	hasher.finish()
}

/// Canonical form for a hash read from an external manifest, so a comparison
/// against a locally computed one doesn't fail on case or stray whitespace.
#[must_use]
pub fn normalize_hash(hash: &str) -> String {
	hash.trim().to_ascii_lowercase()
}

#[must_use]
pub fn to_hex(data: &[u8]) -> String {
	data.iter().fold(String::new(), |mut out, b| {
		let _ = write!(out, "{b:02x}");
		out
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn sha1_bytes_matches_known_vector() {
		assert_eq!(
			sha1_bytes(b"abc"),
			"a9993e364706816aba3e25717850c26c9cd0d89d"
		);
		assert_eq!(
			sha1_bytes(b""),
			"da39a3ee5e6b4b0d3255bfef95601890afd80709"
		);
	}

	#[test]
	fn stream_and_oneshot_agree() {
		let mut stream = Sha1Stream::new();
		stream.update(b"ab");
		stream.update(b"c");
		assert_eq!(stream.finish(), sha1_bytes(b"abc"));
	}

	#[test]
	fn normalize_hash_trims_and_lowercases() {
		assert_eq!(normalize_hash("  A9B2  "), "a9b2");
	}
}
