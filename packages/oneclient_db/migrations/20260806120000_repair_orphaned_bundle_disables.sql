-- Switches bundle content back on where it was left off with nothing recording
-- the choice.
--
-- `cluster_artifacts.enabled` and `cluster_bundle_overrides` are two records of
-- one decision and they were able to come apart: duplicate resolution wrote the
-- flag without the override, and re-resolving a package to a different bundle
-- deleted the override without the flag. Either way the artifact ends up off
-- with no override, which nothing could then repair -- the update flow decides
-- from the override and the manifest default, never from `enabled`, so it read
-- the package as installed and enabled and never touched it again. For a
-- bundle's hidden dependencies that meant a library the pack needs never
-- reaching `mods/`.
--
-- A genuine user disable always leaves a `disabled` or `removed` row for the
-- same bundle, so those are matched here and stay off.
UPDATE cluster_artifacts SET enabled = 1
WHERE bundle_name IS NOT NULL
  AND package_id IS NOT NULL
  AND enabled = 0
  AND NOT EXISTS (SELECT 1 FROM cluster_bundle_overrides o
                  WHERE o.cluster_id   = cluster_artifacts.cluster_id
                    AND o.bundle_name  = cluster_artifacts.bundle_name
                    AND o.package_id   = cluster_artifacts.package_id);
