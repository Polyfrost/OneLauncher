CREATE TABLE release_migration_waitlist (
    target_cluster_id INTEGER NOT NULL,
    provider INTEGER NOT NULL,
    project_id TEXT NOT NULL,
    content_type INTEGER NOT NULL,
    source_hash TEXT NOT NULL,
    display_name TEXT NOT NULL DEFAULT '',
    added_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    PRIMARY KEY (target_cluster_id, provider, project_id),
    FOREIGN KEY (target_cluster_id) REFERENCES clusters (id) ON DELETE CASCADE
);
CREATE INDEX release_migration_waitlist_expires_at_idx ON release_migration_waitlist (expires_at);
