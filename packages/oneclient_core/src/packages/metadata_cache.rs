
use std::collections::HashMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::dao::package_metadata as meta_dao;

use crate::packages::ProviderId;
use crate::packages::domain::{ContentType, GameLoader};
use crate::packages::store::artifact_absolute_path;
use crate::packages::types::{PackageBody, ProjectDetail, VersionDetail, VersionFile};
use crate::{LauncherResult, LauncherServices};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedPackageMeta {
	pub provider: ProviderId,
	pub project_id: String,
	pub name: String,
	pub summary: String,
	pub author: String,
	pub icon_url: Option<String>,
}

#[tracing::instrument(level = "debug", skip(services, project_ids))]
pub async fn read_cached_package_meta(
	services: &LauncherServices,
	provider: ProviderId,
	project_ids: &[String],
) -> HashMap<String, CachedPackageMeta> {
	if project_ids.is_empty() {
		return HashMap::new();
	}

	let rows = meta_dao::get_package_metadata_batch(&services.db, provider as i64, project_ids)
		.await
		.unwrap_or_default();

	rows.into_iter()
		.map(|row| {
			(
				row.project_id.clone(),
				CachedPackageMeta {
					provider,
					project_id: row.project_id,
					name: row.name,
					summary: row.summary,
					author: row.author,
					icon_url: row.icon_url,
				},
			)
		})
		.collect()
}

#[tracing::instrument(level = "debug", skip(services))]
pub async fn cached_project_detail(
	services: &LauncherServices,
	provider: ProviderId,
	project_id: &str,
	content_type: ContentType,
) -> ProjectDetail {
	let cached = read_cached_package_meta(services, provider, std::slice::from_ref(&project_id.to_string()))
		.await;
	let meta = cached.get(project_id);

	ProjectDetail {
		id: project_id.to_string(),
		slug: String::new(),
		provider,
		content_type,
		name: meta
			.map(|m| m.name.clone())
			.filter(|n| !n.is_empty())
			.unwrap_or_else(|| project_id.to_string()),
		summary: meta.map(|m| m.summary.clone()).unwrap_or_default(),
		author: meta.map(|m| m.author.clone()).unwrap_or_default(),
		members: Vec::new(),
		gallery: Vec::new(),
		body: PackageBody::Raw(String::new()),
		license: None,
		links: Vec::new(),
		version_ids: Vec::new(),
		game_versions: Vec::new(),
		loaders: Vec::new(),
		icon_url: meta.and_then(|m| m.icon_url.clone()),
		created: Utc::now(),
		updated: Utc::now(),
		downloads: 0,
	}
}

#[tracing::instrument(level = "debug", skip(services))]
pub async fn get_version_cached(
	services: &LauncherServices,
	provider: ProviderId,
	project_id: &str,
	version_id: &str,
) -> LauncherResult<VersionDetail> {
	if let Some(cached) = cached_version_detail(services, provider, project_id, version_id).await {
		return Ok(cached);
	}

	services
		.packages
		.get(provider)?
		.get_version(project_id, version_id, services)
		.await
}

#[tracing::instrument(level = "debug", skip(services))]
async fn cached_version_detail(
	services: &LauncherServices,
	provider: ProviderId,
	project_id: &str,
	version_id: &str,
) -> Option<VersionDetail> {
	let release =
		artifact_dao::get_provider_release(&services.db, provider as i64, project_id, version_id)
			.await
			.ok()
			.flatten()?;
	let artifact = artifact_dao::get_artifact_by_hash(&services.db, &release.hash)
		.await
		.ok()
		.flatten()?;

	let path = artifact_absolute_path(&artifact.path).ok()?;
	if !path.exists() {
		return None;
	}

	let game_versions: Vec<String> = serde_json::from_str(&release.mc_versions).unwrap_or_default();
	let loaders: Vec<GameLoader> = serde_json::from_str(&release.mc_loaders).unwrap_or_default();
	let published = release
		.published_at
		.as_deref()
		.and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
		.map(|dt| dt.with_timezone(&Utc))
		.unwrap_or_else(Utc::now);

	Some(VersionDetail {
		version_id: version_id.to_string(),
		project_id: project_id.to_string(),
		name: release.display_name,
		version_number: release.display_version,
		changelog: None,
		game_versions,
		loaders,
		published,
		downloads: 0,
		files: vec![VersionFile {
			sha1: release.hash,
			url: String::new(),
			file_name: artifact.file_name,
			primary: true,
			size: artifact.size_bytes.unwrap_or(0) as u64,
			fingerprint: None,
		}],
	})
}

#[tracing::instrument(level = "debug", skip(services, project_ids))]
pub async fn fetch_package_meta(
	services: &LauncherServices,
	provider: ProviderId,
	project_ids: &[String],
) -> LauncherResult<HashMap<String, CachedPackageMeta>> {
	if project_ids.is_empty() {
		return Ok(HashMap::new());
	}

	let provider_repr = provider as i64;
	let mut out: HashMap<String, CachedPackageMeta> = HashMap::new();

	for row in meta_dao::get_package_metadata_batch(&services.db, provider_repr, project_ids).await?
	{
		out.insert(
			row.project_id.clone(),
			CachedPackageMeta {
				provider,
				project_id: row.project_id,
				name: row.name,
				summary: row.summary,
				author: row.author,
				icon_url: row.icon_url,
			},
		);
	}

	let missing: Vec<String> = project_ids
		.iter()
		.filter(|id| out.get(*id).is_none_or(|m| m.author.is_empty()))
		.cloned()
		.collect();
	if missing.is_empty() {
		return Ok(out);
	}

	tracing::debug!(count = missing.len(), "fetching missing package metadata from provider");

	let details = services
		.packages
		.get(provider)?
		.get_projects(&missing, services)
		.await?;

	for detail in details {
		let meta = CachedPackageMeta {
			provider,
			project_id: detail.id,
			name: detail.name,
			summary: detail.summary,
			author: detail.author,
			icon_url: detail.icon_url,
		};
		meta_dao::upsert_package_metadata(
			&services.db,
			provider_repr,
			&meta.project_id,
			&meta.name,
			&meta.summary,
			&meta.author,
			meta.icon_url.as_deref(),
		)
		.await?;
		out.insert(meta.project_id.clone(), meta);
	}

	Ok(out)
}
