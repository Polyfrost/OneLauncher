//! Install modpacks from different sources.

use crate::data::{Loader, PackageData};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Modpack associated [`Cluster`] wrapper.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePackCluster {
	pub name: String,
	pub mc_version: String,
	pub mod_loader: Loader,
	pub loader_version: Option<String>,
	pub icon: Option<PathBuf>,
	pub icon_url: Option<String>,
	pub package_data: Option<PackageData>,
	pub skip: Option<bool>,
	pub skip_watch: Option<bool>,
}
