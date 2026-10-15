CREATE TABLE cluster_bundle_type_opt_outs (
    cluster_id INTEGER NOT NULL,
    bundle_name TEXT NOT NULL,
    content_type INTEGER NOT NULL,
    PRIMARY KEY (cluster_id, bundle_name, content_type),
    FOREIGN KEY (cluster_id) REFERENCES clusters (id) ON DELETE CASCADE
);
