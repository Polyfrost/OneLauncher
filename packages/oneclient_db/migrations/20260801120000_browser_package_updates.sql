-- Auto-updates for packages the user installed from the browser, i.e. remote
-- provider content that no bundle provides. Bundles keep their own update flow
-- and are deliberately out of scope here.
--
-- `browser_update_mode` is the per-profile override of the launcher-wide
-- setting; NULL means "inherit the global value", like every other column on
-- this table. Values are the strings in `PackageUpdateMode::as_str`.
ALTER TABLE setting_profiles ADD COLUMN browser_update_mode TEXT;

-- Cache of the launch-time update check, so the package manager can mark a
-- package out of date without going back to the provider on every render, and
-- still knows about it after a restart.
--
-- Keyed by the artifact hash actually installed in the cluster: that is what
-- the package rows are keyed by, and it changes when the update is applied,
-- which is what makes a stale row impossible to mistake for a live one.
--
-- `skipped` records that the user was asked about *this* newer version and said
-- no. The row stays either way, because a skipped package is still out of date
-- and should still say so in the package manager; it just stops re-opening the
-- modal for the same version.
CREATE TABLE browser_package_updates (
    cluster_id INTEGER NOT NULL,
    hash TEXT NOT NULL,
    provider INTEGER NOT NULL,
    project_id TEXT NOT NULL,
    installed_version_id TEXT NOT NULL,
    installed_version_name TEXT NOT NULL DEFAULT '',
    latest_version_id TEXT NOT NULL,
    latest_version_name TEXT NOT NULL DEFAULT '',
    display_name TEXT NOT NULL DEFAULT '',
    checked_at TEXT NOT NULL,
    skipped INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (cluster_id, hash),
    FOREIGN KEY (cluster_id) REFERENCES clusters (id) ON DELETE CASCADE
);
CREATE INDEX browser_package_updates_cluster_id_idx ON browser_package_updates (cluster_id);
