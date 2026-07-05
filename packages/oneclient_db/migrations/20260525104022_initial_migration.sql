CREATE TABLE `java_versions` (
    `absolute_path` TEXT PRIMARY KEY NOT NULL,
    `major` INT NOT NULL,
    `version` TEXT NOT NULL,
    `vendor` TEXT NOT NULL,
    `os_arch` TEXT NOT NULL
);
CREATE INDEX `java_versions_major_idx` ON `java_versions` (`major`);

CREATE TABLE `setting_profiles` (
    `name` TEXT PRIMARY KEY NOT NULL,
    `java_path` TEXT,
    `resolution` TEXT,
    `force_fullscreen` INTEGER,
    `mem_max` INTEGER,
    `launch_args` TEXT,
    `launch_env` TEXT,
    `hook_pre` TEXT,
    `hook_wrapper` TEXT,
    `hook_post` TEXT,
    `os_extra` TEXT,
    FOREIGN KEY (`java_path`) REFERENCES `java_versions` (`absolute_path`)
);

CREATE TABLE `clusters` (
    `id` INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    `name` TEXT NOT NULL,
    `folder_name` TEXT NOT NULL,
    `setting_profile_name` TEXT,
    `mc_version` TEXT NOT NULL DEFAULT 'unknown',
    `mc_loader` INTEGER NOT NULL DEFAULT 0,
    `stage` INTEGER NOT NULL DEFAULT 0,
    `mc_loader_version` TEXT,
    `created_at` TEXT,
    `last_played` TEXT,
    `overall_played` INTEGER,
    `linked_modpack_hash` TEXT,
    FOREIGN KEY (`setting_profile_name`) REFERENCES `setting_profiles` (`name`)
);
CREATE INDEX `clusters_folder_name_idx` ON `clusters` (`folder_name`);

-- Content-addressed artifact cache (mods, packs, worlds, etc. - modpacks included)
CREATE TABLE artifacts (
    hash TEXT PRIMARY KEY NOT NULL,
    content_type INTEGER NOT NULL,
    path TEXT NOT NULL,
    file_name TEXT NOT NULL,
    size_bytes INTEGER
);
CREATE INDEX artifacts_content_type_idx ON artifacts (content_type);

-- Provider metadata for a specific project version (optional for local-only files)
CREATE TABLE provider_releases (
    provider INTEGER NOT NULL,
    project_id TEXT NOT NULL,
    version_id TEXT NOT NULL,
    hash TEXT NOT NULL,
    display_name TEXT NOT NULL,
    display_version TEXT NOT NULL,
    published_at TEXT,
    mc_versions TEXT NOT NULL DEFAULT '[]',
    mc_loaders TEXT NOT NULL DEFAULT '[]',
    PRIMARY KEY (provider, project_id, version_id),
    FOREIGN KEY (hash) REFERENCES artifacts (hash) ON DELETE CASCADE
);
CREATE INDEX provider_releases_hash_idx ON provider_releases (hash);

-- Links installed artifacts into a cluster instance folder
CREATE TABLE cluster_artifacts (
    cluster_id INTEGER NOT NULL,
    hash TEXT NOT NULL,
    cluster_file_name TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    bundle_name TEXT,
    bundle_version_id TEXT,
    package_id TEXT,
    installed_at TEXT,
    PRIMARY KEY (cluster_id, hash),
    FOREIGN KEY (cluster_id) REFERENCES clusters (id) ON DELETE CASCADE,
    FOREIGN KEY (hash) REFERENCES artifacts (hash) ON DELETE CASCADE
);
CREATE INDEX cluster_artifacts_cluster_id_idx ON cluster_artifacts (cluster_id);
CREATE INDEX cluster_artifacts_bundle_name_idx ON cluster_artifacts (cluster_id, bundle_name);

CREATE TABLE bundles (
    remote_path TEXT PRIMARY KEY NOT NULL,
    mc_version TEXT NOT NULL,
    mc_loader INTEGER NOT NULL,
    file_name TEXT NOT NULL,
    name TEXT,
    version_id TEXT,
    category TEXT,
    loader_version TEXT,
    disk_path TEXT NOT NULL,
    hidden INTEGER NOT NULL DEFAULT 0,
    etag TEXT,
    synced_at TEXT
);
CREATE INDEX bundles_mc_version_loader_idx ON bundles (mc_version, mc_loader);
CREATE INDEX bundles_hidden_idx ON bundles (hidden);

CREATE TABLE cluster_bundle_overrides (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    cluster_id INTEGER NOT NULL,
    bundle_name TEXT NOT NULL,
    package_id TEXT NOT NULL,
    override_type TEXT NOT NULL,
    FOREIGN KEY (cluster_id) REFERENCES clusters (id) ON DELETE CASCADE,
    UNIQUE (cluster_id, bundle_name, package_id)
);
CREATE INDEX cluster_bundle_overrides_cluster_id_idx ON cluster_bundle_overrides (cluster_id);

-- Local cache of remote provider project metadata, keyed by package id.
-- Lets package lists render names/icons/descriptions offline and without
-- refetching from the provider on every session. Icon bytes themselves live in
-- the on-disk image cache (keyed by url); here we persist the url -> package id
-- mapping plus the display fields.
CREATE TABLE package_metadata (
    provider INTEGER NOT NULL,
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    summary TEXT NOT NULL DEFAULT '',
    author TEXT NOT NULL DEFAULT '',
    icon_url TEXT,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (provider, project_id)
);

CREATE TABLE game_sessions (
    cluster_id INTEGER NOT NULL,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    exit_code INTEGER,
    ram_allocated_mb INTEGER NOT NULL,
    mods_enabled INTEGER NOT NULL DEFAULT 0,
    java_vendor TEXT,
    java_version TEXT,
    PRIMARY KEY (started_at, ended_at),
    UNIQUE (started_at),
    FOREIGN KEY (cluster_id) REFERENCES clusters(id) ON DELETE CASCADE
);
CREATE INDEX game_sessions_cluster_id_idx ON game_sessions(cluster_id);

CREATE TABLE game_session_servers (
    session_started_at TEXT NOT NULL,
    address TEXT NOT NULL,
    port INTEGER,
    joined_at TEXT NOT NULL,
    disconnected_at TEXT,
    PRIMARY KEY (joined_at, disconnected_at),
    FOREIGN KEY (session_started_at) REFERENCES game_sessions(started_at) ON DELETE CASCADE
);
CREATE INDEX game_session_servers_session_started_at_idx ON game_session_servers(session_started_at);