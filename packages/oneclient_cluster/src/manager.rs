use oneclient_db::dao::{cluster as cluster_dao, setting_profile as profile_dao};
use oneclient_db::models::{ClusterId, ClusterKind, ClusterPatch, NewCluster};

use crate::error::ClusterResult;
use crate::profile::GameSettingsProfile;
use crate::profiles::ProfileUpdate;
use crate::profiles::{create_profile_from_global, resolve_cluster_profile, update_named_profile};
use oneclient_common::domain::ContentType;
use oneclient_common::patch::Patch;

use oneclient_db::DbPool;
use tokio::sync::Mutex;

use crate::cluster::{Cluster, remove_mods_link};
use crate::error::ClusterError;
use crate::options::{ClusterUpdate, CreateClusterOptions};
use crate::stage::ClusterStage;

const COVER_STEM: &str = "cover";
const COVER_MAX_EDGE: u32 = 1200;

fn shrink_cover(raw: Vec<u8>) -> (Vec<u8>, bool) {
    let Ok(image) = image::load_from_memory(&raw) else {
        return (raw, false);
    };

    if image.width().max(image.height()) <= COVER_MAX_EDGE {
        return (raw, false);
    }

    let resized = image.resize(
        COVER_MAX_EDGE,
        COVER_MAX_EDGE,
        image::imageops::FilterType::Triangle,
    );

    let mut out = Vec::new();
    if resized
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .is_err()
    {
        return (raw, false);
    }

    (out, true)
}

pub struct ClusterManager {
    db: DbPool,
    /// Serialises creation so two concurrent creates cannot resolve to the same
    /// folder name and race each other onto disk
    provisioning: Mutex<()>,
}

impl ClusterManager {
    #[must_use]
    pub fn new(db: DbPool) -> Self {
        Self {
            db,
            provisioning: Mutex::new(()),
        }
    }

    /// Orphan-folder recovery must hold this across its whole scan or a
    /// concurrent create's folder is adopted into a duplicate row
    pub async fn provisioning_guard(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.provisioning.lock().await
    }

    pub fn sanitize_name(name: &str) -> String {
        let mut name = name.to_string();
        name.retain(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ' ' | '.' | '(' | ')')
        });
        name.trim().to_string()
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn get(&self, cluster_id: ClusterId) -> ClusterResult<Cluster> {
        let row = cluster_dao::get_by_id(&self.db, cluster_id)
            .await?
            .ok_or(ClusterError::NotFound(cluster_id))?;
        Cluster::try_from_row(row)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_by_folder_name(&self, folder_name: &str) -> ClusterResult<Option<Cluster>> {
        cluster_dao::get_by_folder_name(&self.db, folder_name)
            .await?
            .map(Cluster::try_from_row)
            .transpose()
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn list(&self) -> ClusterResult<Vec<Cluster>> {
        let rows = cluster_dao::list_all(&self.db).await?;
        rows.into_iter()
            .map(Cluster::try_from_row)
            .collect::<Result<Vec<_>, _>>()
    }

    #[tracing::instrument(skip(self))]
    pub async fn create(
        &self,
        global: &GameSettingsProfile,
        options: CreateClusterOptions,
    ) -> ClusterResult<Cluster> {
        let _guard = self.provisioning.lock().await;
        self.create_core(global, options).await
    }

    /// Returns `None` if a cluster for this version/loader already exists
    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn create_provisioned(
        &self,
        global: &GameSettingsProfile,
        mut options: CreateClusterOptions,
    ) -> ClusterResult<Option<Cluster>> {
        options.kind = ClusterKind::OneClient;
        options.user_created = false;

        let _guard = self.provisioning.lock().await;
        if cluster_dao::find_by_version_loader(
            &self.db,
            &options.mc_version,
            options.mc_loader as i64,
        )
        .await?
        .is_some()
        {
            return Ok(None);
        }
        self.create_core(global, options).await.map(Some)
    }

    /// Callers MUST hold `self.provisioning` this does not lock
    #[tracing::instrument(level = "debug", skip(self, global))]
    async fn create_core(
        &self,
        global: &GameSettingsProfile,
        options: CreateClusterOptions,
    ) -> ClusterResult<Cluster> {
        let folder_stem = Self::sanitize_name(&options.name);
        if folder_stem.is_empty() {
            return Err(ClusterError::EmptyName);
        }

        let name = if options.user_created {
            options.name.trim().to_string()
        } else {
            folder_stem.clone()
        };

        let folder_name = resolve_unique_folder_name(&folder_stem).await?;
        let cluster_path = oneclient_common::paths::clusters_dir()?.join(&folder_name);

        match create_inner(
            &self.db,
            global,
            &options,
            &name,
            &folder_name,
            &cluster_path,
        )
        .await
        {
            Ok(cluster) => {
                tracing::info!(cluster_id = cluster.id, name = %cluster.name, "created cluster");
                Ok(cluster)
            }
            Err(err) => {
                tracing::warn!(name = %name, error = %err, "cluster creation failed, cleaning up directory");
                let _ = polyio::remove_dir_all(&cluster_path).await;
                Err(err)
            }
        }
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn update(
        &self,
        cluster_id: ClusterId,
        update: ClusterUpdate,
    ) -> ClusterResult<Cluster> {
        let existing = self.get(cluster_id).await?;

        if let Patch::Set(ref profile_name) = update.setting_profile_name {
            ensure_profile_exists(&self.db, profile_name).await?;
        }

        let name = match update.name.as_deref() {
            Some(raw) if existing.user_created => Some(raw.trim().to_string()),
            Some(raw) => Some(Self::sanitize_name(raw)),
            None => None,
        };
        if name.as_deref().is_some_and(str::is_empty) {
            return Err(ClusterError::EmptyName);
        }

        let patch = ClusterPatch {
            name,
            setting_profile_name: update.setting_profile_name.into_db_patch(),
            mc_loader_version: update.mc_loader_version.into_db_patch(),
            linked_modpack_hash: update.linked_modpack_hash.into_db_patch(),
            description: update.description.into_db_patch(),
            tags: update
                .tags
                .map(|tags| serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string())),
            cover_path: update.cover_path.into_db_patch(),
        };

        let row = cluster_dao::update(&self.db, cluster_id, &patch).await?;
        let cluster = Cluster::try_from_row(row)?;

        if let Ok(dir) = cluster.dir() {
            crate::identity::write(&dir, &identity_of(&cluster)).await;
        }

        Ok(cluster)
    }

    pub async fn ensure_dedicated_marker(&self, cluster: &Cluster) -> ClusterResult<()> {
        if !cluster.is_isolated() || cluster.uses_dedicated_dir() {
            return Ok(());
        }

        let dir = cluster.dir()?;
        polyio::create_dir_all(&dir).await?;
        polyio::write(dir.join(oneclient_common::paths::DEDICATED_MARKER), b"")
            .await
            .ok();
        tracing::info!(
            cluster_id = cluster.id,
            "restored the dedicated directory marker"
        );
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn set_cover_from_file(
        &self,
        cluster_id: ClusterId,
        source: &std::path::Path,
    ) -> ClusterResult<String> {
        let cluster = self.get(cluster_id).await?;
        let dir = cluster.dir()?;
        polyio::create_dir_all(&dir).await?;

        let original_extension = source
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .filter(|ext| matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif"))
            .unwrap_or_else(|| "png".to_string());

        let raw = polyio::read(source).await?;
        let (bytes, shrunk) = tokio::task::spawn_blocking(move || shrink_cover(raw))
            .await
            .map_err(|err| ClusterError::StdIo(std::io::Error::other(err)))?;

        self.clear_cover_files(&dir).await;

        let extension = if shrunk {
            "png".to_string()
        } else {
            original_extension
        };
        let file_name = format!("{COVER_STEM}.{extension}");
        polyio::write(dir.join(&file_name), &bytes).await?;
        Ok(file_name)
    }

    pub async fn clear_cover(&self, cluster_id: ClusterId) -> ClusterResult<()> {
        let cluster = self.get(cluster_id).await?;
        if let Ok(dir) = cluster.dir() {
            self.clear_cover_files(&dir).await;
        }
        Ok(())
    }

    async fn clear_cover_files(&self, dir: &std::path::Path) {
        let Ok(mut entries) = polyio::read_dir(dir).await else {
            return;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name
                .rsplit_once('.')
                .is_some_and(|(stem, _)| stem == COVER_STEM)
            {
                polyio::remove_file(entry.path()).await.ok();
            }
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn delete(&self, cluster_id: ClusterId, remove_files: bool) -> ClusterResult<()> {
        let cluster = self.get(cluster_id).await?;

        if !cluster_dao::delete_by_id(&self.db, cluster_id).await? {
            return Err(ClusterError::NotFound(cluster_id));
        }

        remove_mods_link(&cluster.folder_name).await;

        if remove_files {
            let path = cluster.dir()?;
            if path.exists() {
                polyio::remove_dir_all(&path).await?;
            }
        }

        tracing::info!(cluster_id, remove_files, "deleted cluster");
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn set_stage(
        &self,
        cluster_id: ClusterId,
        stage: ClusterStage,
    ) -> ClusterResult<Cluster> {
        let row = cluster_dao::set_stage(&self.db, cluster_id, stage as i64).await?;
        Cluster::try_from_row(row)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn uses_dedicated_dir(&self, cluster_id: ClusterId) -> ClusterResult<bool> {
        Ok(self.get(cluster_id).await?.uses_dedicated_dir())
    }

    #[tracing::instrument(level = "debug", skip(self))]
    /// `is_running` is passed in because process liveness belongs to the game
    /// lifecycle not to cluster records
    pub async fn set_dedicated_dir(
        &self,
        cluster_id: ClusterId,
        dedicated: bool,
        is_running: bool,
    ) -> ClusterResult<()> {
        if is_running {
            return Err(ClusterError::AlreadyRunning(cluster_id));
        }

        let cluster = self.get(cluster_id).await?;
        if !dedicated && cluster.is_isolated() {
            return Err(ClusterError::DedicatedRequired(cluster_id));
        }

        let marker = cluster.dedicated_marker()?;
        if dedicated {
            polyio::create_dir_all(cluster.dir()?).await?;
            polyio::write(&marker, b"").await.ok();
        } else if marker.exists() {
            polyio::remove_file(&marker).await.ok();
        }
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn add_playtime(
        &self,
        cluster_id: ClusterId,
        duration: std::time::Duration,
    ) -> ClusterResult<Cluster> {
        let seconds = i64::try_from(duration.as_secs()).unwrap_or(i64::MAX);
        let row = cluster_dao::add_playtime(&self.db, cluster_id, seconds).await?;

        Cluster::try_from_row(row)
    }

    #[tracing::instrument(level = "debug", skip(self, global, cluster), fields(cluster_id = cluster.id))]
    pub async fn resolve_settings(
        &self,
        global: &GameSettingsProfile,
        cluster: &Cluster,
    ) -> ClusterResult<GameSettingsProfile> {
        resolve_cluster_profile(&self.db, global, cluster.setting_profile_name.as_deref()).await
    }

    #[tracing::instrument(level = "debug", skip(self, update))]
    pub async fn update_profile(
        &self,
        cluster_id: ClusterId,
        update: ProfileUpdate,
    ) -> ClusterResult<GameSettingsProfile> {
        let cluster = self.get(cluster_id).await?;
        let profile_name = cluster
            .setting_profile_name
            .ok_or(ClusterError::NoProfile)?;

        update_named_profile(&self.db, &profile_name, update).await
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn create_and_assign_profile(
        &self,
        global: &GameSettingsProfile,
        cluster_id: ClusterId,
        profile_name: &str,
    ) -> ClusterResult<GameSettingsProfile> {
        let profile =
            create_profile_from_global(&self.db, global, profile_name, None, None).await?;

        self.update(
            cluster_id,
            ClusterUpdate::default().setting_profile(&profile.name),
        )
        .await?;

        Ok(profile)
    }
}

#[tracing::instrument(level = "debug", skip(db, global, options))]
async fn create_inner(
    db: &DbPool,
    global: &GameSettingsProfile,
    options: &CreateClusterOptions,
    name: &str,
    folder_name: &str,
    cluster_path: &std::path::Path,
) -> ClusterResult<Cluster> {
    polyio::create_dir_all(cluster_path).await?;
    ensure_content_dirs(cluster_path).await?;

    if options.kind != ClusterKind::OneClient {
        polyio::write(
            cluster_path.join(oneclient_common::paths::DEDICATED_MARKER),
            b"",
        )
        .await
        .ok();
    }

    let profile =
        create_profile_from_global(db, global, folder_name, options.mem_max, None).await?;

    let tags = serde_json::to_string(&options.tags).unwrap_or_else(|_| "[]".to_string());

    let row = cluster_dao::insert(
        db,
        &NewCluster {
            name,
            folder_name,
            mc_version: &options.mc_version,
            mc_loader: options.mc_loader as i64,
            mc_loader_version: options.mc_loader_version.as_deref(),
            setting_profile_name: Some(&profile.name),
            stage: ClusterStage::NotReady as i64,
            kind: options.kind.as_i64(),
            user_created: i64::from(options.user_created),
            description: options.description.as_deref(),
            tags: &tags,
            cover_path: None,
        },
    )
    .await?;

    let cluster = Cluster::try_from_row(row)?;
    crate::identity::write(cluster_path, &identity_of(&cluster)).await;
    Ok(cluster)
}

fn identity_of(cluster: &Cluster) -> crate::identity::InstanceIdentity {
    crate::identity::InstanceIdentity {
        name: cluster.name.clone(),
        mc_version: cluster.mc_version.clone(),
        mc_loader: cluster.mc_loader,
        mc_loader_version: cluster.mc_loader_version.clone(),
        kind: cluster.kind,
        user_created: cluster.user_created,
        description: cluster.description.clone(),
        tags: cluster.tags.clone(),
        cover_path: cluster.cover_path.clone(),
    }
}

#[tracing::instrument(level = "debug", skip(pool))]
async fn ensure_profile_exists(pool: &oneclient_db::DbPool, name: &str) -> ClusterResult<()> {
    if profile_dao::get_by_name(pool, name).await?.is_none() {
        return Err(ClusterError::ProfileNotFound(name.to_string()));
    }
    Ok(())
}

#[tracing::instrument(level = "debug")]
async fn resolve_unique_folder_name(name: &str) -> ClusterResult<String> {
    let cluster_dir = oneclient_common::paths::clusters_dir()?;
    let mut folder_name = name.to_string();
    let mut path = cluster_dir.join(&folder_name);

    if path.exists() {
        let mut which = 1;
        loop {
            let candidate = format!("{folder_name} ({which})");
            path = cluster_dir.join(&candidate);
            if !path.exists() {
                folder_name = candidate;
                break;
            }
            which += 1;
        }
    }

    Ok(folder_name)
}

#[tracing::instrument(level = "debug")]
async fn ensure_content_dirs(cluster_path: &std::path::Path) -> ClusterResult<()> {
    for content_type in [
        ContentType::Mod,
        ContentType::ResourcePack,
        ContentType::Shader,
        ContentType::DataPack,
        ContentType::World,
    ] {
        polyio::create_dir_all(cluster_path.join(content_type.folder_name())).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{COVER_MAX_EDGE, shrink_cover};

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut out = Vec::new();
        image::DynamicImage::ImageRgb8(image::RgbImage::new(width, height))
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .expect("encode fixture");
        out
    }

    #[test]
    fn an_oversized_cover_is_shrunk_once_on_assignment() {
        let raw = png(COVER_MAX_EDGE * 2, COVER_MAX_EDGE);
        let original_len = raw.len();

        let (bytes, shrunk) = shrink_cover(raw);

        assert!(shrunk);
        assert!(bytes.len() < original_len);
        let decoded = image::load_from_memory(&bytes).expect("decode result");
        assert_eq!(decoded.width(), COVER_MAX_EDGE);
        assert_eq!(decoded.height(), COVER_MAX_EDGE / 2);
    }

    #[test]
    fn a_cover_that_already_fits_is_stored_untouched() {
        let raw = png(64, 64);
        let original = raw.clone();

        let (bytes, shrunk) = shrink_cover(raw);

        assert!(!shrunk);
        assert_eq!(bytes, original);
    }

    #[test]
    fn an_undecodable_cover_falls_back_to_the_original_bytes() {
        let (bytes, shrunk) = shrink_cover(b"not an image".to_vec());

        assert!(!shrunk);
        assert_eq!(bytes, b"not an image");
    }
}
