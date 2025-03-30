use std::{collections::HashMap, sync::Arc};

use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::sync::{OnceCell, RwLock};

use crate::{data::{Loader, ManagedPackage, ManagedUser, ManagedVersion}, store::{ManagedVersionFile, ProviderSearchResults, SearchResult}, utils::{http, pagination::Pagination}, Result, State};

async fn fetch<T: DeserializeOwned>(url: &str) -> Result<T> {
	let state = State::get().await?;
	Ok(serde_json::from_slice(
		&http::fetch(
			format!("{}/{}", crate::constants::SKYCLIENT_BASE_URL, url).as_str(),
			None,
			&state.fetch_semaphore
		).await?
	)?)
}

static SKYCLIENTSTORE_STATIC: OnceCell<RwLock<SkyClientStore>> = OnceCell::const_new();

struct SkyClientStore {
	pub mods: Option<Vec<SkyClientMod>>
}

impl SkyClientStore {
	pub async fn get() -> Result<Arc<tokio::sync::RwLockReadGuard<'static, Self>>> {
		Ok(Arc::new(
			SKYCLIENTSTORE_STATIC
				.get_or_try_init(Self::initialize)
				.await?
				.read()
				.await,
		))
	}

	#[tracing::instrument]
	#[onelauncher_macros::memory]
	async fn initialize() -> Result<RwLock<Self>> {
		let mods: Vec<SkyClientMod> = fetch("/mods/mods.json").await?;

		Ok(RwLock::new(SkyClientStore {
			mods: Some(mods)
		}))
	}
}


#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct SkyClientMod {
    pub id: String,
    #[serde(default)]
    pub nicknames: Vec<String>,
    pub display: String,
    pub creator: String,
    pub discord_code: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    #[serde(default)]
    pub links: Vec<SkyClientModLink>,
    pub versions: Vec<SkyClientModVersion>,
    #[serde(default)]
    pub commands: Vec<String>,
    pub icon_scaling: Option<String>,
    pub oneconfig: Option<bool>,
}

impl Into<SearchResult> for SkyClientMod {
	fn into(self) -> SearchResult {
		SearchResult {
			slug: self.id.clone(),
			project_id: self.id,
			author: self.creator,
			categories: vec![],
			title: self.display,
			client_side: crate::store::PackageSide::Required,
			server_side: crate::store::PackageSide::Unknown,
			date_created: None,
			date_modified: None,
			description: self.description.unwrap_or_default(),
			project_type: crate::data::PackageType::Mod,
			downloads: 0,
			icon_url: self.icon.map(|i| format!("{}/icons/{}", crate::constants::SKYCLIENT_BASE_URL, i)).unwrap_or_default(),
			versions: self.versions.iter().map(|v| v.version.clone()).collect(),
			follows: 0,
		}
	}
}

impl Into<ManagedPackage> for SkyClientMod {
	fn into(self) -> ManagedPackage {
		ManagedPackage {
			id: self.id.clone(),
			main: self.id,
			title: self.display,
			body: crate::store::PackageBody::Markdown(self.description.clone().unwrap_or_default()),
			categories: vec![],
			client: crate::store::PackageSide::Required,
			server: crate::store::PackageSide::Unknown,
			created: None, // TODO: Get the oldest date from versions
			updated: None, // TODO: Get the newest date from versions
			description: self.description.unwrap_or_default(),
			downloads: 0,
			followers: 0,
			game_versions: self.versions.iter().flat_map(|v| v.game_versions.clone()).collect(),
			icon_url: self.icon.map(|i| format!("{}/icons/{}", crate::constants::SKYCLIENT_BASE_URL, i)),
			is_archived: false,
			license: None,
			loaders: self.versions.iter().flat_map(|v| v.loaders.clone().into_iter().map(Loader::from_string)).collect(),
			versions: self.versions.iter().map(|v| v.version.clone()).collect(),
			optional_categories: None,
			author: crate::store::Author::Users(vec![
				ManagedUser {
					id: self.creator.clone(),
					username: self.creator,
					is_organization_user: false,
					avatar_url: None,
					bio: None,
					role: None,
					url: self.discord_code.map(|code| format!("https://discord.gg/{}", code)),
				}
			]),
			package_type: crate::data::PackageType::Mod,
			provider: super::Providers::SkyClient,
		}
	}
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct SkyClientModLink {
    pub icon: String,
    pub text: String,
    pub link: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct SkyClientModVersion {
    pub version: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub file: String,
    pub url: String,
    pub hash: String,
	pub date_published: DateTime<Utc>,
    pub mod_id: Option<String>,
}

fn get_managed_version(m: &SkyClientMod, v: &SkyClientModVersion) -> ManagedVersion {
	let mut map = HashMap::new();
	map.insert(String::from("md5"), v.hash.clone());

	ManagedVersion {
		id: v.version.clone(),
		package_id: m.id.clone(),
		author: m.creator.clone(),
		changelog_url: None,
		changelog: String::new(),
		deps: vec![],
		downloads: 0,
		featured: false,
		files: vec![ManagedVersionFile {
			file_name: v.file.clone(),
			url: v.url.clone(),
			file_type: None,
			hashes: map,
			primary: true,
			size: 0,
		}],
		game_versions: v.game_versions.clone(),
		is_available: true,
		loaders: v.loaders.iter().map(|l| Loader::from_string(l.to_owned())).collect(),
		name: format!("{} v{}", m.display, v.version),
		published: Some(v.date_published),
		version_display: v.version.clone(),
		version_type: crate::store::ManagedVersionReleaseType::Release,
	}
}

pub async fn get(id: &str) -> Result<SkyClientMod> {
	let store = SkyClientStore::get().await?;

	let Some(mods) = &store.mods else {
		return Err(anyhow::anyhow!("mod not found").into())
	};

	for m in mods {
		if m.id == id {
			return Ok(m.clone());
		}
	}

	Err(anyhow::anyhow!("mod not found").into())
}

pub async fn get_multiple(slug_or_ids: &[String]) -> Result<Vec<SkyClientMod>> {
	let store = SkyClientStore::get().await?;

	let mods = match &store.mods {
		Some(mods) => mods,
		None => return Ok(vec![])
	};

	let mut results = vec![];

	for id in slug_or_ids {
		for m in mods {
			if m.id == *id {
				results.push(m.clone());
			}
		}
	}

	Ok(results)
}

pub async fn get_all_versions(
	project_id: &str,
	game_versions: Option<Vec<String>>,
	loaders: Option<Vec<Loader>>,
	page: Option<u32>,
	page_size: Option<u16>,
) -> Result<(Vec<ManagedVersion>, Pagination)> {
	let mut pagination = Pagination::default();
	let mut versions = vec![];

	if let Ok(m) = &get(project_id).await {
		for v in &m.versions {
			let mut can_add = true;

			if let Some(game_versions) = &game_versions {
				if !game_versions.is_empty() {
					can_add = v.game_versions.iter().any(|gv| game_versions.contains(gv))
				}
			}

			if can_add {
				if let Some(loaders) = &loaders {
					if !loaders.is_empty() {
						can_add = v.loaders.iter().any(|l| loaders.contains(&Loader::from_string(l.to_owned())))
					}
				}
			}

			if can_add {
				if page_size.map(|ps| versions.len() < ps as usize).unwrap_or(true) && page.map(|p| p <= pagination.index).unwrap_or(true) {
					versions.push(get_managed_version(m, v));
				}

				pagination.total_count += 1;
			}
		}
	}

	Ok((versions, pagination))
}

pub async fn get_versions(versions: Vec<String>) -> Result<Vec<ManagedVersion>> {
	let store = SkyClientStore::get().await?;

	let mods = match &store.mods {
		Some(mods) => mods,
		None => return Ok(vec![])
	};

	let mut results = vec![];

	for m in mods {
		for v in &m.versions {
			if versions.contains(&v.version) {
				results.push(get_managed_version(m, v));
			}
		}
	}

	Ok(results)
}

pub async fn get_versions_by_hashes(hashes: Vec<String>) -> Result<HashMap<String, ManagedVersion>> {
	let store = SkyClientStore::get().await?;

	let mods = match &store.mods {
		Some(mods) => mods,
		None => return Ok(HashMap::new())
	};

	let mut results = HashMap::new();

	for m in mods {
		for v in &m.versions {
			if hashes.contains(&v.hash) {
				results.insert(v.hash.clone(), get_managed_version(m, v));
			}
		}
	}

	Ok(results)
}

pub async fn search(
	query: Option<String>,
	limit: Option<u8>,
	offset: Option<u32>,
	game_versions: Option<Vec<String>>,
	// package_types: Option<Vec<PackageType>>,
	loaders: Option<Vec<Loader>>,
) -> Result<ProviderSearchResults> {
	let store = SkyClientStore::get().await?;

	let mut results = ProviderSearchResults {
		provider: super::Providers::SkyClient,
		results: vec![],
		total: 0,
	};

	let mods = match &store.mods {
		Some(mods) => mods,
		None => return Ok(results),
	};

	let query = query.map(|q| q.trim().to_lowercase());
	let limit = limit.map(|l| l as usize);
	let offset = offset.unwrap_or(0);

	for m in mods {
		let mut can_add = true;

		if let Some(query) = &query {
			if !query.is_empty() {
				let display = m.display.to_lowercase();

				can_add = display.contains(query) || m.id.contains(query) || m.nicknames.contains(query)
			}
		}

		if can_add {
			if let Some(game_versions) = &game_versions {
				if !game_versions.is_empty() {
					can_add = m.versions.iter().any(|v| v.game_versions.iter().any(|gv| game_versions.contains(&gv)))
				}
			}
		}

		if can_add {
			if let Some(loaders) = &loaders {
				if !loaders.is_empty() {
					can_add = m.versions.iter().any(|v| v.loaders.iter().any(|l| loaders.contains(&Loader::from_string(l.to_owned()))))
				}
			}
		}

		if can_add {
			if limit.map(|l| results.results.len() < l).unwrap_or(true) && offset <= results.total {
				let result: SearchResult = m.clone().into();
				results.results.push(result);
			}

			results.total += 1;
		}

	}

	Ok(results)
}