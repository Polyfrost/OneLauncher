use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use oneclient_content::packages::store::{manifest, remove_entry};
use oneclient_db::DbPool;
use sqlx::Acquire;

use oneclient_common::paths;

use crate::LauncherState;
use crate::settings::data_dir;
use crate::settings::store::save_settings;
use crate::storage::{dir_size, format_bytes};

pub const IN_PROGRESS_MARKER: &str = ".oneclient_migrating";

static IN_PROGRESS: AtomicBool = AtomicBool::new(false);

#[must_use]
pub fn in_progress() -> bool {
	IN_PROGRESS.load(Ordering::SeqCst)
}

struct InProgress;

impl InProgress {
	fn begin() -> Self {
		IN_PROGRESS.store(true, Ordering::SeqCst);
		Self
	}
}

impl Drop for InProgress {
	fn drop(&mut self) {
		IN_PROGRESS.store(false, Ordering::SeqCst);
	}
}

const HEADROOM: f64 = 1.05;

const REPORT_EVERY: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RelocationPlan {
	pub from: PathBuf,
	pub to: PathBuf,
	pub bytes: u64,
	pub available: Option<u64>,
	pub warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationOutcome {
	pub to: PathBuf,
	pub bytes: u64,
	pub skipped_links: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leftovers {
	pub path: PathBuf,
	pub bytes: u64,
}

pub async fn plan(state: &LauncherState, picked: &Path) -> Result<RelocationPlan, String> {
	let from = current_dir()?;

	if let Some(reason) = busy_reason(state) {
		return Err(reason);
	}

	if let Some(reason) = restart_pending(state, &from) {
		return Err(reason);
	}

	let checked = data_dir::check(picked).await?;
	let to = checked.path;

	if to == from {
		return Err("Your game data is already there.".to_string());
	}

	if to.starts_with(&from) || from.starts_with(&to) {
		return Err(format!(
			"Pick a folder outside {} — one cannot hold the other.",
			from.display()
		));
	}

	if incomplete(&to) {
		return Err(format!(
			"{} holds a move that was interrupted. Delete that folder and try again.",
			to.display()
		));
	}

	if polyio::dir_has_content(&to).await {
		return Err(format!("{} is not empty. Pick an empty folder.", to.display()));
	}

	if let Some(old) = leftovers(state).await {
		return Err(format!(
			"Clear the {} still sitting in {} first — OneClient keeps track of one old folder at \
			 a time.",
			format_bytes(old.bytes),
			old.path.display()
		));
	}

	let bytes = collect(&from, &skipped_names(&from)).await?.total;
	let available = data_dir::available_space(&to);

	if let Some(available) = available
		&& (available as f64) < bytes as f64 * HEADROOM
	{
		return Err(format!(
			"{} needs {} and only {} is free there.",
			to.display(),
			format_bytes(bytes),
			format_bytes(available)
		));
	}

	Ok(RelocationPlan {
		from,
		to,
		bytes,
		available,
		warning: checked.warning,
	})
}

/// `on_copied` is handed the bytes written and the total to write, throttled to
/// one call per [`REPORT_EVERY`] bytes plus a final one when the tree lands
#[tracing::instrument(skip(state, plan, on_copied), fields(from = %plan.from.display(), to = %plan.to.display()))]
pub async fn relocate(
	state: &LauncherState,
	plan: &RelocationPlan,
	mut on_copied: impl FnMut(u64, u64),
) -> Result<RelocationOutcome, String> {
	if let Some(reason) = busy_reason(state) {
		return Err(reason);
	}

	if plan.from != current_dir()? {
		return Err("The data folder changed since this move was planned.".to_string());
	}

	if let Some(reason) = restart_pending(state, &plan.from) {
		return Err(reason);
	}

	let _in_progress = InProgress::begin();

	drop_materialized_content(state).await;

	let skip = skipped_names(&plan.from);
	let found = collect(&plan.from, &skip).await?;

	polyio::create_dir_all(&plan.to)
		.await
		.map_err(|err| format!("Couldn't create {}: {err}", plan.to.display()))?;

	let marker = plan.to.join(IN_PROGRESS_MARKER);
	polyio::write(&marker, b"".as_slice())
		.await
		.map_err(|err| format!("Couldn't write to {}: {err}", plan.to.display()))?;

	let copied = match write_everything(state, plan, &found, &mut on_copied).await {
		Ok(copied) => copied,
		Err(err) => {
			abandon(&plan.to).await;
			return Err(err);
		}
	};

	polyio::remove_file(&marker)
		.await
		.map_err(|err| format!("Couldn't finish the move at {}: {err}", plan.to.display()))?;

	if let Err(err) = commit(state, &plan.from, &plan.to).await {
		discard_unclaimed(&plan.to).await;
		return Err(err);
	}

	tracing::info!(
		bytes = copied,
		path = %plan.to.display(),
		"the game data moved; a restart picks up the new folder"
	);

	Ok(RelocationOutcome {
		to: plan.to.clone(),
		bytes: copied,
		skipped_links: found.links.len(),
	})
}

pub async fn leftovers(state: &LauncherState) -> Option<Leftovers> {
	let path = state.settings.read().previous_data_dir.clone()?;

	if current_dir().is_ok_and(|current| current == path) {
		return None;
	}

	if !polyio::try_exists(&path).await.unwrap_or(false) {
		return None;
	}

	Some(Leftovers {
		bytes: dir_size(&path).await,
		path,
	})
}

#[tracing::instrument(skip(state))]
pub async fn discard_leftovers(state: &LauncherState) -> Result<u64, String> {
	let Some(old) = state.settings.read().previous_data_dir.clone() else {
		return Ok(0);
	};

	let current = current_dir()?;
	if old == current {
		return Err("That is the folder you are using now.".to_string());
	}

	let freed = dir_size(&old).await;
	let keep = config_owned_names(&old);

	if keep.is_empty() {
		polyio::remove_dir_all(&old)
			.await
			.map_err(|err| format!("Couldn't remove {}: {err}", old.display()))?;
	} else {
		remove_children_except(&old, &keep).await?;
	}

	{
		let mut settings = state.settings.write();
		settings.previous_data_dir = None;
	}

	let snapshot = state.settings.read().clone();
	save_settings(&snapshot)
		.await
		.map_err(|err| format!("Couldn't save your settings: {err}"))?;

	Ok(freed)
}

#[must_use]
pub fn incomplete(dir: &Path) -> bool {
	dir.join(IN_PROGRESS_MARKER).exists()
}

fn current_dir() -> Result<PathBuf, String> {
	paths::data_dir()
		.map(Path::to_path_buf)
		.map_err(|err| format!("Couldn't work out where your game data lives: {err}"))
}

pub fn restart_pending(state: &LauncherState, current: &Path) -> Option<String> {
	let settled = state.settings.read().data_dir.clone()?;

	(settled != current).then(|| {
		format!(
			"A move to {} is already waiting. Restart OneClient to finish it.",
			settled.display()
		)
	})
}

fn busy_reason(state: &LauncherState) -> Option<String> {
	let active = state.games.active_ids().len();
	if active == 0 {
		return None;
	}

	Some(format!(
		"Close Minecraft first — {active} instance{} still open.",
		if active == 1 { " is" } else { "s are" }
	))
}

fn skipped_names(from: &Path) -> Vec<OsString> {
	let mut names = database_names();
	names.extend(config_owned_names(from));
	names
}

fn database_names() -> Vec<OsString> {
	let Ok(db) = paths::database_file() else {
		return Vec::new();
	};
	let Some(name) = db.file_name().and_then(|name| name.to_str()) else {
		return Vec::new();
	};

	["", "-wal", "-shm", "-journal"]
		.iter()
		.map(|suffix| OsString::from(format!("{name}{suffix}")))
		.collect()
}

fn config_owned_names(dir: &Path) -> Vec<OsString> {
	if paths::config_dir().is_ok_and(|config| config == dir) {
		[
			paths::settings_file(),
			paths::damaged_settings_file(),
			paths::auth_file(),
			paths::logs_dir(),
		]
		.into_iter()
		.flatten()
		.filter_map(|path| path.file_name().map(std::ffi::OsStr::to_os_string))
		.collect()
	} else {
		Vec::new()
	}
}

async fn drop_materialized_content(state: &LauncherState) {
	let mut dirs = Vec::new();

	if let Ok(shared) = paths::shared_minecraft_dir() {
		dirs.push(shared);
	}

	match state.clusters.list().await {
		Ok(clusters) => dirs.extend(
			clusters
				.iter()
				.filter(|cluster| cluster.uses_dedicated_dir())
				.filter_map(|cluster| cluster.game_dir().ok()),
		),
		Err(err) => tracing::warn!(%err, "could not list clusters; only clearing the shared folder"),
	}

	for dir in dirs {
		let Some(loaded) = manifest::load(&dir).await else {
			continue;
		};

		for entry in &loaded.entries {
			let mut path = dir.clone();
			path.extend(entry.path.split('/'));

			if let Err(err) = remove_entry(&path).await {
				tracing::warn!(path = %path.display(), %err, "could not clear linked package");
			}
		}

		manifest::clear(&dir).await;
	}

	if let Ok(shared) = paths::shared_minecraft_dir() {
		crate::game::unlink_cluster_logs(&shared).await;
	}
}

#[derive(Debug, Default)]
struct Found {
	dirs: Vec<PathBuf>,
	files: Vec<(PathBuf, u64)>,
	links: Vec<PathBuf>,
	total: u64,
}

async fn collect(root: &Path, skip_top: &[OsString]) -> Result<Found, String> {
	let mut found = Found::default();
	let mut stack = vec![(root.to_path_buf(), true)];

	while let Some((dir, is_top)) = stack.pop() {
		let mut entries = match polyio::read_dir(&dir).await {
			Ok(entries) => entries,
			Err(err) => {
				return Err(format!("Couldn't read {}: {err}", dir.display()));
			}
		};

		while let Ok(Some(entry)) = entries.next_entry().await {
			let path = entry.path();
			let name = entry.file_name();

			if is_top && skip_top.contains(&name) {
				continue;
			}

			let Ok(file_type) = entry.file_type().await else {
				continue;
			};

			let Ok(relative) = path.strip_prefix(root).map(Path::to_path_buf) else {
				continue;
			};

			if file_type.is_symlink() {
				found.links.push(relative);
				continue;
			}

			if file_type.is_dir() {
				found.dirs.push(relative);
				stack.push((path, false));
			} else if let Ok(meta) = entry.metadata().await {
				found.total += meta.len();
				found.files.push((relative, meta.len()));
			}
		}
	}

	Ok(found)
}

async fn copy_tree(
	from: &Path,
	to: &Path,
	found: &Found,
	on_copied: &mut impl FnMut(u64, u64),
) -> Result<u64, String> {
	for relative in &found.dirs {
		let dir = to.join(relative);
		polyio::create_dir_all(&dir)
			.await
			.map_err(|err| format!("Couldn't create {}: {err}", dir.display()))?;
	}

	for link in &found.links {
		tracing::warn!(path = %link.display(), "leaving a link behind; it points outside our tree");
	}

	let mut copied = 0u64;
	let mut reported = 0u64;

	for (relative, size) in &found.files {
		let src = from.join(relative);
		let dest = to.join(relative);

		if let Some(parent) = dest.parent() {
			polyio::create_dir_all(parent).await.ok();
		}

		let written = polyio::copy(&src, &dest)
			.await
			.map_err(|err| format!("Couldn't copy {}: {err}", relative.display()))?;

		if written != *size {
			return Err(format!(
				"{} arrived short. Nothing was removed from the old folder.",
				relative.display()
			));
		}

		copied += written;

		if copied - reported >= REPORT_EVERY {
			reported = copied;
			on_copied(copied, found.total);
		}
	}

	on_copied(copied, found.total.max(copied));
	Ok(copied)
}

async fn write_everything(
	state: &LauncherState,
	plan: &RelocationPlan,
	found: &Found,
	on_copied: &mut impl FnMut(u64, u64),
) -> Result<u64, String> {
	let copied = copy_tree(&plan.from, &plan.to, found, on_copied).await?;

	if copied != found.total {
		return Err(format!(
			"Only {} of {} arrived at {}.",
			format_bytes(copied),
			format_bytes(found.total),
			plan.to.display()
		));
	}

	let database = snapshot_database(&state.services.db, &plan.to).await?;

	let rewritten = rewrite_java_paths(&database, &plan.from, &plan.to).await?;
	if rewritten > 0 {
		tracing::info!(rows = rewritten, "repointed Java runtimes at the new folder");
	}

	Ok(copied)
}

async fn snapshot_database(pool: &DbPool, to: &Path) -> Result<PathBuf, String> {
	let Ok(name) = paths::database_file() else {
		return Err("Couldn't work out where the database lives.".to_string());
	};
	let Some(name) = name.file_name() else {
		return Err("Couldn't work out what the database is called.".to_string());
	};

	let target = to.join(name);

	sqlx::query("VACUUM INTO ?")
		.bind(target.to_string_lossy().as_ref())
		.execute(pool)
		.await
		.map_err(|err| format!("Couldn't copy the database to {}: {err}", target.display()))?;

	Ok(target)
}

async fn rewrite_java_paths(database: &Path, from: &Path, to: &Path) -> Result<u64, String> {
	let old = java_root(from);
	let new = java_root(to);

	let options = sqlx::sqlite::SqliteConnectOptions::new()
		.filename(database)
		.create_if_missing(false)
		.foreign_keys(false);

	let mut conn = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(&options)
		.await
		.map_err(|err| format!("Couldn't open the copied database: {err}"))?;

	let result = swap_java_prefix(&mut conn, &old, &new).await;

	if let Err(err) = sqlx::Connection::close(conn).await {
		tracing::warn!(%err, "failed to close the copied database cleanly");
	}

	result
}

fn java_root(dir: &Path) -> String {
	dir.join("metadata")
		.join("java")
		.to_string_lossy()
		.into_owned()
}

async fn swap_java_prefix(
	conn: &mut sqlx::SqliteConnection,
	old: &str,
	new: &str,
) -> Result<u64, String> {
	let len = i64::try_from(old.chars().count()).unwrap_or(i64::MAX);

	let mut tx = conn
		.begin()
		.await
		.map_err(|err| format!("Couldn't start a database transaction: {err}"))?;

	let profiles = sqlx::query(
		"UPDATE setting_profiles
            SET java_path = ? || substr(java_path, ?)
          WHERE java_path IS NOT NULL AND substr(java_path, 1, ?) = ?",
	)
	.bind(new)
	.bind(len + 1)
	.bind(len)
	.bind(old)
	.execute(&mut *tx)
	.await
	.map_err(|err| format!("Couldn't update your Java settings: {err}"))?;

	let runtimes = sqlx::query(
		"UPDATE java_versions
            SET absolute_path = ? || substr(absolute_path, ?)
          WHERE substr(absolute_path, 1, ?) = ?",
	)
	.bind(new)
	.bind(len + 1)
	.bind(len)
	.bind(old)
	.execute(&mut *tx)
	.await
	.map_err(|err| format!("Couldn't update your Java runtimes: {err}"))?;

	let orphans: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM setting_profiles p
          WHERE p.java_path IS NOT NULL
            AND NOT EXISTS (SELECT 1 FROM java_versions j WHERE j.absolute_path = p.java_path)",
	)
	.fetch_one(&mut *tx)
	.await
	.map_err(|err| format!("Couldn't check your Java settings: {err}"))?;

	if orphans > 0 {
		tx.rollback().await.ok();
		return Err(format!(
			"{orphans} of your profiles would lose their Java runtime. Nothing was changed."
		));
	}

	tx.commit()
		.await
		.map_err(|err| format!("Couldn't save the database changes: {err}"))?;

	Ok(profiles.rows_affected() + runtimes.rows_affected())
}

async fn commit(state: &LauncherState, from: &Path, to: &Path) -> Result<(), String> {
	{
		let mut settings = state.settings.write();
		settings.data_dir = Some(to.to_path_buf());
		settings.previous_data_dir = Some(from.to_path_buf());
	}

	let snapshot = state.settings.read().clone();

	save_settings(&snapshot).await.map_err(|err| {
		format!("Everything was copied but your settings could not be saved: {err}")
	})
}

async fn abandon(to: &Path) {
	if !incomplete(to) {
		tracing::error!(
			path = %to.display(),
			"leaving the destination alone; it is not carrying our marker"
		);
		return;
	}

	discard_unclaimed(to).await;
}

async fn discard_unclaimed(to: &Path) {
	match polyio::remove_dir_all(to).await {
		Ok(()) => tracing::info!(path = %to.display(), "cleared the abandoned folder"),
		Err(err) => tracing::error!(
			path = %to.display(),
			%err,
			"could not clear the abandoned folder; delete it before trying again"
		),
	}
}

async fn remove_children_except(dir: &Path, keep: &[OsString]) -> Result<(), String> {
	let mut entries = polyio::read_dir(dir)
		.await
		.map_err(|err| format!("Couldn't read {}: {err}", dir.display()))?;

	while let Ok(Some(entry)) = entries.next_entry().await {
		let name = entry.file_name();
		if keep.contains(&name) {
			continue;
		}

		let path = entry.path();
		let Ok(file_type) = entry.file_type().await else {
			continue;
		};

		let removed = if file_type.is_dir() {
			polyio::remove_dir_all(&path).await
		} else {
			polyio::remove_file(&path).await
		};

		if let Err(err) = removed {
			tracing::warn!(path = %path.display(), %err, "could not remove leftover");
		}
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_managed_runtime_sits_under_the_prefix_the_swap_looks_for() {
		let root = PathBuf::from("D:").join("OneClient");
		let prefix = java_root(&root);

		let runtime = paths::java_dir()
			.map(|dir| dir.join("21").join("bin").join("java"))
			.expect("java dir resolves");

		let managed = root.join("metadata").join("java").join("21");
		assert!(managed.to_string_lossy().starts_with(&prefix));
		assert!(runtime.to_string_lossy().contains("java"));
	}

	#[test]
	fn the_write_ahead_sidecars_are_never_copied_by_hand() {
		let names = database_names();

		assert!(
			names.iter().any(|name| name == "user_data.db-wal"),
			"a copy that took the database but not its log would arrive stale"
		);
		assert!(names.iter().any(|name| name == "user_data.db-shm"));
	}

	#[test]
	fn a_folder_that_is_not_the_config_folder_gives_nothing_up() {
		let names = config_owned_names(Path::new("D:").join("OneClient").as_path());

		assert!(
			names.is_empty(),
			"settings only stay behind when the source folder is the one holding them"
		);
	}

	#[tokio::test]
	async fn the_walk_skips_what_it_is_told_to_and_counts_the_rest() {
		let root = polyio::testing::ScratchDir::new("relocate_collect");
		let dir = root.path();
		polyio::create_dir_all(dir.join("metadata")).await.unwrap();

		polyio::write(dir.join("user_data.db"), b"skipped".as_slice())
			.await
			.unwrap();
		polyio::write(dir.join("metadata").join("kept.bin"), b"1234".as_slice())
			.await
			.unwrap();

		let found = collect(dir, &[OsString::from("user_data.db")]).await.unwrap();

		assert_eq!(found.total, 4, "only the file we did not skip counts");
		assert_eq!(found.files.len(), 1);
		assert_eq!(found.dirs.len(), 1, "the folder itself still has to be made");

		std::fs::remove_dir_all(dir).ok();
	}

	#[tokio::test]
	async fn a_folder_kept_out_of_the_copy_is_kept_out_of_the_total_too() {
		let root = polyio::testing::ScratchDir::new("relocate_skip_dir");
		let dir = root.path();
		polyio::create_dir_all(dir.join("logs")).await.unwrap();

		polyio::write(dir.join("logs").join("latest.log"), vec![0u8; 900])
			.await
			.unwrap();
		polyio::write(dir.join("kept.bin"), vec![0u8; 100])
			.await
			.unwrap();

		let found = collect(dir, &[OsString::from("logs")]).await.unwrap();

		assert_eq!(found.total, 100, "the skipped folder is not counted");
		assert!(
			found.dirs.is_empty(),
			"nor is it recreated at the destination"
		);

		std::fs::remove_dir_all(dir).ok();
	}
}
