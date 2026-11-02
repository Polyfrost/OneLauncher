CREATE TABLE cluster_bisect_edges (
    cluster_id INTEGER NOT NULL,
    from_hash  TEXT NOT NULL,
    to_hash    TEXT NOT NULL,
    PRIMARY KEY (cluster_id, from_hash, to_hash),
    FOREIGN KEY (cluster_id) REFERENCES cluster_bisect_sessions (cluster_id) ON DELETE CASCADE
);

ALTER TABLE cluster_bisect_sessions ADD COLUMN retry_note TEXT;
