//! Deliberately damaging an installation, so the repair paths can be exercised
//! without waiting for a real bad connection.
//!
//! Every function here breaks files in a *specific* way, because the launcher
//! catches different damage at different layers and the distinction is the
//! whole point of testing it:
//!
//! - **Same length, wrong bytes** slips past the launch-time size check by
//!   design, and is only caught by the hashing pass behind "Verify Files".
//! - **Truncated** is what an interrupted download used to leave behind, and is
//!   caught cheaply on the next launch.
//! - **Absent** is the case that used to surface as a raw "No such file or
//!   directory" naming a path inside the launcher's own metadata folder.
//!
//! Destructive by nature. Everything is repairable by the launcher itself,
//! which is what makes it safe to point at a real installation.

use std::path::{Path, PathBuf};

use oneclient_content::packages::PackageStore;
use oneclient_content::packages::store::artifact_absolute_path;
use oneclient_common::paths;
use oneclient_db::dao::artifact as artifact_dao;

use crate::state::LauncherState;
use crate::LauncherResult;

/// What a simulation actually did, so the UI can say so rather than claiming
/// success over an installation that had nothing to break.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SimulationReport {
    pub affected: usize,
    /// A few of the paths touched, for a notification body. Not the full list:
    /// corrupting 50 assets should not produce a 50-line toast.
    pub samples: Vec<String>,
}

impl SimulationReport {
    #[must_use]
    pub fn summary(&self, verb: &str) -> String {
        if self.affected == 0 {
            return "Nothing to affect — is the game installed?".to_string();
        }

        let mut text = format!("{verb} {} file(s)", self.affected);
        if let Some(first) = self.samples.first() {
            text.push_str(&format!(": {first}"));
            if self.affected > 1 {
                text.push_str(&format!(" and {} more", self.affected - 1));
            }
        }
        text
    }

    fn push(&mut self, path: &Path) {
        self.affected += 1;
        if self.samples.len() < 3 {
            self.samples.push(
                path.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string()),
            );
        }
    }
}

/// How a file should be damaged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Damage {
    /// Flip bytes in the middle, keeping the length. Passes a size check and
    /// fails a hash check — the case "Verify Files" exists for.
    Corrupt,
    /// Cut the file to half its length, as an interrupted write would.
    Truncate,
    /// Remove it outright.
    Delete,
}

impl Damage {
    #[must_use]
    pub fn verb(self) -> &'static str {
        match self {
            Self::Corrupt => "Corrupted",
            Self::Truncate => "Truncated",
            Self::Delete => "Deleted",
        }
    }

    fn apply(self, path: &Path) -> std::io::Result<()> {
        match self {
            Self::Delete => std::fs::remove_file(path),
            Self::Truncate => {
                let len = std::fs::metadata(path)?.len();
                let file = std::fs::OpenOptions::new().write(true).open(path)?;
                file.set_len(len / 2)
            }
            Self::Corrupt => {
                let mut bytes = std::fs::read(path)?;
                if bytes.is_empty() {
                    return Ok(());
                }
                // Flipping every bit of a few bytes in the middle guarantees a
                // different hash without changing the length, and lands past
                // any header a reader might validate first — so the file stays
                // superficially plausible, exactly like real bit rot.
                let start = bytes.len() / 2;
                let end = (start + 8).min(bytes.len());
                for byte in &mut bytes[start..end] {
                    *byte = !*byte;
                }
                std::fs::write(path, &bytes)
            }
        }
    }
}

/// Damages `count` asset objects, spread across the sharded subdirectories.
///
/// Spread rather than taken in bulk from one shard because that is how real
/// damage arrives — a handful of failed requests scattered through a download —
/// and because it proves the verify pass walks the whole tree.
#[tracing::instrument(level = "debug")]
pub async fn damage_assets(count: usize, damage: Damage) -> LauncherResult<SimulationReport> {
    let dir = paths::assets_object_dir()?;

    tokio::task::spawn_blocking(move || {
        let mut report = SimulationReport::default();
        let Ok(shards) = std::fs::read_dir(&dir) else {
            return report;
        };

        // One file per shard before taking a second from any, so a small count
        // never all lands in `00/`.
        let mut shard_dirs: Vec<PathBuf> = shards
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        shard_dirs.sort();

        let mut round = 0;
        while report.affected < count {
            let mut progressed = false;

            for shard in &shard_dirs {
                if report.affected >= count {
                    break;
                }

                let Ok(entries) = std::fs::read_dir(shard) else {
                    continue;
                };
                let Some(path) = entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| path.is_file())
                    .nth(round)
                else {
                    continue;
                };

                progressed = true;
                match damage.apply(&path) {
                    Ok(()) => report.push(&path),
                    Err(err) => {
                        tracing::warn!(path = %path.display(), "could not damage: {err}");
                    }
                }
            }

            // Every shard is exhausted; asking for more than exists is not an
            // error, there simply are not that many assets installed.
            if !progressed {
                break;
            }
            round += 1;
        }

        tracing::warn!(
            affected = report.affected,
            ?damage,
            "simulated asset damage"
        );
        report
    })
    .await
    .map_err(std::io::Error::other)
    .map_err(Into::into)
}

/// Damages the installed libraries, which are on the classpath and so tend to
/// stop the game outright rather than degrade it.
#[tracing::instrument(level = "debug")]
pub async fn damage_libraries(count: usize, damage: Damage) -> LauncherResult<SimulationReport> {
    let dir = paths::libraries_dir()?;

    tokio::task::spawn_blocking(move || {
        let mut report = SimulationReport::default();
        let mut stack = vec![dir];

        while let Some(current) = stack.pop() {
            if report.affected >= count {
                break;
            }
            let Ok(entries) = std::fs::read_dir(&current) else {
                continue;
            };

            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().is_none_or(|ext| ext != "jar") {
                    continue;
                }
                if report.affected >= count {
                    break;
                }
                match damage.apply(&path) {
                    Ok(()) => report.push(&path),
                    Err(err) => tracing::warn!(path = %path.display(), "could not damage: {err}"),
                }
            }
        }

        tracing::warn!(
            affected = report.affected,
            ?damage,
            "simulated library damage"
        );
        report
    })
    .await
    .map_err(std::io::Error::other)
    .map_err(Into::into)
}

/// Removes the whole assets tree.
///
/// Reproduces the reported failure exactly: a cluster already marked `Ready`
/// skips preparation on every later launch, so nothing notices until something
/// tries to read a path that is no longer there.
#[tracing::instrument(level = "debug")]
pub async fn delete_assets_tree() -> LauncherResult<SimulationReport> {
    let dir = paths::assets_dir()?;

    let existed = polyio::try_exists(&dir).await.unwrap_or(false);
    if existed {
        tokio::fs::remove_dir_all(&dir)
            .await
            .map_err(std::io::Error::other)?;
    }

    tracing::warn!(path = %dir.display(), existed, "simulated a missing assets tree");

    Ok(SimulationReport {
        affected: usize::from(existed),
        samples: vec![dir.display().to_string()],
    })
}

/// Damages the installed content of one cluster.
///
/// Content is cached content-addressed, so a mod that no longer hashes to its
/// own key cannot be found again — the repair has to go back to the provider
/// for it, which is a different path from the game files.
#[tracing::instrument(level = "debug", skip(state))]
pub async fn damage_cluster_content(
    state: &std::sync::Arc<LauncherState>,
    cluster_id: i64,
    count: usize,
    damage: Damage,
) -> LauncherResult<SimulationReport> {
    let content = state.services.content();
    let linked = PackageStore::list_linked_artifacts(cluster_id, &content).await?;

    let mut paths = Vec::new();
    for link in linked {
        let Some(artifact) =
            artifact_dao::get_artifact_by_hash(&state.services.db, &link.hash).await?
        else {
            continue;
        };
        if let Ok(path) = artifact_absolute_path(&artifact.path) {
            paths.push(path);
        }
        if paths.len() >= count {
            break;
        }
    }

    tokio::task::spawn_blocking(move || {
        let mut report = SimulationReport::default();
        for path in paths {
            if !path.is_file() {
                continue;
            }
            match damage.apply(&path) {
                Ok(()) => report.push(&path),
                Err(err) => tracing::warn!(path = %path.display(), "could not damage: {err}"),
            }
        }

        tracing::warn!(
            affected = report.affected,
            ?damage,
            "simulated content damage"
        );
        report
    })
    .await
    .map_err(std::io::Error::other)
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "oneclient-simulate-{tag}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn corrupting_keeps_the_length_and_changes_the_bytes() {
        // The whole point of this mode: it has to survive a size check so that
        // only the hashing pass can catch it.
        let dir = scratch("corrupt");
        let file = dir.join("object.bin");
        let original: Vec<u8> = (0..=255u8).collect();
        std::fs::write(&file, &original).unwrap();

        Damage::Corrupt.apply(&file).unwrap();

        let after = std::fs::read(&file).unwrap();
        assert_eq!(after.len(), original.len(), "length must not change");
        assert_ne!(after, original, "bytes must change");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn corrupting_an_empty_file_is_a_no_op_rather_than_a_panic() {
        let dir = scratch("corrupt-empty");
        let file = dir.join("empty.bin");
        std::fs::write(&file, []).unwrap();

        Damage::Corrupt.apply(&file).unwrap();

        assert!(std::fs::read(&file).unwrap().is_empty());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn truncating_halves_the_file() {
        let dir = scratch("truncate");
        let file = dir.join("object.bin");
        std::fs::write(&file, vec![7u8; 100]).unwrap();

        Damage::Truncate.apply(&file).unwrap();

        assert_eq!(std::fs::metadata(&file).unwrap().len(), 50);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_report_over_nothing_says_so() {
        let report = SimulationReport::default();
        assert_eq!(
            report.summary("Corrupted"),
            "Nothing to affect — is the game installed?"
        );
    }

    #[test]
    fn a_report_names_a_sample_and_counts_the_rest() {
        let mut report = SimulationReport::default();
        report.push(Path::new("/tmp/assets/objects/00/aaaa"));
        report.push(Path::new("/tmp/assets/objects/01/bbbb"));
        report.push(Path::new("/tmp/assets/objects/02/cccc"));
        report.push(Path::new("/tmp/assets/objects/03/dddd"));

        assert_eq!(report.affected, 4);
        // Only a few names are kept, so a large run cannot produce a toast the
        // size of the asset index.
        assert_eq!(report.samples.len(), 3);
        assert_eq!(report.summary("Corrupted"), "Corrupted 4 file(s): aaaa and 3 more");
    }
}
