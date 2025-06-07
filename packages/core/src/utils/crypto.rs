use std::fmt::{Display, Write};
use std::io::Read;
use std::str::FromStr;

use digest::DynDigest;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;

use super::io::IOError;

#[onelauncher_macro::specta]
#[derive(Debug, thiserror::Error, Serialize)]
pub enum CryptoError {
	#[error("expected {algorithm} hash {expected}, but got {actual}")]
	InvalidHash {
		algorithm: HashAlgorithm,
		expected: String,
		actual: String,
	},
	#[error("invalid hash algorithm")]
	InvalidAlgorithm,
}

#[onelauncher_macro::specta]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HashAlgorithm {
	Sha1,
	Sha256,
	Md5,
}

const BUFFER_SIZE: usize = 1024 * 256; // 16 KiB

impl HashAlgorithm {
	pub async fn hash(&self, data: &[u8]) -> Result<String, IOError> {
		let mut hasher = get_inner(self);
		hasher.update(data);
		Ok(to_hex(&hasher.finalize()))
	}

	pub fn hash_sync(&self, data: &[u8]) -> Result<String, IOError> {
		let mut hasher = get_inner(self);
		hasher.update(data);
		Ok(to_hex(&hasher.finalize()))
	}

	pub async fn hash_file(&self, path: impl AsRef<std::path::Path>) -> Result<String, IOError> {
		let path = path.as_ref();

		let mut hasher = get_inner(self);

		let file = tokio::fs::File::open(path).await?;
		let mut reader = tokio::io::BufReader::new(file);

		let mut buffer = vec![0; BUFFER_SIZE].into_boxed_slice();
		loop {
			let bytes_read = reader.read(&mut buffer).await?;
			if bytes_read == 0 {
				break;
			}

			hasher.update(&buffer[..bytes_read]);
		}

		Ok(to_hex(&hasher.finalize()))
	}

	pub fn hash_file_sync(&self, path: impl AsRef<std::path::Path>) -> Result<String, IOError> {
		let path = path.as_ref();
		let mut hasher = get_inner(self);
		let mut file = std::fs::File::open(path)?;

		let mut buffer = vec![0; BUFFER_SIZE].into_boxed_slice();
		loop {
			let bytes_read = file.read(&mut buffer)?;
			if bytes_read == 0 {
				break;
			}

			hasher.update(&buffer[..bytes_read]);
		}
		Ok(to_hex(&hasher.finalize()))
	}
}

impl Display for HashAlgorithm {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Sha1 => write!(f, "sha1"),
			Self::Sha256 => write!(f, "sha256"),
			Self::Md5 => write!(f, "md5"),
		}
	}
}

impl FromStr for HashAlgorithm {
	type Err = CryptoError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s.to_lowercase().as_str() {
			"sha1" | "sha-1" => Ok(Self::Sha1),
			"sha256" | "sha-256" => Ok(Self::Sha256),
			"md5" | "md-5" => Ok(Self::Md5),
			_ => Err(CryptoError::InvalidAlgorithm),
		}
	}
}

fn get_inner(algorithm: &HashAlgorithm) -> Box<dyn DynDigest + Send + Sync> {
	match algorithm {
		HashAlgorithm::Sha1 => Box::new(sha1::Sha1::default()),
		HashAlgorithm::Sha256 => Box::new(sha2::Sha256::default()),
		HashAlgorithm::Md5 => Box::new(md5::Md5::default())
	}
}

fn to_hex(data: &[u8]) -> String {
	data.iter().fold(String::new(), |mut output, b| {
		let _ = write!(output, "{b:02x}");
		output
	})
}