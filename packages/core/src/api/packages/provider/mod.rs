use std::collections::HashMap;

use onelauncher_entity::loader::GameLoader;
use onelauncher_entity::package::Provider;

use crate::api::packages::PackageError;
use crate::api::packages::data::{ManagedPackageBody, SearchResult};
use crate::error::LauncherResult;
use crate::utils::pagination::Paginated;

use super::data::{ManagedPackage, ManagedUser, ManagedVersion, PackageAuthor, SearchQuery};

#[cfg(test)]
pub(crate) mod tests;

mod curseforge;
mod modrinth;

pub use curseforge::CurseForgeProviderImpl;
pub use modrinth::ModrinthProviderImpl;

#[async_trait::async_trait]
pub trait ProviderExt {
	async fn search(&self, query: &SearchQuery) -> LauncherResult<Paginated<SearchResult>>;
	async fn get(&self, slug: &str) -> LauncherResult<ManagedPackage>;
	async fn get_multiple(&self, slugs: &[String]) -> LauncherResult<Vec<ManagedPackage>>;
	async fn get_versions_by_hashes(
		&self,
		hashes: &[String],
	) -> LauncherResult<HashMap<String, ManagedVersion>>;
	async fn get_version_by_hash(&self, hash: &str) -> LauncherResult<Option<ManagedVersion>>;

	// async fn get_org_projects(&self, slug: &str) -> LauncherResult<Vec<ManagedPackage>>;

	async fn get_users_from_author(
		&self,
		author: PackageAuthor,
	) -> LauncherResult<Vec<ManagedUser>>;
	// async fn get_users(&self, slugs: &[String]) -> LauncherResult<Vec<ManagedUser>>;
	// async fn get_user(&self, slug: &str) -> LauncherResult<ManagedUser>;

	async fn get_versions_paginated(
		&self,
		slug: &str,
		mc_version: Option<String>,
		loader: Option<GameLoader>,
		offset: usize,
		limit: usize,
	) -> LauncherResult<Paginated<ManagedVersion>>;

	async fn get_versions(&self, slugs: &[String]) -> LauncherResult<Vec<ManagedVersion>>;

	async fn get_body(&self, body: &ManagedPackageBody) -> LauncherResult<String> {
		match body {
			ManagedPackageBody::Raw(raw) => Ok(raw.clone()),
			_ => Err(PackageError::UnsupportedBodyType(body.clone()).into()),
		}
	}
}

/// Cleans up code duplication by delegating method calls to the appropriate provider implementation.
macro_rules! delegate {
    ($self:expr, $method:ident($($arg:expr),*)) => {
        match $self {
            Provider::Modrinth => ModrinthProviderImpl.$method($($arg),*).await,
            Provider::CurseForge => CurseForgeProviderImpl.$method($($arg),*).await,
        }
    };
}

/// Wrapper around all supported providers into a single enum.
#[async_trait::async_trait]
impl ProviderExt for Provider {
	async fn search(&self, query: &SearchQuery) -> LauncherResult<Paginated<SearchResult>> {
		delegate!(self, search(query))
	}

	async fn get(&self, slug: &str) -> LauncherResult<ManagedPackage> {
		delegate!(self, get(slug))
	}

	async fn get_multiple(&self, slugs: &[String]) -> LauncherResult<Vec<ManagedPackage>> {
		delegate!(self, get_multiple(slugs))
	}

	async fn get_versions_by_hashes(
		&self,
		hashes: &[String],
	) -> LauncherResult<HashMap<String, ManagedVersion>> {
		delegate!(self, get_versions_by_hashes(hashes))
	}

	async fn get_version_by_hash(&self, hash: &str) -> LauncherResult<Option<ManagedVersion>> {
		delegate!(self, get_version_by_hash(hash))
	}

	async fn get_users_from_author(
		&self,
		author: PackageAuthor,
	) -> LauncherResult<Vec<ManagedUser>> {
		delegate!(self, get_users_from_author(author))
	}

	// async fn get_users(&self, slugs: &[String]) -> LauncherResult<Vec<ManagedUser>> {
	// 	delegate!(self, get_users(slugs))
	// }

	// async fn get_user(&self, slug: &str) -> LauncherResult<ManagedUser> {
	// 	delegate!(self, get_user(slug))
	// }

	async fn get_versions_paginated(
		&self,
		slug: &str,
		mc_version: Option<String>,
		loader: Option<GameLoader>,
		offset: usize,
		limit: usize,
	) -> LauncherResult<Paginated<ManagedVersion>> {
		delegate!(
			self,
			get_versions_paginated(slug, mc_version, loader, offset, limit)
		)
	}

	async fn get_versions(&self, slugs: &[String]) -> LauncherResult<Vec<ManagedVersion>> {
		delegate!(self, get_versions(slugs))
	}

	async fn get_body(&self, body: &ManagedPackageBody) -> LauncherResult<String> {
		delegate!(self, get_body(body))
	}
}
