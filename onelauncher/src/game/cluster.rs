use std::{fs, io::Read, path::PathBuf};

use anyhow::anyhow;
use chrono::{DateTime, Local};
use flate2::bufread::GzDecoder;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{clients::ClientType, minecraft::MinecraftManifest};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
	pub id: Uuid,
    #[serde(rename = "manifest")]
	pub minecraft_manifest: MinecraftManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cluster {
	pub id: Uuid,
	pub created_at: DateTime<Local>,
	pub name: String,
	pub cover: Option<String>,
	pub group: Option<Uuid>,
	pub client: ClientType,
}

impl Cluster {
    pub fn dir(&self) -> crate::Result<PathBuf> {
        Ok(crate::utils::dirs::cluster_dir(self.id.to_string())?)
    }

    pub fn game_dir(&self) -> crate::Result<PathBuf> {
        Ok(self.dir()?.join("game"))
    }

    pub fn get_log_files(&self) -> crate::Result<Vec<PathBuf>> {
        let dir = self.game_dir()?.join("logs");
        let mut files = vec![];

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if path.file_name().unwrap_or_default().to_str().unwrap_or_default() == "latest.log" {
                    files.insert(0, path);
                } else {
                    files.push(path);
                }

            }
        }

        Ok(files)
    }

    pub fn get_log(&self, file: &PathBuf) -> crate::Result<String> {
        if !file.exists() {
            return Err(anyhow!("File does not exist").into());
        } 

        let ext = file.extension().unwrap_or_default().to_str().unwrap_or_default();
        if ext == "gz" {
            // Read a maximum of 256 KB of the file
            let buf = &mut [0; 256 * 1024];
            let file = fs::File::open(file)?.read(buf)?;

            let mut decoder = GzDecoder::new(&buf[..file]);
            let mut contents = String::new();
            decoder.read_to_string(&mut contents)?;

            return Ok(contents);
        } 

        Ok(fs::read_to_string(file)?)
    }
}