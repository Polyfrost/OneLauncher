mod manager;
mod manifest;

pub use manager::VersionsManager;
pub use manifest::{
    MigrationSource, MigrationTarget, ReleaseTarget, RemoteMigration, VersionMetadata,
    VersionsManifest, added_release_targets, resolve_migration_chain,
};
