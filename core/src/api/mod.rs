//! **OneLauncher API**
//! 
//! API for interacting with our Rust core.

pub mod java;
pub mod proxy;
pub mod handler;
pub mod settings;
pub mod ingress;
pub mod processor;
pub mod minecraft;
pub mod metadata;
pub mod logger;
pub mod package;
pub mod cluster;

pub mod data {
    pub use crate::store::{
        Credentials, MinecraftCredentials, Directories, InitHooks, JavaOptions,
        PackageData, Memory, Loader, ClusterMeta, Settings, Theme, Resolution,
        PackageType, ManagedDependency, ManagedPackage, ManagedUser, ManagedVersion,
    };
}

pub mod prelude {
    pub use crate::{
        data::*,
        proxy::InternetPayload,
        java, metadata, minecraft, package, processor,
        settings, store::JavaVersions, store::ClusterPath,
        utils::io::{canonicalize, IOError}, utils::java::JavaVersion,
        State,
    };
}
