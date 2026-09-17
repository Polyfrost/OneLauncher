mod manager;
mod manifest;

pub use manager::VersionsManager;
pub use manifest::{
    MigrationSource, MigrationTarget, ReleaseTarget, RemoteMigration, added_release_targets, VersionMetadata, VersionsManifest,
    resolve_migration_chain,
};
