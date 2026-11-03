-- When a file was added to the cluster, for sorting its content by date added.
-- installed_at only covers bundle files and is cleared when a bundle lets go of one
ALTER TABLE cluster_artifacts ADD COLUMN added_at TEXT;

UPDATE cluster_artifacts SET added_at = installed_at WHERE installed_at IS NOT NULL;

-- A trigger rather than a column default, which SQLite won't allow to be non-constant on
-- ALTER TABLE, and so every insert path stamps it. Relinking the same file is an upsert
-- and keeps its original time
CREATE TRIGGER cluster_artifacts_stamp_added_at
AFTER INSERT ON cluster_artifacts
WHEN NEW.added_at IS NULL
BEGIN
    UPDATE cluster_artifacts
    SET added_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
    WHERE cluster_id = NEW.cluster_id AND hash = NEW.hash;
END;
