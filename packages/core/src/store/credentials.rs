//! Core credential management and storage with IOTA Stronghold.

use super::Directories;
use crate::utils::http::IoSemaphore;
use iota_stronghold::{KeyProvider, SnapshotPath};
use serde::{Serialize, Serializer};
use std::ops::Deref;
use std::path::Path;
use zeroize::Zeroizing;

/// A K/V encrypted store to handle digital secrets using IOTA Stronghold.
#[derive(Debug)]
pub struct Credentials {
	inner: iota_stronghold::Stronghold,
	path: iota_stronghold::SnapshotPath,
	keyprovider: iota_stronghold::KeyProvider,
}

impl Credentials {
	/// Initialize the K/V encrypted stronghold.
	pub fn new<P: AsRef<Path>>(
		path: P,
		password: Zeroizing<Vec<u8>>,
		_dirs: &Directories,
		_io_semaphore: &IoSemaphore,
	) -> crate::Result<Self> {
		let path = SnapshotPath::from_path(path);
		let stronghold = iota_stronghold::Stronghold::default();
		let keyprovider = KeyProvider::try_from(password).map_err(StrongholdError::MemoryError)?;
		if path.exists() {
			stronghold
				.load_snapshot(&keyprovider, &path)
				.map_err(StrongholdError::ClientError)?;
		}
		Ok(Self {
			inner: stronghold,
			path,
			keyprovider,
		})
	}

	pub fn save(&self) -> crate::Result<()> {
		self.inner
			.commit_with_keyprovider(&self.path, &self.keyprovider)
			.map_err(StrongholdError::ClientError)?;
		Ok(())
	}

	pub fn inner(&self) -> &iota_stronghold::Stronghold {
		&self.inner
	}
}

impl Deref for Credentials {
	type Target = iota_stronghold::Stronghold;
	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

#[derive(thiserror::Error, Debug)]
pub enum StrongholdError {
	#[error("stronghold not initialized")]
	StrongholdNotInitialized,
	#[error(transparent)]
	ClientError(#[from] iota_stronghold::ClientError),
	#[error(transparent)]
	MemoryError(#[from] iota_stronghold::MemoryError),
	#[error(transparent)]
	ProcedureError(#[from] iota_stronghold::procedures::ProcedureError),
}

impl Serialize for StrongholdError {
	fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		serializer.serialize_str(self.to_string().as_str())
	}
}
