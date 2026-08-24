use std::path::{Path, PathBuf};

use oneclient_common::paths;

use super::launcher::LauncherSettings;
use super::store::save_settings;

pub const FOLDER_NAME: &str = "OneClient";
const LOW_SPACE_BYTES: u64 = 5 * 1000 * 1000 * 1000;

const PROBE_NAME: &str = ".oneclient_write_test";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDirCheck {
	pub path: PathBuf,
	pub warning: Option<String>,
}

#[must_use]
pub fn resolve(picked: &Path) -> PathBuf {
	if picked.file_name().is_some_and(|name| name == FOLDER_NAME) {
		picked.to_path_buf()
	} else {
		picked.join(FOLDER_NAME)
	}
}

pub async fn check(picked: &Path) -> Result<DataDirCheck, String> {
	let path = resolve(picked);

	let existed = polyio::try_exists(&path).await.unwrap_or(false);
	polyio::create_dir_all(&path)
		.await
		.map_err(|err| format!("Couldn't create {}: {err}", path.display()))?;

	let probe = path.join(PROBE_NAME);
	let writable = polyio::write(&probe, b"".as_slice()).await;
	polyio::remove_file(&probe).await.ok();

	if let Err(err) = writable {
		if !existed {
			polyio::remove_dir(&path).await.ok();
		}
		return Err(format!("Can't write to {}: {err}", path.display()));
	}

	if !existed {
		polyio::remove_dir(&path).await.ok();
	}

	Ok(DataDirCheck {
		warning: low_space_warning(&path),
		path,
	})
}

pub async fn apply(picked: Option<PathBuf>) -> Result<(), String> {
	let mut settings = LauncherSettings::default();
	let mut chosen = None;

	if let Some(picked) = picked {
		let checked = check(&picked).await?;

		polyio::create_dir_all(&checked.path)
			.await
			.map_err(|err| format!("Couldn't create {}: {err}", checked.path.display()))?;

		settings.data_dir = Some(checked.path.clone());
		chosen = Some(checked.path);
	}

	save_settings(&settings)
		.await
		.map_err(|err| format!("Couldn't save your settings: {err}"))?;

	if let Some(chosen) = chosen {
		paths::set_data_dir(chosen);
	}

	Ok(())
}

fn low_space_warning(path: &Path) -> Option<String> {
	let available = available_space(path)?;
	if available >= LOW_SPACE_BYTES {
		return None;
	}

	Some(format!(
		"Only {} free here. Minecraft, its libraries and a Java runtime need a few GB before you \
		 install anything.",
		crate::storage::format_bytes(available)
	))
}

fn available_space(path: &Path) -> Option<u64> {
	let target = polyio::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

	sysinfo::Disks::new_with_refreshed_list()
		.iter()
		.filter(|disk| target.starts_with(disk.mount_point()))
		.max_by_key(|disk| disk.mount_point().as_os_str().len())
		.map(sysinfo::Disk::available_space)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_drive_root_gets_a_folder_of_its_own() {
		let resolved = resolve(Path::new(std::path::MAIN_SEPARATOR_STR));
		assert_eq!(resolved.file_name().unwrap(), FOLDER_NAME);
	}

	#[test]
	fn resolving_twice_does_not_nest() {
		let once = resolve(Path::new("D:/Games"));
		assert_eq!(resolve(&once), once);
	}

	#[tokio::test]
	async fn a_writable_folder_passes_and_is_not_left_behind() {
		let root = polyio::testing::ScratchDir::new("data_dir_check");
		polyio::create_dir_all(root.path()).await.unwrap();

		let checked = check(root.path()).await.unwrap();
		assert_eq!(checked.path, root.join(FOLDER_NAME));
		assert!(
			!checked.path.exists(),
			"a folder the user has not confirmed yet is litter"
		);

		std::fs::remove_dir_all(root.path()).ok();
	}
}
