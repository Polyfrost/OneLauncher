use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(ClusterBundleOverrides::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(ClusterBundleOverrides::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(ClusterBundleOverrides::ClusterId)
							.integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(ClusterBundleOverrides::BundleName)
							.text()
							.not_null(),
					)
					.col(
						ColumnDef::new(ClusterBundleOverrides::PackageId)
							.text()
							.not_null(),
					)
					.col(
						ColumnDef::new(ClusterBundleOverrides::OverrideType)
							.text()
							.not_null(),
					)
					.foreign_key(
						ForeignKey::create()
							.from(
								ClusterBundleOverrides::Table,
								ClusterBundleOverrides::ClusterId,
							)
							.to(Clusters::Table, Clusters::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("cluster_bundle_overrides_cluster_id_idx")
					.table(ClusterBundleOverrides::Table)
					.col(ClusterBundleOverrides::ClusterId)
					.to_owned(),
			)
			.await?;

		// Unique constraint ensures save_bundle_override upserts remain correct
		// even under concurrent callers, and prevents duplicate override rows.
		manager
			.create_index(
				Index::create()
					.name("cluster_bundle_overrides_unique_idx")
					.table(ClusterBundleOverrides::Table)
					.col(ClusterBundleOverrides::ClusterId)
					.col(ClusterBundleOverrides::BundleName)
					.col(ClusterBundleOverrides::PackageId)
					.unique()
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_index(
				Index::drop()
					.name("cluster_bundle_overrides_unique_idx")
					.table(ClusterBundleOverrides::Table)
					.to_owned(),
			)
			.await?;

		manager
			.drop_index(
				Index::drop()
					.name("cluster_bundle_overrides_cluster_id_idx")
					.table(ClusterBundleOverrides::Table)
					.to_owned(),
			)
			.await?;

		manager
			.drop_table(
				Table::drop()
					.table(ClusterBundleOverrides::Table)
					.to_owned(),
			)
			.await?;

		Ok(())
	}
}

#[derive(Iden)]
enum ClusterBundleOverrides {
	Table,
	Id,
	ClusterId,
	BundleName,
	PackageId,
	OverrideType,
}

#[derive(Iden)]
enum Clusters {
	Table,
	Id,
}
