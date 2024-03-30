use crate::{
	constants::{self, MINECRAFT_VERSIONS_MANIFEST}, 
    create_game_client, 
    game::{client::{ClientTrait, Cluster, Manifest}, minecraft::{Library, MinecraftManifest, MinecraftVersion, RuleListExt}}, 
    impl_game_client, 
    utils::{dirs, http}
};
use anyhow::anyhow;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

pub fn vanilla_dir() -> crate::Result<PathBuf> {
    Ok(dirs::clients_dir()?.join("vanilla"))
}

create_game_client! {
	VanillaClientProps {}
	VanillaClient {}
}

#[async_trait]
impl<'a> ClientTrait<'a> for VanillaClient<'a> {
	impl_game_client!();

	fn new(cluster: &'a Cluster, manifest: &'a Manifest) -> Self {
		VanillaClient { cluster, manifest }
	}

	async fn launch(&self) -> crate::Result<()> {
		Ok(())
	}

	async fn setup(&self) -> crate::Result<()> {
        let manifest = &self.manifest.minecraft_manifest;

        let client_path = install_game(manifest).await?;
        let libraries = install_libraries(manifest).await?;
		Ok(())
	}
}

async fn install_game(manifest: &MinecraftManifest) -> crate::Result<PathBuf> {
    let file_name = format!("{}.jar", manifest.version);
    let file = vanilla_dir()?.join(file_name);

    if !file.exists() {
        fs::create_dir_all(&file.parent().ok_or(anyhow!("Couldn't take client jar parent folder"))?)?;
        
        let artifact = &manifest.downloads.client;
        http::download_file_sha1_check(&artifact.url, &file, &artifact.sha1).await?;
    }

    Ok(file)
}

pub async fn install_libraries(manifest: &MinecraftManifest) -> crate::Result<String> {
    let libraries = &manifest.libraries;

    let mut natives_ret: Vec<&Library> = vec![];
    let mut libraries_ret: Vec<String> = vec![];

    for library in libraries {
        if !&library.rules.check() {
            continue;
        }

        if let Some(_) = library.natives {
            natives_ret.push(library);
            continue;
        }

        let artifact = library.downloads.artifact.clone().ok_or(anyhow!("No artifact object"))?;
        let path = artifact.path;
        let url = artifact.url;
        
        let dest = dirs::libraries_dir()?.join(path);
        fs::create_dir_all(dest.parent().ok_or(anyhow!("Couldn't get library parent"))?)?;

        if !dest.exists() {
            http::download_file_sha1_check(url.as_str(), &dest, &artifact.sha1).await?;
        }

        libraries_ret.push(dest.to_str().unwrap().to_string());
    }
    
    install_natives(natives_ret).await?;
    Ok(libraries_ret.join(constants::LIBRARY_SPLITTER))
}

pub async fn install_natives(natives: Vec<&Library>) -> crate::Result<()> {
    for native in natives {
        let classifiers = native.natives.as_ref().unwrap();
        let classifier = match classifiers.get(constants::TARGET_OS) {
            Some(classifier) => classifier.replace("${arch}", constants::NATIVE_ARCH),
            None => continue,
        };

        let artifact = native.downloads.classifiers.as_ref()
            .ok_or(anyhow!("No classifiers object"))?
            .get(&classifier)
            .ok_or(anyhow!("No classifier object"))?;
        
        let path = artifact.path.clone();
        let url = artifact.url.clone();
        
        let dest = dirs::libraries_dir()?.join(path);
        fs::create_dir_all(dest.parent().ok_or(anyhow!("Couldn't get native parent"))?)?;

        if !dest.exists() {
            http::download_file_sha1_check(url.as_str(), &dest, &artifact.sha1).await?;
        }
    }

    Ok(())
}

pub async fn install_assets() -> crate::Result<()> {
    Ok(())
}


#[derive(Clone, Serialize, Deserialize)]
struct VersionList {
	versions: Vec<MinecraftVersion>,
}

#[derive(Serialize, Deserialize)]
struct CachedVersions {
	last_updated: i64,
	versions: Vec<MinecraftVersion>,
}

pub async fn get_versions(file: Option<&PathBuf>) -> crate::Result<Vec<MinecraftVersion>> {
	if let Some(file) = file {
		if file.exists() && file.is_file() {
            let cached = serde_json::from_str::<CachedVersions>(&fs::read_to_string(file)?)?;
            let head_request = http::create_client()?
                .head(MINECRAFT_VERSIONS_MANIFEST)
                .send()
                .await?;
        
            let last_updated = head_request
                .headers()
                .get("Last-Modified")
                .ok_or(anyhow!("Last-Modified header not found"))?;
            
            let last_updated = chrono::DateTime::parse_from_rfc2822(last_updated.to_str()?)?;
        
            if cached.last_updated > last_updated.timestamp() {
                return Ok(cached.versions);
            }
		}
	}

	let response = http::create_client()?
		.get(MINECRAFT_VERSIONS_MANIFEST)
		.send()
		.await?;
	let response = response.json::<VersionList>().await?;

	if let Some(file) = file {
		fs::write(
			file,
			serde_json::to_string(&CachedVersions {
				last_updated: chrono::Utc::now().timestamp(),
				versions: response.versions.clone(),
			})?,
		)?;
	}

	Ok(response.versions)
}

pub async fn retrieve_version_manifest(url: &str) -> crate::Result<MinecraftManifest> {
	let manifest = http::create_client()?
		.get(url)
		.send()
		.await?
		.json::<MinecraftManifest>()
		.await?;
	Ok(manifest)
}
