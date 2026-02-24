use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.alter_table(
				Table::alter()
					.table(ClusterPackages::Table)
					.add_column(ColumnDef::new(ClusterPackages::BundleName).text().null())
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(ClusterPackages::Table)
					.add_column(
						ColumnDef::new(ClusterPackages::BundleVersionId)
							.text()
							.null(),
					)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(ClusterPackages::Table)
					.add_column(ColumnDef::new(ClusterPackages::PackageId).text().null())
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(ClusterPackages::Table)
					.add_column(
						ColumnDef::new(ClusterPackages::InstalledAt)
							.date_time()
							.null(),
					)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("cluster_packages_bundle_name_idx")
					.table(ClusterPackages::Table)
					.col(ClusterPackages::ClusterId)
					.col(ClusterPackages::BundleName)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("cluster_packages_package_id_idx")
					.table(ClusterPackages::Table)
					.col(ClusterPackages::PackageId)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_index(
				Index::drop()
					.name("cluster_packages_bundle_name_idx")
					.table(ClusterPackages::Table)
					.to_owned(),
			)
			.await?;

		manager
			.drop_index(
				Index::drop()
					.name("cluster_packages_package_id_idx")
					.table(ClusterPackages::Table)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(ClusterPackages::Table)
					.drop_column(ClusterPackages::BundleName)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(ClusterPackages::Table)
					.drop_column(ClusterPackages::BundleVersionId)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(ClusterPackages::Table)
					.drop_column(ClusterPackages::PackageId)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(ClusterPackages::Table)
					.drop_column(ClusterPackages::InstalledAt)
					.to_owned(),
			)
			.await?;

		Ok(())
	}
}

#[derive(Iden)]
enum ClusterPackages {
	Table,
	ClusterId,
	BundleName,
	BundleVersionId,
	PackageId,
	InstalledAt,
}
