use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteBundleEntry {
    pub path: String,
    pub sha1: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleManifest {
    pub versions: HashMap<String, HashMap<String, Vec<RemoteBundleEntry>>>,
}

impl BundleManifest {
    pub fn remote_paths(&self) -> Vec<RemoteBundleRef> {
        let mut refs = Vec::new();

        for (mc_version, loaders) in &self.versions {
            for (loader, entries) in loaders {
                for entry in entries {
                    refs.push(RemoteBundleRef {
                        mc_version: mc_version.clone(),
                        loader: loader.clone(),
                        remote_path: entry.path.clone(),
                        sha1: entry.sha1.clone(),
                    });
                }
            }
        }

        refs
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteBundleRef {
    pub mc_version: String,
    pub loader: String,
    pub remote_path: String,
    pub sha1: String,
}
