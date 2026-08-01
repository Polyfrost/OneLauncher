//! Cluster orchestration: preparing a cluster for launch, provisioning from the
//! remote catalog, and applying version migrations.
//!
//! The records themselves, plus settings profiles, logs and screenshots, live
//! in `oneclient_cluster`. These three modules are what compose that crate with
//! Java, Minecraft metadata, the package store and the versions manifest, which
//! is why they stay here.

mod migrate;
pub(crate) mod prepare;
mod provision;
mod unlink_legacy;

pub use migrate::apply_remote_migrations;
pub use prepare::{estimate_cluster_download, prepare_cluster, prepare_cluster_locked};
pub use provision::{ensure_from_bundles, ensure_from_versions};
pub use unlink_legacy::{SweepReport, unlink_legacy_cluster_content};

// Re-exported so the rest of the launcher keeps addressing these through
// `clusters::`, as it did before the split.
pub use oneclient_cluster::{
    Cluster, ClusterError, ClusterLinkTarget, ClusterManager, ClusterStage, ClusterUpdate,
    CreateClusterOptions,
};
