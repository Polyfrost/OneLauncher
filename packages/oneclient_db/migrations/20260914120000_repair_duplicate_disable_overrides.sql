-- Repairs the overrides duplicate resolution filed against the user.
--
-- `reconcile_duplicate_activity` switched the older copy of a package off and
-- recorded it as a user disable, but `cluster_bundle_overrides` is keyed by
-- package id and the surviving copy shares it, so the row spoke for a copy
-- nobody turned off. Every lookup that resolves a user's choice then answers
-- "off" for the package: the next bundle update installs the new copy and
-- immediately disables it, and the update check stops shipping the file at all.
--
-- A row alongside a copy that is still on cannot be a considered choice about
-- the package -- had the user turned the package off, nothing of it would be
-- left enabled. Only those go. A package whose copies are all off is
-- indistinguishable from a genuine disable here and is left for the runtime
-- pass, which has the manifest and can tell whether the file was ever shown.
DELETE FROM cluster_bundle_overrides
WHERE override_type = 'disabled'
  AND EXISTS (SELECT 1 FROM cluster_artifacts a
              WHERE a.cluster_id = cluster_bundle_overrides.cluster_id
                AND a.package_id = cluster_bundle_overrides.package_id
                AND a.enabled    = 1);
