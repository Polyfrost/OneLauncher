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

/// Which digest a manifest published its hash in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChecksumAlgorithm {
	Sha1,
	Sha256,
}

impl ChecksumAlgorithm {
	/// Length of this digest written as hex, used to tell one apart from
	/// another when a manifest gives a bare string with no algorithm field.
	#[must_use]
	pub const fn hex_len(self) -> usize {
		match self {
			Self::Sha1 => 40,
			Self::Sha256 => 64,
		}
	}

	#[must_use]
	pub const fn name(self) -> &'static str {
		match self {
			Self::Sha1 => "SHA-1",
			Self::Sha256 => "SHA-256",
		}
	}
}

/// A hash a manifest promised, together with the algorithm it is written in.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Checksum {
	pub algorithm: ChecksumAlgorithm,
	/// Always normalized, so comparing two `Checksum`s never trips on case.
	pub hex: String,
}

impl Checksum {
	#[must_use]
	pub fn new(algorithm: ChecksumAlgorithm, hex: impl AsRef<str>) -> Self {
		Self {
			algorithm,
			hex: normalize_hash(hex.as_ref()),
		}
	}

	#[must_use]
	pub fn sha1(hex: impl AsRef<str>) -> Self {
		Self::new(ChecksumAlgorithm::Sha1, hex)
	}

	#[must_use]
	pub fn sha256(hex: impl AsRef<str>) -> Self {
		Self::new(ChecksumAlgorithm::Sha256, hex)
	}

	/// Whether the hash is the right shape for its algorithm.
	///
	/// A vendor that starts returning an empty string, a placeholder, or a
	/// digest in another algorithm would otherwise fail every download with a
	/// mismatch that looks like corruption. Callers drop malformed checksums and
	/// download unverified rather than making the runtime uninstallable.
	#[must_use]
	pub fn is_well_formed(&self) -> bool {
		self.hex.len() == self.algorithm.hex_len()
			&& self.hex.bytes().all(|b| b.is_ascii_hexdigit())
	}

	#[must_use]
	pub fn matches(&self, actual: &str) -> bool {
		self.hex == normalize_hash(actual)
	}
}

/// Incremental hashing in whichever algorithm a [`Checksum`] calls for, so a
/// download is verified from the bytes as they stream past rather than by
/// re-reading the finished file.
pub enum ChecksumStream {
	Sha1(sha1::Sha1),
	Sha256(sha2::Sha256),
}

impl ChecksumStream {
	#[must_use]
	pub fn new(algorithm: ChecksumAlgorithm) -> Self {
		match algorithm {
			ChecksumAlgorithm::Sha1 => Self::Sha1(sha1::Sha1::new()),
			ChecksumAlgorithm::Sha256 => Self::Sha256(sha2::Sha256::new()),
		}
	}

	pub fn update(&mut self, data: &[u8]) {
		match self {
			Self::Sha1(hasher) => Digest::update(hasher, data),
			Self::Sha256(hasher) => Digest::update(hasher, data),
		}
	}

	#[must_use]
	pub fn finish(self) -> String {
		match self {
			Self::Sha1(hasher) => to_hex(&hasher.finalize()),
			Self::Sha256(hasher) => to_hex(&hasher.finalize()),
		}
	}
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

	#[test]
	fn checksum_streams_match_their_one_shot_digests() {
		let mut sha1 = ChecksumStream::new(ChecksumAlgorithm::Sha1);
		sha1.update(b"ab");
		sha1.update(b"c");
		assert_eq!(sha1.finish(), "a9993e364706816aba3e25717850c26c9cd0d89d");

		let mut sha256 = ChecksumStream::new(ChecksumAlgorithm::Sha256);
		sha256.update(b"ab");
		sha256.update(b"c");
		assert_eq!(
			sha256.finish(),
			"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
		);
	}

	#[test]
	fn checksum_comparison_ignores_case_and_padding() {
		let expected = Checksum::sha1("  A9993E364706816ABA3E25717850C26C9CD0D89D ");
		assert!(expected.matches("a9993e364706816aba3e25717850c26c9cd0d89d"));
		assert!(expected.matches("A9993E364706816ABA3E25717850C26C9CD0D89D"));
		assert!(!expected.matches("da39a3ee5e6b4b0d3255bfef95601890afd80709"));
	}

	#[test]
	fn a_hash_of_the_wrong_shape_is_not_well_formed() {
		// What a vendor sending a placeholder, an empty field, or a digest in
		// another algorithm would look like. Verifying against these would fail
		// every download with what reads to the user as corruption.
		assert!(!Checksum::sha256("").is_well_formed());
		assert!(!Checksum::sha256("n/a").is_well_formed());
		assert!(!Checksum::sha256("a9993e364706816aba3e25717850c26c9cd0d89d").is_well_formed());
		assert!(!Checksum::sha1("zzz3e364706816aba3e25717850c26c9cd0d89d!").is_well_formed());

		assert!(Checksum::sha1("a9993e364706816aba3e25717850c26c9cd0d89d").is_well_formed());
		assert!(
			Checksum::sha256(
				"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
			)
			.is_well_formed()
		);
	}
}
