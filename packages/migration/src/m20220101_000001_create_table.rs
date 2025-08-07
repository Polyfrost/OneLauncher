use chrono::Utc;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(ClusterGroups::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(ClusterGroups::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(ClusterGroups::Name).text().not_null())
					.to_owned(),
			)
			.await?;

		manager
			.create_table(
				Table::create()
					.table(JavaVersions::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(JavaVersions::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(JavaVersions::MajorVersion)
							.integer()
							.not_null(),
					)
					.col(ColumnDef::new(JavaVersions::VendorName).text().not_null())
					.col(ColumnDef::new(JavaVersions::AbsolutePath).text().not_null())
					.col(ColumnDef::new(JavaVersions::FullVersion).text().not_null())
					.col(ColumnDef::new(JavaVersions::Arch).text().not_null())
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("java_versions_major_version_idx")
					.table(JavaVersions::Table)
					.col(JavaVersions::MajorVersion)
					.to_owned(),
			)
			.await?;

		manager
			.create_table(
				Table::create()
					.table(Packages::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(Packages::Hash)
							.text()
							.not_null()
							.primary_key(),
					)
					.col(ColumnDef::new(Packages::FileName).text().not_null())
					.col(ColumnDef::new(Packages::VersionId).text().not_null())
					.col(ColumnDef::new(Packages::PublishedAt).date_time().not_null())
					.col(ColumnDef::new(Packages::DisplayName).text().not_null())
					.col(ColumnDef::new(Packages::DisplayVersion).text().not_null())
					.col(ColumnDef::new(Packages::PackageType).integer().not_null())
					.col(ColumnDef::new(Packages::Provider).integer().not_null())
					.col(ColumnDef::new(Packages::PackageId).text().not_null())
					.col(ColumnDef::new(Packages::McVersions).json().not_null())
					.col(ColumnDef::new(Packages::McLoader).integer().not_null())
					.col(ColumnDef::new(Packages::Icon).text().null())
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("packages_mc_loader_mc_versions_idx")
					.table(Packages::Table)
					.col(Packages::PackageType)
					.col(Packages::McLoader)
					.to_owned(),
			)
			.await?;

		manager
			.create_table(
				Table::create()
					.table(SettingProfiles::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(SettingProfiles::Name)
							.text()
							.not_null()
							.primary_key(),
					)
					.col(ColumnDef::new(SettingProfiles::JavaId).integer().null())
					.col(ColumnDef::new(SettingProfiles::Res).json().null())
					.col(
						ColumnDef::new(SettingProfiles::ForceFullscreen)
							.boolean()
							.null(),
					)
					.col(ColumnDef::new(SettingProfiles::MemMax).integer().null())
					.col(ColumnDef::new(SettingProfiles::LaunchArgs).text().null())
					.col(ColumnDef::new(SettingProfiles::LaunchEnv).text().null())
					.col(ColumnDef::new(SettingProfiles::HookPre).text().null())
					.col(ColumnDef::new(SettingProfiles::HookWrapper).text().null())
					.col(ColumnDef::new(SettingProfiles::HookPost).text().null())
					.col(ColumnDef::new(SettingProfiles::OsExtra).json().null())
					.foreign_key(
						ForeignKey::create()
							.from(SettingProfiles::Table, SettingProfiles::JavaId)
							.to(JavaVersions::Table, JavaVersions::Id),
					)
					.to_owned(),
			)
			.await?;

		manager
			.create_table(
				Table::create()
					.table(Clusters::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(Clusters::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(Clusters::FolderName).text().not_null())
					.col(
						ColumnDef::new(Clusters::Stage)
							.integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(Clusters::CreatedAt)
							.date_time()
							.not_null()
							.default(Utc::now()),
					)
					.col(ColumnDef::new(Clusters::GroupId).integer().null())
					.col(ColumnDef::new(Clusters::Name).text().not_null())
					.col(ColumnDef::new(Clusters::McVersion).text().not_null())
					.col(ColumnDef::new(Clusters::McLoader).integer().not_null())
					.col(ColumnDef::new(Clusters::McLoaderVersion).text().null())
					.col(ColumnDef::new(Clusters::LastPlayed).date_time().null())
					.col(ColumnDef::new(Clusters::OverallPlayed).integer().null())
					.col(ColumnDef::new(Clusters::IconUrl).text().null())
					.col(ColumnDef::new(Clusters::SettingProfileName).text().null())
					.col(ColumnDef::new(Clusters::LinkedModpackHash).text().null())
					.foreign_key(
						ForeignKey::create()
							.from(Clusters::Table, Clusters::GroupId)
							.to(ClusterGroups::Table, ClusterGroups::Id),
					)
					.foreign_key(
						ForeignKey::create()
							.from(Clusters::Table, Clusters::SettingProfileName)
							.to(SettingProfiles::Table, SettingProfiles::Name),
					)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("clusters_path_idx")
					.table(Clusters::Table)
					.col(Clusters::FolderName)
					.to_owned(),
			)
			.await?;

		manager
			.create_table(
				Table::create()
					.table(ClusterPackages::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(ClusterPackages::ClusterId)
							.integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(ClusterPackages::PackageHash)
							.text()
							.not_null(),
					)
					.primary_key(
						Index::create()
							.col(ClusterPackages::ClusterId)
							.col(ClusterPackages::PackageHash)
							.primary(),
					)
					.foreign_key(
						ForeignKey::create()
							.from(ClusterPackages::Table, ClusterPackages::ClusterId)
							.to(Clusters::Table, Clusters::Id),
					)
					.foreign_key(
						ForeignKey::create()
							.from(ClusterPackages::Table, ClusterPackages::PackageHash)
							.to(Packages::Table, Packages::Hash),
					)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
		Err(DbErr::Migration("no rollback available".to_owned()))
	}
}

#[derive(Iden)]
enum ClusterGroups {
	Table,
	Id,
	Name,
}

#[derive(Iden)]
enum JavaVersions {
	Table,
	Id,
	MajorVersion,
	VendorName,
	AbsolutePath,
	FullVersion,
	Arch,
}

#[derive(Iden)]
enum Packages {
	Table,
	Hash,
	FileName,
	VersionId,
	PublishedAt,
	DisplayName,
	DisplayVersion,
	PackageType,
	Provider,
	PackageId,
	McVersions,
	McLoader,
	Icon,
}

#[derive(Iden)]
enum SettingProfiles {
	Table,
	Name,
	JavaId,
	Res,
	ForceFullscreen,
	MemMax,
	LaunchArgs,
	LaunchEnv,
	HookPre,
	HookWrapper,
	HookPost,
	OsExtra,
}

#[derive(Iden)]
enum Clusters {
	Table,
	Id,
	FolderName,
	Stage,
	CreatedAt,
	GroupId,
	Name,
	McVersion,
	McLoader,
	McLoaderVersion,
	LastPlayed,
	OverallPlayed,
	IconUrl,
	SettingProfileName,
	LinkedModpackHash,
}

#[derive(Iden)]
enum ClusterPackages {
	Table,
	ClusterId,
	PackageHash,
}
