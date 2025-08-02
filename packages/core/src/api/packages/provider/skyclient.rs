use std::collections::HashMap;

use onelauncher_entity::loader::GameLoader;

use crate::api::packages::data::{
	ManagedPackage, ManagedUser, ManagedVersion, PackageAuthor, SearchQuery, SearchResult,
};
use crate::api::packages::provider::ProviderExt;
use crate::error::LauncherResult;
use crate::utils::pagination::Paginated;

#[derive(Default)]
pub struct SkyClientProviderImpl;

#[async_trait::async_trait]
impl ProviderExt for SkyClientProviderImpl {
	async fn search(&self, query: &SearchQuery) -> LauncherResult<Paginated<SearchResult>> {
		todo!("SkyClient search not implemented yet")
	}

	async fn get(&self, slug: &str) -> LauncherResult<ManagedPackage> {
		todo!("SkyClient get package not implemented yet")
	}

	async fn get_multiple(&self, slugs: &[String]) -> LauncherResult<Vec<ManagedPackage>> {
		todo!("SkyClient get multiple packages not implemented yet")
	}

	async fn get_versions_by_hashes(
		&self,
		hashes: &[String],
	) -> LauncherResult<HashMap<String, ManagedVersion>> {
		todo!("SkyClient get versions by hashes not implemented yet")
	}

	async fn get_version_by_hash(&self, hash: &str) -> LauncherResult<Option<ManagedVersion>> {
		todo!("SkyClient get version by hash not implemented yet")
	}

	async fn get_users_from_author(
		&self,
		author: PackageAuthor,
	) -> LauncherResult<Vec<ManagedUser>> {
		todo!("SkyClient get users from author not implemented yet")
	}

	// async fn get_users(&self, slugs: &[String]) -> LauncherResult<Vec<ManagedUser>> {
	// 	todo!("SkyClient get users not implemented yet")
	// }

	// async fn get_user(&self, slug: &str) -> LauncherResult<ManagedUser> {
	// 	todo!("SkyClient get user not implemented yet")
	// }

	async fn get_versions_paginated(
		&self,
		slug: &str,
		mc_version: Option<String>,
		loader: Option<GameLoader>,
		offset: usize,
		limit: usize,
	) -> LauncherResult<Paginated<ManagedVersion>> {
		todo!("SkyClient get versions paginated not implemented yet")
	}

	async fn get_versions(&self, slugs: &[String]) -> LauncherResult<Vec<ManagedVersion>> {
		todo!("SkyClient get versions not implemented yet")
	}
}
