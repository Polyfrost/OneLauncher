use std::ffi::OsString;
use std::path::{Path, PathBuf};

use oneclient_common::paths;

use super::launcher::LauncherSettings;
use super::store::save_settings;

pub const FOLDER_NAME: &str = "OneClient";
const LOW_SPACE_BYTES: u64 = 5 * 1000 * 1000 * 1000;

const PROBE_NAME: &str = ".oneclient_write_test";

const OS_CLUTTER: &[&str] = &[
	".DS_Store",
	".localized",
	".Spotlight-V100",
	".Trashes",
	".fseventsd",
	"desktop.ini",
	"Thumbs.db",
	"$RECYCLE.BIN",
	"System Volume Information",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDirCheck {
	pub path: PathBuf,
	pub warning: Option<String>,
}

pub async fn looks_empty(dir: &Path, ignored: &[OsString]) -> bool {
	let Ok(mut entries) = polyio::read_dir(dir).await else {
		return true;
	};

	while let Ok(Some(entry)) = entries.next_entry().await {
		let name = entry.file_name();

		if ignored.contains(&name) {
			continue;
		}

		let Some(name) = name.to_str() else {
			return false;
		};

		if !OS_CLUTTER.iter().any(|junk| junk.eq_ignore_ascii_case(name)) {
			return false;
		}
	}

	true
}

pub async fn resolve(picked: &Path) -> PathBuf {
	if picked.file_name().is_some_and(|name| name == FOLDER_NAME)
		|| looks_empty(picked, &[]).await
	{
		picked.to_path_buf()
	} else {
		picked.join(FOLDER_NAME)
	}
}

pub fn default_path() -> Result<PathBuf, String> {
	paths::config_dir()
		.map(Path::to_path_buf)
		.map_err(|err| format!("Couldn't work out where OneClient keeps its settings: {err}"))
}

#[must_use]
pub fn is_default(dir: &Path) -> bool {
	default_path().is_ok_and(|home| home == dir)
}

pub async fn check(picked: &Path) -> Result<DataDirCheck, String> {
	check_exact(&resolve(picked).await).await
}

pub async fn check_exact(path: &Path) -> Result<DataDirCheck, String> {
	let path = path.to_path_buf();

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

pub(crate) fn available_space(path: &Path) -> Option<u64> {
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

	#[tokio::test]
	async fn a_folder_with_things_in_it_gets_a_folder_of_its_own() {
		let root = polyio::testing::ScratchDir::new("data_dir_busy");
		let dir = root.path();
		polyio::create_dir_all(dir).await.unwrap();
		polyio::write(dir.join("holiday.jpg"), b"mine".as_slice())
			.await
			.unwrap();

		assert_eq!(resolve(dir).await, dir.join(FOLDER_NAME));

		std::fs::remove_dir_all(dir).ok();
	}

	#[tokio::test]
	async fn an_empty_folder_is_used_as_it_stands() {
		let root = polyio::testing::ScratchDir::new("data_dir_empty");
		let dir = root.path();
		polyio::create_dir_all(dir).await.unwrap();

		assert_eq!(
			resolve(dir).await,
			dir.to_path_buf(),
			"a folder made for this should not get another one inside it"
		);

		std::fs::remove_dir_all(dir).ok();
	}

	#[tokio::test]
	async fn a_folder_the_finder_has_been_in_still_counts_as_empty() {
		let root = polyio::testing::ScratchDir::new("data_dir_clutter");
		let dir = root.path();
		polyio::create_dir_all(dir).await.unwrap();
		polyio::write(dir.join(".DS_Store"), b"junk".as_slice())
			.await
			.unwrap();

		assert_eq!(
			resolve(dir).await,
			dir.to_path_buf(),
			"the folder looks empty to the person who picked it"
		);

		std::fs::remove_dir_all(dir).ok();
	}

	#[tokio::test]
	async fn resolving_twice_does_not_nest() {
		let root = polyio::testing::ScratchDir::new("data_dir_nest");
		let dir = root.path();
		polyio::create_dir_all(dir).await.unwrap();
		polyio::write(dir.join("holiday.jpg"), b"mine".as_slice())
			.await
			.unwrap();

		let once = resolve(dir).await;
		assert_eq!(resolve(&once).await, once);

		std::fs::remove_dir_all(dir).ok();
	}

	#[tokio::test]
	async fn the_settings_that_stayed_behind_do_not_make_the_default_folder_look_busy() {
		let root = polyio::testing::ScratchDir::new("data_dir_settled");
		let dir = root.path();
		polyio::create_dir_all(dir).await.unwrap();
		polyio::write(dir.join("settings.json"), b"{}".as_slice())
			.await
			.unwrap();

		let settled = [OsString::from("settings.json")];

		assert!(
			looks_empty(dir, &settled).await,
			"moving back in has to be allowed over the settings that never left"
		);

		polyio::create_dir_all(dir.join("clusters")).await.unwrap();

		assert!(
			!looks_empty(dir, &settled).await,
			"game data already sitting there is another matter"
		);

		std::fs::remove_dir_all(dir).ok();
	}

	#[tokio::test]
	async fn a_writable_folder_passes_and_is_not_left_behind() {
		let root = polyio::testing::ScratchDir::new("data_dir_check");
		polyio::create_dir_all(root.path()).await.unwrap();

		polyio::write(root.join("holiday.jpg"), b"mine".as_slice())
			.await
			.unwrap();

		let checked = check(root.path()).await.unwrap();
		assert_eq!(checked.path, root.join(FOLDER_NAME));
		assert!(
			!checked.path.exists(),
			"a folder the user has not confirmed yet is litter"
		);

		std::fs::remove_dir_all(root.path()).ok();
	}
}
