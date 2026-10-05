CREATE TABLE cluster_bundle_choices (
    cluster_id INTEGER NOT NULL,
    bundle_name TEXT NOT NULL,
    accepted INTEGER NOT NULL,
    PRIMARY KEY (cluster_id, bundle_name),
    FOREIGN KEY (cluster_id) REFERENCES clusters (id) ON DELETE CASCADE
);

-- Opt-in bundles used to install without asking, so whatever a cluster already holds counts as taken
INSERT OR IGNORE INTO cluster_bundle_choices (cluster_id, bundle_name, accepted)
SELECT DISTINCT cluster_id, bundle_name, 1
FROM cluster_artifacts
WHERE bundle_name IS NOT NULL;

-- A bundle setup recorded removals for and that installed nothing was turned down at setup
INSERT OR IGNORE INTO cluster_bundle_choices (cluster_id, bundle_name, accepted)
SELECT DISTINCT cluster_id, bundle_name, 0
FROM cluster_bundle_overrides
WHERE override_type = 'removed';
