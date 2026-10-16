//! The Modrinth App keeps everything in `app.db` (SQLite) next to a
//! `profiles/` folder holding each instance's game directory
//!
//! Two schemas are in circulation: the `profiles` table every release up to
//! mid 2026 used, and the `instances` + `instance_*` tables that replaced it.
//! Both are read; anything else about the database is left alone

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use crate::LauncherResult;
use oneclient_common::domain::{GameLoader, ProviderId};

use super::{ExternalDetection, ExternalInstance, ExternalLauncher, ExternalSettings, LinkedPack};

const DB_FILE: &str = "app.db";
const PROFILES_DIR: &str = "profiles";
const ICON_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif"];

pub fn default_root() -> Option<PathBuf> {
    let candidates = [
        super::env_dir("MODRINTH_APP_DIR"),
        super::env_dir("THESEUS_CONFIG_DIR"),
        directories::BaseDirs::new().map(|d| d.data_dir().join("ModrinthApp")),
        directories::BaseDirs::new().map(|d| d.data_dir().join("com.modrinth.theseus")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|root| looks_like_root(root))
}

pub fn looks_like_root(root: &Path) -> bool {
    root.join(DB_FILE).is_file()
}

#[tracing::instrument]
pub async fn detect_at(root: &Path) -> LauncherResult<Option<ExternalDetection>> {
    if !looks_like_root(root) {
        return Ok(None);
    }

    // The app keeps the database in WAL mode and may well be running, so a
    // snapshot (with its WAL) is read instead of the live file
    let snapshot = polyio::tempdir().await?;
    for suffix in ["", "-wal", "-shm"] {
        let name = format!("{DB_FILE}{suffix}");
        let src = root.join(&name);
        if src.is_file() {
            polyio::copy(&src, snapshot.dir_path().join(&name)).await?;
        }
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(snapshot.dir_path().join(DB_FILE)))
        .await?;

    let result = read_db(root, &pool).await;
    pool.close().await;

    let instances = result?;
    tracing::info!(instances = instances.len(), "detected Modrinth App");

    Ok(Some(ExternalDetection {
        launcher: ExternalLauncher::ModrinthApp,
        root: root.to_path_buf(),
        instances,
    }))
}

async fn read_db(root: &Path, pool: &SqlitePool) -> LauncherResult<Vec<ExternalInstance>> {
    let data_dir = custom_dir(pool)
        .await
        .filter(|dir| dir.is_dir())
        .unwrap_or_else(|| root.to_path_buf());
    let profiles_dir = data_dir.join(PROFILES_DIR);

    let mut instances = if has_table(pool, "instances").await {
        read_instances(pool, &profiles_dir).await?
    } else if has_table(pool, "profiles").await {
        read_legacy_profiles(pool, &profiles_dir).await?
    } else {
        Vec::new()
    };

    instances.sort_by_key(|instance| instance.name.to_lowercase());
    Ok(instances)
}

async fn has_table(pool: &SqlitePool, name: &str) -> bool {
    sqlx::query("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?")
        .bind(name)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .is_some()
}

async fn custom_dir(pool: &SqlitePool) -> Option<PathBuf> {
    let row = sqlx::query("SELECT custom_dir FROM settings LIMIT 1")
        .fetch_optional(pool)
        .await
        .ok()??;
    let dir: Option<String> = row.try_get("custom_dir").ok()?;
    dir.filter(|d| !d.trim().is_empty()).map(PathBuf::from)
}

async fn read_instances(
    pool: &SqlitePool,
    profiles_dir: &Path,
) -> LauncherResult<Vec<ExternalInstance>> {
    let groups = read_groups(pool).await;

    let rows = sqlx::query(
        "SELECT i.id, i.path, i.name, i.icon_path, \
                i.submitted_time_played, i.recent_time_played, \
                cs.game_version, cs.loader, cs.loader_version, \
                l.modrinth_project_id, l.modrinth_version_id, \
                json(o.overrides) AS overrides \
         FROM instances i \
         LEFT JOIN instance_content_sets cs ON cs.id = i.applied_content_set_id \
         LEFT JOIN instance_links l ON l.instance_id = i.id \
         LEFT JOIN instance_launch_overrides o ON o.instance_id = i.id",
    )
    .fetch_all(pool)
    .await?;

    let mut instances = Vec::with_capacity(rows.len());
    for row in rows {
        let id: String = row.try_get("id")?;
        let path: String = row.try_get("path")?;
        let Some(game_version) = row.try_get::<Option<String>, _>("game_version")? else {
            tracing::debug!(path, "modrinth: instance has no content set");
            continue;
        };
        let Some(loader) = parse_loader(
            &row.try_get::<Option<String>, _>("loader")?
                .unwrap_or_default(),
        ) else {
            continue;
        };

        let overrides: Option<Value> = row
            .try_get::<Option<String>, _>("overrides")?
            .and_then(|json| serde_json::from_str(&json).ok());
        let mut settings = overrides
            .as_ref()
            .map(settings_from_overrides)
            .unwrap_or_default();
        settings.java_major =
            java_major(overrides.as_ref().and_then(|v| v["java_path"].as_str())).await;

        instances.push(ExternalInstance {
            launcher: ExternalLauncher::ModrinthApp,
            game_dir: profiles_dir.join(&path),
            name: row.try_get("name")?,
            mc_version: game_version,
            loader,
            loader_version: row
                .try_get::<Option<String>, _>("loader_version")?
                .filter(|v| !v.is_empty()),
            icon: icon_path(row.try_get("icon_path")?),
            notes: None,
            groups: groups.get(&id).cloned().unwrap_or_default(),
            played_secs: played(&row),
            settings,
            linked_pack: linked_pack(
                row.try_get("modrinth_project_id")?,
                row.try_get("modrinth_version_id")?,
            ),
            id: path,
        });
    }
    Ok(instances)
}

/// Group memberships moved from `instance_groups(instance_id, group_name)`
/// to a definitions + memberships pair; either layout is accepted
async fn read_groups(pool: &SqlitePool) -> HashMap<String, Vec<String>> {
    let query = if has_table(pool, "instance_group_memberships").await {
        "SELECT m.instance_id AS instance_id, g.name AS group_name \
         FROM instance_group_memberships m JOIN instance_groups g ON g.id = m.group_id"
    } else {
        "SELECT instance_id, group_name FROM instance_groups"
    };

    let rows = match sqlx::query(query).fetch_all(pool).await {
        Ok(rows) => rows,
        Err(err) => {
            tracing::debug!(error = %err, "modrinth: could not read instance groups");
            return HashMap::new();
        }
    };

    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows {
        if let (Ok(instance), Ok(group)) = (
            row.try_get::<String, _>("instance_id"),
            row.try_get::<String, _>("group_name"),
        ) {
            groups.entry(instance).or_default().push(group);
        }
    }
    for list in groups.values_mut() {
        list.sort();
    }
    groups
}

async fn read_legacy_profiles(
    pool: &SqlitePool,
    profiles_dir: &Path,
) -> LauncherResult<Vec<ExternalInstance>> {
    let rows = sqlx::query(
        "SELECT path, name, icon_path, game_version, mod_loader, mod_loader_version, \
                json(groups) AS groups, linked_project_id, linked_version_id, \
                submitted_time_played, recent_time_played, \
                json(override_extra_launch_args) AS extra_launch_args, \
                json(override_custom_env_vars) AS custom_env_vars, \
                override_mc_memory_max, override_mc_force_fullscreen, \
                override_mc_game_resolution_x, override_mc_game_resolution_y, \
                override_hook_pre_launch, override_hook_wrapper, override_hook_post_exit, \
                override_java_path \
         FROM profiles",
    )
    .fetch_all(pool)
    .await?;

    let mut instances = Vec::with_capacity(rows.len());
    for row in rows {
        let path: String = row.try_get("path")?;
        let Some(loader) = parse_loader(&row.try_get::<String, _>("mod_loader")?) else {
            continue;
        };

        let json_column = |name: &str| -> Value {
            row.try_get::<Option<String>, _>(name)
                .ok()
                .flatten()
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or(Value::Null)
        };

        let width: Option<i64> = row.try_get("override_mc_game_resolution_x")?;
        let height: Option<i64> = row.try_get("override_mc_game_resolution_y")?;
        let settings = ExternalSettings {
            mem_max: row
                .try_get::<Option<i64>, _>("override_mc_memory_max")?
                .and_then(|m| u32::try_from(m).ok()),
            jvm_args: join_args(&json_column("extra_launch_args")),
            env: env_pairs(&json_column("custom_env_vars")),
            resolution: resolution(width, height),
            fullscreen: row
                .try_get::<Option<i64>, _>("override_mc_force_fullscreen")?
                .map(|v| v != 0),
            hook_pre: non_empty(row.try_get("override_hook_pre_launch")?),
            hook_wrapper: non_empty(row.try_get("override_hook_wrapper")?),
            hook_post: non_empty(row.try_get("override_hook_post_exit")?),
            java_major: java_major(
                row.try_get::<Option<String>, _>("override_java_path")?
                    .as_deref(),
            )
            .await,
        };

        let mut groups: Vec<String> = json_column("groups")
            .as_array()
            .map(|groups| {
                groups
                    .iter()
                    .filter_map(|g| g.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        groups.sort();

        instances.push(ExternalInstance {
            launcher: ExternalLauncher::ModrinthApp,
            game_dir: profiles_dir.join(&path),
            name: row.try_get("name")?,
            mc_version: row.try_get("game_version")?,
            loader,
            loader_version: row
                .try_get::<Option<String>, _>("mod_loader_version")?
                .filter(|v| !v.is_empty()),
            icon: icon_path(row.try_get("icon_path")?),
            notes: None,
            groups,
            played_secs: played(&row),
            settings,
            linked_pack: linked_pack(
                row.try_get("linked_project_id")?,
                row.try_get("linked_version_id")?,
            ),
            id: path,
        });
    }
    Ok(instances)
}

/// Only an instance that overrides Java says which one it ran on; without
/// an override the app picks Java by Minecraft version, as OneClient does
async fn java_major(java_path: Option<&str>) -> Option<u32> {
    let path = java_path.map(str::trim).filter(|p| !p.is_empty())?;
    super::java::major_from_install(Path::new(path)).await
}

fn parse_loader(raw: &str) -> Option<GameLoader> {
    match raw.to_ascii_lowercase().as_str() {
        "vanilla" | "minecraft" | "" => Some(GameLoader::Vanilla),
        "fabric" => Some(GameLoader::Fabric),
        "quilt" => Some(GameLoader::Quilt),
        "forge" => Some(GameLoader::Forge),
        "neoforge" => Some(GameLoader::NeoForge),
        other => {
            tracing::debug!(
                loader = other,
                "modrinth: skipping instance with unknown loader"
            );
            None
        }
    }
}

fn played(row: &sqlx::sqlite::SqliteRow) -> u64 {
    let total: i64 = ["submitted_time_played", "recent_time_played"]
        .iter()
        .filter_map(|col| row.try_get::<Option<i64>, _>(*col).ok().flatten())
        .map(|secs| secs.max(0))
        .sum();
    u64::try_from(total).unwrap_or(0)
}

fn icon_path(raw: Option<String>) -> Option<PathBuf> {
    let path = PathBuf::from(raw?);
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    (ICON_EXTENSIONS.contains(&ext.as_str()) && path.is_file()).then_some(path)
}

/// Only a pack still pointing at a specific version can be followed for
/// updates; a project-only link is a server, not a modpack
fn linked_pack(project: Option<String>, version: Option<String>) -> Option<LinkedPack> {
    let project = project.filter(|p| !p.is_empty())?;
    let version = version.filter(|v| !v.is_empty())?;
    Some(LinkedPack {
        provider: ProviderId::Modrinth,
        project_id: project,
        version_id: version,
    })
}

fn settings_from_overrides(value: &Value) -> ExternalSettings {
    let hooks = &value["hooks"];
    let resolution_pair = value["game_resolution"].as_array();
    ExternalSettings {
        mem_max: value["memory"]["maximum"]
            .as_u64()
            .and_then(|m| u32::try_from(m).ok()),
        jvm_args: join_args(&value["extra_launch_args"]),
        env: env_pairs(&value["custom_env_vars"]),
        resolution: resolution_pair.and_then(|pair| {
            resolution(
                pair.first().and_then(Value::as_i64),
                pair.get(1).and_then(Value::as_i64),
            )
        }),
        fullscreen: value["force_fullscreen"].as_bool(),
        hook_pre: non_empty(hooks["pre_launch"].as_str().map(str::to_string)),
        hook_wrapper: non_empty(hooks["wrapper"].as_str().map(str::to_string)),
        hook_post: non_empty(hooks["post_exit"].as_str().map(str::to_string)),
        java_major: None,
    }
}

fn join_args(value: &Value) -> Option<String> {
    let args: Vec<&str> = value
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .filter(|arg| !arg.trim().is_empty())
        .collect();
    (!args.is_empty()).then(|| args.join(" "))
}

fn env_pairs(value: &Value) -> Vec<(String, String)> {
    value
        .as_array()
        .map(|pairs| {
            pairs
                .iter()
                .filter_map(|pair| {
                    let pair = pair.as_array()?;
                    Some((
                        pair.first()?.as_str()?.to_string(),
                        pair.get(1)?.as_str()?.to_string(),
                    ))
                })
                .filter(|(key, _)| !key.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn resolution(width: Option<i64>, height: Option<i64>) -> Option<(u32, u32)> {
    Some((
        u32::try_from(width?).ok().filter(|w| *w > 0)?,
        u32::try_from(height?).ok().filter(|h| *h > 0)?,
    ))
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn make_db(root: &Path, schema: &'static str) -> SqlitePool {
        polyio::create_dir_all(root).await.unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(root.join(DB_FILE))
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::raw_sql(schema).execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn reads_the_legacy_profiles_table() {
        let tmp = polyio::tempdir().await.unwrap();
        let root = tmp.dir_path();
        let pool = make_db(
            root,
            "CREATE TABLE settings (id INTEGER, custom_dir TEXT NULL);
             INSERT INTO settings VALUES (0, NULL);
             CREATE TABLE profiles (
                path TEXT, name TEXT, icon_path TEXT NULL, game_version TEXT, mod_loader TEXT,
                mod_loader_version TEXT NULL, groups JSONB, linked_project_id TEXT NULL,
                linked_version_id TEXT NULL, submitted_time_played INTEGER, recent_time_played INTEGER,
                override_extra_launch_args JSONB, override_custom_env_vars JSONB,
                override_mc_memory_max INTEGER NULL, override_mc_force_fullscreen INTEGER NULL,
                override_mc_game_resolution_x INTEGER NULL, override_mc_game_resolution_y INTEGER NULL,
                override_hook_pre_launch TEXT NULL, override_hook_wrapper TEXT NULL,
                override_hook_post_exit TEXT NULL, override_java_path TEXT NULL);
             INSERT INTO profiles VALUES (
                'fabulous', 'Fabulous', NULL, '1.21.1', 'fabric', '0.16.5',
                jsonb('[\"Modded\"]'), 'pack', 'v1', 100, 20,
                jsonb('[\"-Xss4M\"]'), jsonb('[[\"FOO\",\"bar\"]]'),
                6144, 1, 1280, 720, 'echo pre', NULL, '', NULL);",
        )
        .await;
        pool.close().await;

        let detection = detect_at(root).await.unwrap().unwrap();
        let instance = &detection.instances[0];
        assert_eq!(instance.name, "Fabulous");
        assert_eq!(instance.game_dir, root.join("profiles/fabulous"));
        assert_eq!(instance.loader, GameLoader::Fabric);
        assert_eq!(instance.groups, vec!["Modded".to_string()]);
        assert_eq!(instance.played_secs, 120);
        assert_eq!(instance.settings.mem_max, Some(6144));
        assert_eq!(instance.settings.jvm_args.as_deref(), Some("-Xss4M"));
        assert_eq!(
            instance.settings.env,
            vec![("FOO".to_string(), "bar".to_string())]
        );
        assert_eq!(instance.settings.resolution, Some((1280, 720)));
        assert_eq!(instance.settings.fullscreen, Some(true));
        assert_eq!(instance.settings.hook_pre.as_deref(), Some("echo pre"));
        assert_eq!(instance.settings.hook_post, None);
        assert_eq!(
            instance.linked_pack.as_ref().map(|p| p.version_id.as_str()),
            Some("v1")
        );
    }

    #[tokio::test]
    async fn reads_the_instances_schema() {
        let tmp = polyio::tempdir().await.unwrap();
        let root = tmp.dir_path();
        let pool = make_db(
            root,
            "CREATE TABLE settings (id INTEGER, custom_dir TEXT NULL);
             CREATE TABLE instances (id TEXT, path TEXT, applied_content_set_id TEXT, name TEXT,
                icon_path TEXT NULL, submitted_time_played INTEGER, recent_time_played INTEGER);
             CREATE TABLE instance_content_sets (id TEXT, instance_id TEXT, game_version TEXT,
                loader TEXT, loader_version TEXT NULL);
             CREATE TABLE instance_links (instance_id TEXT, link_kind TEXT,
                modrinth_project_id TEXT NULL, modrinth_version_id TEXT NULL);
             CREATE TABLE instance_launch_overrides (instance_id TEXT, overrides JSONB);
             CREATE TABLE instance_groups (id TEXT, name TEXT, display_order INTEGER);
             CREATE TABLE instance_group_memberships (instance_id TEXT, group_id TEXT);
             INSERT INTO instances VALUES ('i1', 'vanilla-ish', 'cs1', 'Plain', NULL, 5, 0);
             INSERT INTO instance_content_sets VALUES ('cs1', 'i1', '1.20.4', 'vanilla', NULL);
             INSERT INTO instance_links VALUES ('i1', 'unmanaged', NULL, NULL);
             INSERT INTO instance_launch_overrides VALUES ('i1', jsonb('{\"memory\":{\"maximum\":3072},
                \"extra_launch_args\":null,\"custom_env_vars\":null,\"force_fullscreen\":null,
                \"game_resolution\":[854,480],\"hooks\":{\"pre_launch\":\"\",\"wrapper\":\"gamemoderun\",\"post_exit\":\"\"}}'));
             INSERT INTO instance_groups VALUES ('g1', 'Favorites', -1);
             INSERT INTO instance_group_memberships VALUES ('i1', 'g1');",
        )
        .await;
        pool.close().await;

        let detection = detect_at(root).await.unwrap().unwrap();
        let instance = &detection.instances[0];
        assert_eq!(instance.id, "vanilla-ish");
        assert_eq!(instance.loader, GameLoader::Vanilla);
        assert_eq!(instance.mc_version, "1.20.4");
        assert_eq!(instance.groups, vec!["Favorites".to_string()]);
        assert_eq!(instance.settings.mem_max, Some(3072));
        assert_eq!(instance.settings.resolution, Some((854, 480)));
        assert_eq!(
            instance.settings.hook_wrapper.as_deref(),
            Some("gamemoderun")
        );
        assert_eq!(instance.settings.hook_pre, None);
        assert_eq!(instance.linked_pack, None);
    }
}
