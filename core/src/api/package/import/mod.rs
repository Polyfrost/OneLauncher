//! Manages importing data from other launchers.

use std::{fmt, path::{Path, PathBuf}};
use serde::{Serialize, Deserialize};
use crate::{prelude::ClusterPath, utils::io::{self, IOError}};

pub mod atlauncher;
pub mod curseforge;
pub mod gdlauncher;
pub mod multibased;

/// List of launcher types we support importing from.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImportType {
    /// MultiMC based launchers
    MultiMC,
    /// Prism Launcher has its own category because it has different logic (also is objectively better than mmc)
    PrismLauncher,
    /// GDLauncher
    GDLauncher,
    /// Curseforge's launcher
    Curseforge,
    /// ATLauncher
    ATLauncher,
    /// Modrinth app.
    Modrinth,
    /// Unknown import option (not widely adopted -> probably a custom launcher with a similar file structure to the above)
    #[serde(other)]
    Unknown,
}

impl fmt::Display for ImportType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportType::MultiMC => write!(f, "MultiMC"),
            ImportType::ATLauncher => write!(f, "ATLauncher"),
            ImportType::Curseforge => write!(f, "Curseforge"),
            ImportType::GDLauncher => write!(f, "GDLauncher"),
            ImportType::Modrinth => write!(f, "Modrinth"),
            ImportType::PrismLauncher => write!(f, "PrismLauncher"),
            ImportType::Unknown => write!(f, "Unknown"),
        }
    }
}

pub async fn import_instances(import: ImportType, path: PathBuf) -> crate::Result<Vec<String>> {
    let instances_path = match import {
        ImportType::GDLauncher | ImportType::ATLauncher => "instances".to_string(),
        ImportType::Curseforge => "Instances".to_string(),
        ImportType::MultiMC => multibased::get_instances_path(path.clone().join("multimc.cfg")).await.unwrap_or_else(|| "instances".to_string()),
        ImportType::PrismLauncher => multibased::get_instances_path(path.clone().join("prismlauncher.cfg")).await.unwrap_or_else(|| "instances".to_string()),
        ImportType::Modrinth => "Profiles".to_string(),
        ImportType::Unknown | ImportType::Unknown => return Err(anyhow::anyhow!("launcher type unknown, cant import").into()),
    };

    let instances_dir = path.join(&instances_path);
    let mut instances = Vec::new();
    let mut dir = io::read_dir(&instances_dir).await.map_err(|_| {
        anyhow::anyhow!("invalid {import} launcher path, failed to import.")
    })?;

    while let Some(e) = dir.next_entry().await.map_err(|e| IOError::with_path(e, &instances_dir))? {
        let path = e.path();
        if path.is_dir() {
            if is_valid_instance(path.clone(), import).await {
                let name = path.file_name();
                if let Some(name) = name {
                    instances.push(name.to_string_lossy().to_string());
                }
            }
        }
    }

    Ok(instances)
}

#[tracing::instrument]
#[onelauncher_debug::debugger]
pub async fn import_instance(
    cluster_path: ClusterPath,
    import: ImportType,
    path: PathBuf,
    instance_path: String,
) -> crate::Result<()> {
    tracing::debug!("importing instance from {instance_path}");
    
    tracing::debug!("completed import of instance.");
    Ok(())
}

/// recursively get a [`Vec<PathBuf>`] of all subfiles.
#[onelauncher_debug::debugger]
#[async_recursion::async_recursion]
#[tracing::instrument]
pub async fn sub(path: &Path) -> crate::Result<Vec<PathBuf>> {
    if !path.is_dir() { return Ok(vec![path.to_path_buf()]); }
    let mut files = Vec::new();
    let mut dir = io::read_dir(&path).await?;
    while let Some(child) = dir.next_entry().await.map_err(|e| IOError::with_path(e, path))? {
        let path_child = child.path();
        files.append(&mut sub(&path_child).await?);
    }
    
    Ok(files)
}
