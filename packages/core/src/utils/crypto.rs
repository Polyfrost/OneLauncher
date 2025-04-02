use std::{fmt::Display, str::FromStr};

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
	#[error("expected {algorithm} hash {expected}, but got {actual}")]
	InvalidHash {
		algorithm: HashAlgorithm,
		expected: String,
		actual: String,
	},
	#[error("invalid hash algorithm")]
	InvalidAlgorithm
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HashAlgorithm {
	Sha1,
}

impl HashAlgorithm {
	#[must_use]
	pub fn hash(&self, data: &[u8]) -> String {
		match self {
			Self::Sha1 => sha1_smol::Sha1::from(data).digest().to_string(),
		}
	}
}

impl Display for HashAlgorithm {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Sha1 => write!(f, "sha1"),
		}
	}
}

impl FromStr for HashAlgorithm {
	type Err = CryptoError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s.to_lowercase().as_str() {
			"sha1" => Ok(Self::Sha1),
			_ => Err(CryptoError::InvalidAlgorithm),
		}
	}
}