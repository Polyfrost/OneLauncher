//! **OneLauncher API**
//!
//! API for interacting with our Rust core.

pub mod cluster;
pub mod handler;
pub mod ingress;
pub mod java;
pub mod logger;
pub mod metadata;
pub mod minecraft;
pub mod package;
pub mod processor;
pub mod proxy;
pub mod settings;

pub mod data {
	pub use crate::store::{
		ClusterMeta, Credentials, Directories, InitHooks, JavaOptions, Loader, ManagedDependency,
		ManagedPackage, ManagedUser, ManagedVersion, Memory, MinecraftCredentials, PackageData,
		PackageType, Resolution, Settings, Theme,
	};
}

pub mod prelude {
	pub use crate::cluster::{self, create, Cluster};
	pub use crate::data::*;
	pub use crate::proxy::InternetPayload;
	pub use crate::store::{ClusterPath, JavaVersions, PackageDependency, PackagePath};
	pub use crate::utils::io::{canonicalize, IOError};
	pub use crate::utils::java::JavaVersion;
	pub use crate::{java, metadata, minecraft, package, processor, settings, State};
}
