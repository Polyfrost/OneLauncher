//! Launch prep catalog provisioning and version migrations stay in
//! `oneclient_core` they need Java MC metadata and the package store
//! No event bus here the network client exists only for mclo.gs uploads

mod cluster;
mod error;
mod manager;
mod options;
mod profile;
mod stage;

pub mod identity;
pub mod logs;
pub mod profiles;
pub mod screenshots;

pub use cluster::{Cluster, ClusterLinkTarget, remove_mods_link};
pub use error::{ClusterError, ClusterResult};
pub use identity::InstanceIdentity;
pub use manager::ClusterManager;
pub use oneclient_db::models::ClusterKind;
pub use options::{ClusterUpdate, CreateClusterOptions};
pub use profile::{GameSettingsProfile, PackageUpdateMode, SettingsOsExtra};
pub use profiles::ProfileUpdate;
pub use stage::ClusterStage;
