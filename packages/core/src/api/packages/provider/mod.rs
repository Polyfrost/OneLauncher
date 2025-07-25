use onelauncher_entity::loader::GameLoader;
use onelauncher_entity::package::Provider;

use crate::api::packages::data::SearchResult;
use crate::error::LauncherResult;
use crate::utils::pagination::Paginated;

use super::data::{ManagedPackage, ManagedUser, ManagedVersion, PackageAuthor, SearchQuery};

mod modrinth;

pub use modrinth::ModrinthProviderImpl;

#[async_trait::async_trait]
pub trait ProviderExt {
	async fn search(&self, query: &SearchQuery) -> LauncherResult<Paginated<SearchResult>>;
	async fn get(&self, slug: &str) -> LauncherResult<ManagedPackage>;
	async fn get_multiple(&self, slugs: &[String]) -> LauncherResult<Vec<ManagedPackage>>;
	async fn get_versions_by_hashes(
		&self,
		hashes: &[String],
	) -> LauncherResult<Vec<ManagedVersion>>;
	async fn get_version_by_hash(&self, hash: &str) -> LauncherResult<ManagedVersion>;

	// async fn get_org_projects(&self, slug: &str) -> LauncherResult<Vec<ManagedPackage>>;

	async fn get_users_from_author(
		&self,
		author: PackageAuthor,
	) -> LauncherResult<Vec<ManagedUser>>;
	async fn get_users(&self, slugs: &[String]) -> LauncherResult<Vec<ManagedUser>>;
	async fn get_user(&self, slug: &str) -> LauncherResult<ManagedUser>;

	async fn get_versions_paginated(
		&self,
		slug: &str,
		mc_versions: Option<Vec<String>>,
		loaders: Option<Vec<GameLoader>>,
		offset: usize,
		limit: usize,
	) -> LauncherResult<Paginated<ManagedVersion>>;

	async fn get_versions(&self, slugs: &[String]) -> LauncherResult<Vec<ManagedVersion>>;
}

#[async_trait::async_trait]
impl ProviderExt for Provider {
	async fn search(&self, query: &SearchQuery) -> LauncherResult<Paginated<SearchResult>> {
		match self {
			Self::Modrinth => ModrinthProviderImpl.search(query).await,
			_ => todo!("unimplemented provider"),
		}
	}

	async fn get(&self, slug: &str) -> LauncherResult<ManagedPackage> {
		match self {
			Self::Modrinth => ModrinthProviderImpl.get(slug).await,
			_ => todo!("unimplemented provider"),
		}
	}

	async fn get_multiple(&self, slugs: &[String]) -> LauncherResult<Vec<ManagedPackage>> {
		match self {
			Self::Modrinth => ModrinthProviderImpl.get_multiple(slugs).await,
			_ => todo!("unimplemented provider"),
		}
	}

	async fn get_versions_by_hashes(
		&self,
		hashes: &[String],
	) -> LauncherResult<Vec<ManagedVersion>> {
		match self {
			Self::Modrinth => ModrinthProviderImpl.get_versions_by_hashes(hashes).await,
			_ => todo!("unimplemented provider"),
		}
	}

	async fn get_version_by_hash(&self, hash: &str) -> LauncherResult<ManagedVersion> {
		match self {
			Self::Modrinth => ModrinthProviderImpl.get_version_by_hash(hash).await,
			_ => todo!("unimplemented provider"),
		}
	}

	async fn get_users_from_author(
		&self,
		author: PackageAuthor,
	) -> LauncherResult<Vec<ManagedUser>> {
		match self {
			Self::Modrinth => ModrinthProviderImpl.get_users_from_author(author).await,
			_ => todo!("unimplemented provider"),
		}
	}

	async fn get_users(&self, slugs: &[String]) -> LauncherResult<Vec<ManagedUser>> {
		match self {
			Self::Modrinth => ModrinthProviderImpl.get_users(slugs).await,
			_ => todo!("unimplemented provider"),
		}
	}

	async fn get_user(&self, slug: &str) -> LauncherResult<ManagedUser> {
		match self {
			Self::Modrinth => ModrinthProviderImpl.get_user(slug).await,
			_ => todo!("unimplemented provider"),
		}
	}

	async fn get_versions_paginated(
		&self,
		slug: &str,
		mc_versions: Option<Vec<String>>,
		loaders: Option<Vec<GameLoader>>,
		offset: usize,
		limit: usize,
	) -> LauncherResult<Paginated<ManagedVersion>> {
		match self {
			Self::Modrinth => {
				ModrinthProviderImpl
					.get_versions_paginated(slug, mc_versions, loaders, offset, limit)
					.await
			}
			_ => todo!("unimplemented provider"),
		}
	}

	async fn get_versions(&self, slugs: &[String]) -> LauncherResult<Vec<ManagedVersion>> {
		match self {
			Self::Modrinth => ModrinthProviderImpl.get_versions(slugs).await,
			_ => todo!("unimplemented provider"),
		}
	}
}
