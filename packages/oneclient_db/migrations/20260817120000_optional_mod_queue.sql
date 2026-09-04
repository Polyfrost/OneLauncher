CREATE TABLE cluster_optional_mods (
    id                INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    cluster_id        INTEGER NOT NULL,
    bundle_name       TEXT NOT NULL,
    package_id        TEXT NOT NULL,
    bundle_version_id TEXT NOT NULL,
    seen_status       INTEGER NOT NULL,
    queued_at         TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (cluster_id) REFERENCES clusters (id) ON DELETE CASCADE,
    UNIQUE (cluster_id, package_id)
);

CREATE INDEX cluster_optional_mods_cluster_id_idx ON cluster_optional_mods (cluster_id);
