use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::models::PackageMetadataRow;

pub async fn get_package_metadata_batch(
	pool: &SqlitePool,
	provider: i64,
	project_ids: &[String],
) -> Result<Vec<PackageMetadataRow>, sqlx::Error> {
	if project_ids.is_empty() {
		return Ok(Vec::new());
	}

	let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(
		"SELECT provider, project_id, name, summary, author, icon_url \
		 FROM package_metadata WHERE provider = ",
	);
	builder.push_bind(provider);
	builder.push(" AND project_id IN (");
	let mut separated = builder.separated(", ");
	for id in project_ids {
		separated.push_bind(id);
	}
	separated.push_unseparated(")");

	builder
		.build_query_as::<PackageMetadataRow>()
		.fetch_all(pool)
		.await
}

pub async fn upsert_package_metadata(
	pool: &SqlitePool,
	provider: i64,
	project_id: &str,
	name: &str,
	summary: &str,
	author: &str,
	icon_url: Option<&str>,
) -> Result<(), sqlx::Error> {
	sqlx::query!(
		r#"
		INSERT INTO package_metadata (provider, project_id, name, summary, author, icon_url, updated_at)
		VALUES (?, ?, ?, ?, ?, ?, datetime('now'))
		ON CONFLICT(provider, project_id) DO UPDATE SET
			name = excluded.name,
			summary = excluded.summary,
			author = excluded.author,
			icon_url = excluded.icon_url,
			updated_at = excluded.updated_at
		"#,
		provider,
		project_id,
		name,
		summary,
		author,
		icon_url
	)
	.execute(pool)
	.await?;

	Ok(())
}
