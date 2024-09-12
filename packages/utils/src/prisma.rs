use onelauncher_prisma::prisma::PrismaClient;
use prisma_client_rust::migrations::{DbPushError, MigrateDeployError};
use prisma_client_rust::NewClientError;

/// Represents an error that occured while opening and running migrations
/// on the [`prisma_client_rust`] database.
#[derive(thiserror::Error, Debug)]
pub enum MigrationError {
	#[error("failed to initialize a new database connection: {0}")]
	NewClient(#[from] Box<NewClientError>),
	#[error("failed to migrate the database: {0}")]
	MigrateFailed(#[from] MigrateDeployError),
	#[cfg(debug_assertions)]
	#[error("failed to migrate the database: {0}")]
	DbPushFailed(#[from] DbPushError),
}

/// Loads the database from the given path and migrates it to the latest schema.
pub async fn load_and_migrate(db_url: &str) -> Result<PrismaClient, MigrationError> {
	let client = PrismaClient::_builder()
		.with_url(db_url.to_string())
		.build()
		.await
		.map_err(Box::new)?;
	client._migrate_deploy().await?;

	#[cfg(debug_assertions)]
	{
		let mut builder = client._db_push();
		if std::env::var("ONELAUNCHER_DO_NOT_USE_THIS")
			.map(|v| v == "true")
			.unwrap_or(false)
		{
			builder = builder.accept_data_loss();
		}

		if std::env::var("ONELAUNCHER_FORCE_RESET")
			.map(|v| v == "true")
			.unwrap_or(false)
		{
			builder = builder.force_reset();
		}

		let res = builder.await;

		match res {
			Ok(_) => {}
			Err(e @ DbPushError::PossibleDataLoss(_)) => {
				eprintln!("pushing prisma schema may result in data loss. use `ONELAUNCHER_DO_NOT_USE_THIS=true` to force it.");
				Err(e)?;
			}
			Err(e) => Err(e)?,
		}
	}

	Ok(client)
}

/// Construct back an inode after storing it in database
#[must_use]
pub const fn inode_from_db(db_inode: &[u8]) -> u64 {
	u64::from_le_bytes([
		db_inode[0],
		db_inode[1],
		db_inode[2],
		db_inode[3],
		db_inode[4],
		db_inode[5],
		db_inode[6],
		db_inode[7],
	])
}

#[must_use]
pub fn inode_to_db(inode: u64) -> Vec<u8> {
	inode.to_le_bytes().to_vec()
}

#[must_use]
pub const fn size_in_bytes_from_db(db_size_in_bytes: &[u8]) -> u64 {
	u64::from_be_bytes([
		db_size_in_bytes[0],
		db_size_in_bytes[1],
		db_size_in_bytes[2],
		db_size_in_bytes[3],
		db_size_in_bytes[4],
		db_size_in_bytes[5],
		db_size_in_bytes[6],
		db_size_in_bytes[7],
	])
}

#[must_use]
pub fn size_in_bytes_to_db(size: u64) -> Vec<u8> {
	size.to_be_bytes().to_vec()
}

#[derive(thiserror::Error, Debug)]
#[error("missing database field {0}")]
pub struct MissingFieldError(&'static str);

impl MissingFieldError {
	#[must_use]
	pub const fn new(value: &'static str) -> Self {
		Self(value)
	}
}

pub trait OptionalField: Sized {
	type Out;

	fn transform(self) -> Option<Self::Out>;
}

impl<T> OptionalField for Option<T> {
	type Out = T;

	fn transform(self) -> Self {
		self
	}
}

impl<'a, T> OptionalField for &'a Option<T> {
	type Out = &'a T;

	fn transform(self) -> Option<Self::Out> {
		self.as_ref()
	}
}

pub fn maybe_missing<T: OptionalField>(
	data: T,
	field: &'static str,
) -> Result<T::Out, MissingFieldError> {
	data.transform().ok_or(MissingFieldError(field))
}
