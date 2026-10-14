-- Marks a linked artifact as newly installed or freshly updated, so the package
-- manager can badge it. The status rides on `cluster_artifacts` rather than a
-- side table because that row *is* the identity of "this version of this mod in
-- this cluster": an update installs a new hash and unlinks the old one, so the
-- status can never outlive the version it describes, and removing the mod or
-- the cluster takes it with them.
--
-- Values are the `SeenStatus` discriminants: 0 = New, 1 = Updated, 2 = Seen.
--
-- The default is `Seen`, not `New`. Every artifact already linked when this
-- migration runs, and every artifact laid down by a cluster's first install,
-- must start unbadged -- otherwise a freshly created cluster would light up
-- every single mod. Only the update flows promote a row out of `Seen`.
--
-- The launcher resets every row back to `Seen` on startup, before it applies
-- pending updates, so a badge survives exactly until the next launch.
ALTER TABLE cluster_artifacts ADD COLUMN seen_status INTEGER NOT NULL DEFAULT 2;

CREATE INDEX cluster_artifacts_seen_status_idx ON cluster_artifacts (cluster_id, seen_status);
