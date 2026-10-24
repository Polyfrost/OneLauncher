ALTER TABLE cluster_bisect_edges ADD COLUMN kind INTEGER NOT NULL DEFAULT 1;

CREATE TABLE cluster_bisect_suggestions (
    cluster_id INTEGER NOT NULL,
    from_hash  TEXT,
    to_hash    TEXT NOT NULL,
    FOREIGN KEY (cluster_id) REFERENCES cluster_bisect_sessions (cluster_id) ON DELETE CASCADE
);

CREATE INDEX cluster_bisect_suggestions_cluster_id_idx ON cluster_bisect_suggestions (cluster_id);
