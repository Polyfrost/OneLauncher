//! Cluster records and everything that belongs to one: the settings profile it
//! launches with, and the logs and screenshots it writes.
//!
//! Not the whole of cluster management, though. Preparing a cluster for launch,
//! provisioning from the remote catalog and applying version migrations all need
//! Java, Minecraft metadata, the package store and the versions manifest; they
//! compose this crate rather than living in it, and stay in `oneclient_core`.
//!
//! It carries no event bus: nothing here reports progress or asks questions. It
//! does take a network client, for the one thing that needs it: uploading a log
//! to mclo.gs.

mod cluster;
mod error;
mod manager;
mod options;
mod profile;
mod stage;

pub mod logs;
pub mod profiles;
pub mod screenshots;

pub use cluster::{Cluster, ClusterLinkTarget};
pub use error::{ClusterError, ClusterResult};
pub use manager::ClusterManager;
pub use options::{ClusterUpdate, CreateClusterOptions};
pub use profile::{GameSettingsProfile, SettingsOsExtra};
pub use profiles::ProfileUpdate;
pub use stage::ClusterStage;
