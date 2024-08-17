//! Import and Export `.mrpack` mod packs.

use crate::package::from::{set_cluster_information, EnvType, PackFile, PackFileHash};
use crate::prelude::{ClusterPath, ManagedVersion};
use crate::proxy::ingress_try_for_each;
use crate::proxy::send::{init_or_edit_ingress, send_ingress};
use crate::store::{ClusterStage, Clusters, PackageMetadata, PackageSide};
use crate::utils::http::{fetch_from_mirrors, fetch_json, write};
use crate::utils::io;
use crate::{cluster, IngressType, State};
use async_zip::base::read::seek::ZipFileReader;
use reqwest::Method;
use serde_json::json;

use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Component, PathBuf};

use super::from::{
	generate_pack_from_file, generate_pack_from_version_id, CreatePack, CreatePackLocation,
	PackFormat,
};

/// Install a pack
/// Wrapper around [`install_pack_files`] that generates a pack creation description, and
/// attempts to install the pack files. If it fails, it will remove the cluster (fail safely)
/// Install a modpack from a mrpack file (a modrinth .zip format)
#[onelauncher_macros::memory]
pub async fn install_zipped_mrpack(
	location: CreatePackLocation,
	cluster_path: ClusterPath,
) -> crate::Result<ClusterPath> {
	let create_pack: CreatePack = match location {
		CreatePackLocation::FromModrinth {
			package_id,
			version_id,
			title,
			icon_url,
		} => {
			generate_pack_from_version_id(
				package_id,
				version_id,
				title,
				icon_url,
				cluster_path.clone(),
				None,
			)
			.await?
		}
		CreatePackLocation::FromFile { path } => {
			generate_pack_from_file(path, cluster_path.clone()).await?
		}
	};

	let result = install_zipped_mrpack_files(create_pack, false).await;
	tokio::task::spawn(Clusters::update_versions());

	match result {
		Ok(cluster) => Ok(cluster),
		Err(err) => {
			let _ = crate::api::cluster::remove(&cluster_path).await;

			Err(err)
		}
	}
}

/// Install all pack files from a description. Does not remove the cluster if it fails
#[onelauncher_macros::memory]
pub async fn install_zipped_mrpack_files(
	create_pack: CreatePack,
	ignore_lock: bool,
) -> crate::Result<ClusterPath> {
	let state = &State::get().await?;
	let file = create_pack.file;
	let description = create_pack.description.clone(); // make a copy for cluster::edit
	let icon = create_pack.description.icon;
	let package_id = create_pack.description.package_id;
	let version_id = create_pack.description.version_id;
	let existing_ingress = create_pack.description.existing_ingress;
	let cluster_path = create_pack.description.cluster_path;
	let icon_exists = icon.is_some();
	let reader: Cursor<&bytes::Bytes> = Cursor::new(&file);

	let mut zip_reader = ZipFileReader::with_tokio(reader).await?;

	let zip_index_option = zip_reader
		.file()
		.entries()
		.iter()
		.position(|f| f.filename().as_str().unwrap_or_default() == "modrinth.index.json");
	if let Some(zip_index) = zip_index_option {
		let mut manifest = String::new();
		let mut reader = zip_reader.reader_with_entry(zip_index).await?;
		reader.read_to_string_checked(&mut manifest).await?;
		let pack: PackFormat = serde_json::from_str(&manifest)?;

		if &*pack.game != "minecraft" {
			return Err(anyhow::anyhow!("pack doesn't support Minecraft").into());
		}

		set_cluster_information(
			cluster_path.clone(),
			&description,
			&pack.name,
			&pack.dependencies,
			ignore_lock,
		)
		.await?;

		let cluster_path = cluster_path.clone();
		let ingress = init_or_edit_ingress(
			existing_ingress,
			IngressType::DownloadPackage {
				cluster_path: cluster_path.full_path().await?.clone(),
				package_name: pack.name.clone(),
				icon,
				package_id,
				package_version: version_id,
			},
			100.0,
			"downloading modpack",
		)
		.await?;

		let num_files = pack.files.len();
		use futures::StreamExt;
		ingress_try_for_each(
			futures::stream::iter(pack.files.into_iter()).map(Ok::<PackFile, crate::Error>),
			None,
			Some(&ingress),
			70.0,
			num_files,
			None,
			|pack| {
				let cluster_path = cluster_path.clone();
				async move {
					// TODO: prompt user for optional files in a modpack
					if let Some(env) = pack.env {
						if env
							.get(&EnvType::Client)
							.map(|x| x == &PackageSide::Unsupported)
							.unwrap_or(false)
						{
							return Ok(());
						}
					}

					let file = fetch_from_mirrors(
						&pack.downloads.iter().map(|x| &**x).collect::<Vec<&str>>(),
						pack.hashes.get(&PackFileHash::Sha1).map(|x| &**x),
						&state.fetch_semaphore,
					)
					.await?;

					let package_path = pack.path.to_string();
					let path = std::path::Path::new(&package_path).components().next();
					if let Some(Component::CurDir | Component::Normal(_)) = path {
						let path = cluster_path.full_path().await?.join(&package_path);
						write(&path, &file, &state.io_semaphore).await?;
					};
					Ok(())
				}
			},
		)
		.await?;

		send_ingress(&ingress, 0.0, Some("extracting overrides")).await?;

		let mut total_len = 0;

		for index in 0..zip_reader.file().entries().len() {
			let file = zip_reader.file().entries().get(index).unwrap();
			let filename = file.filename().as_str().unwrap_or_default();

			if (filename.starts_with("overrides") || filename.starts_with("client-overrides"))
				&& !filename.ends_with('/')
			{
				total_len += 1;
			}
		}

		for index in 0..zip_reader.file().entries().len() {
			let file = zip_reader.file().entries().get(index).unwrap();

			let filename = file.filename().as_str().unwrap_or_default();

			let file_path = PathBuf::from(filename);
			if (filename.starts_with("overrides") || filename.starts_with("client-overrides"))
				&& !filename.ends_with('/')
			{
				let mut content = Vec::new();
				let mut reader = zip_reader.reader_with_entry(index).await?;
				reader.read_to_end_checked(&mut content).await?;

				let mut new_path = PathBuf::new();
				let components = file_path.components().skip(1);

				for component in components {
					new_path.push(component);
				}

				if new_path.file_name().is_some() {
					write(
						&cluster_path.full_path().await?.join(new_path),
						&content,
						&state.io_semaphore,
					)
					.await?;
				}

				send_ingress(
					&ingress,
					30.0 / total_len as f64,
					Some(&format!("extracting override {}/{}", index, total_len)),
				)
				.await?;
			}
		}

		let potential_icon = cluster_path.full_path().await?.join("icon.png");
		if !icon_exists && potential_icon.exists() {
			cluster::edit_icon(&cluster_path, Some(&potential_icon)).await?;
		}

		if let Some(cluster) = cluster::get(&cluster_path, None).await? {
			crate::game::install_minecraft(&cluster, Some(ingress), false).await?;

			State::sync().await?;
		}

		Ok::<ClusterPath, crate::Error>(cluster_path.clone())
	} else {
		Err(anyhow::anyhow!("no pack manifest found in mrpack").into())
	}
}

#[tracing::instrument(skip(mrpack_file))]
#[onelauncher_macros::memory]
pub async fn remove_all_related_files(
	cluster_path: ClusterPath,
	mrpack_file: bytes::Bytes,
) -> crate::Result<()> {
	let reader: Cursor<&bytes::Bytes> = Cursor::new(&mrpack_file);

	let mut zip_reader = ZipFileReader::with_tokio(reader).await?;

	let zip_index_option = zip_reader
		.file()
		.entries()
		.iter()
		.position(|f| f.filename().as_str().unwrap_or_default() == "modrinth.index.json");
	if let Some(zip_index) = zip_index_option {
		let mut manifest = String::new();

		let mut reader = zip_reader.reader_with_entry(zip_index).await?;
		reader.read_to_string_checked(&mut manifest).await?;

		let pack: PackFormat = serde_json::from_str(&manifest)?;

		if &*pack.game != "minecraft" {
			return Err(anyhow::anyhow!("pack doesn't support Minecraft").into());
		}

		crate::api::cluster::edit(&cluster_path, |cl| {
			cl.stage = ClusterStage::PackDownloading;
			async { Ok(()) }
		})
		.await?;

		let state = State::get().await?;
		let all_hashes = pack
			.files
			.iter()
			.filter_map(|f| Some(f.hashes.get(&PackFileHash::Sha512)?.clone()))
			.collect::<Vec<_>>();

		let files_url = format!("{}version_files", crate::constants::MODRINTH_API_URL);

		let hash_packages = fetch_json::<HashMap<String, ManagedVersion>>(
			Method::POST,
			&files_url,
			None,
			Some(json!({
				"hashes": all_hashes,
				"algorithm": "sha512",
			})),
			&state.fetch_semaphore,
		)
		.await?;
		let to_remove = hash_packages
			.into_values()
			.map(|p| p.package_id)
			.collect::<Vec<_>>();
		let cluster = cluster::get(&cluster_path, None).await?.ok_or_else(|| {
			anyhow::anyhow!("{} is an unmanaged cluster!", cluster_path.to_string())
		})?;
		for (package_id, package) in &cluster.packages {
			if let PackageMetadata::Managed { package, .. } = &package.meta {
				if to_remove.contains(&package.id) {
					let path = cluster.get_full_path().await?.join(package_id.0.clone());
					if path.exists() {
						io::remove_file(&path).await?;
					}
				}
			}
		}

		for file in pack.files {
			let path: PathBuf = cluster_path.full_path().await?.join(file.path.to_string());
			if path.exists() {
				io::remove_file(&path).await?;
			}
		}

		for index in 0..zip_reader.file().entries().len() {
			let file = zip_reader.file().entries().get(index).unwrap();
			let filename = file.filename().as_str().unwrap_or_default();
			let file_path = PathBuf::from(filename);
			if (filename.starts_with("overrides") || filename.starts_with("client-overrides"))
				&& !filename.ends_with('/')
			{
				let mut new_path = PathBuf::new();
				let components = file_path.components().skip(1);

				for component in components {
					new_path.push(component);
				}

				let existing_file = cluster_path.full_path().await?.join(&new_path);
				if existing_file.exists() {
					io::remove_file(&existing_file).await?;
				}
			}
		}
		Ok(())
	} else {
		Err(anyhow::anyhow!("no pack manifest found in mrpack").into())
	}
}
