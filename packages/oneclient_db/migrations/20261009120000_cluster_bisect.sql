CREATE TABLE cluster_bisect_sessions (
    cluster_id   INTEGER PRIMARY KEY NOT NULL,
    round        INTEGER NOT NULL DEFAULT 0,
    pending_exit INTEGER NOT NULL DEFAULT 0,
    started_at   TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (cluster_id) REFERENCES clusters (id) ON DELETE CASCADE
);

CREATE TABLE cluster_bisect_mods (
    cluster_id    INTEGER NOT NULL,
    hash          TEXT NOT NULL,
    cleared_round INTEGER,
    testing       INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (cluster_id, hash),
    FOREIGN KEY (cluster_id) REFERENCES cluster_bisect_sessions (cluster_id) ON DELETE CASCADE
);
