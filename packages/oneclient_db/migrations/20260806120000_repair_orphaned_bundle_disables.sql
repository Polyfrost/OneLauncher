-- Repairs bundle content whose two records of one decision came apart.
--
-- `cluster_artifacts.enabled` and `cluster_bundle_overrides` both record whether
-- a package is wanted, and the launcher was able to write one without the other:
-- duplicate resolution wrote the flag with no override, and re-resolving a
-- package to a different bundle deleted the override with no flag. Either way
-- the artifact ends up off with nothing recording why, which nothing could then
-- repair -- the update flow decides from the override and the manifest default
-- and never reads `enabled`, so it saw the package as installed and enabled and
-- never touched it again. For a bundle's hidden dependencies that meant a
-- library the pack needs never reaching `mods/`.

-- First, the objections that are no longer about anything. Declining a bundle at
-- onboarding recorded `removed` for its hidden files, including libraries that
-- an accepted bundle right beside it ships too -- so a cluster ends up claiming
-- to have removed a package it demonstrably installed, under a different bundle.
-- Left in place these answer for the package in every lookup that resolves a
-- user's choice across bundles, which is now how a choice is resolved.
DELETE FROM cluster_bundle_overrides
WHERE override_type = 'removed'
  AND EXISTS (SELECT 1 FROM cluster_artifacts a
              WHERE a.cluster_id  = cluster_bundle_overrides.cluster_id
                AND a.bundle_name IS NOT NULL
                AND a.bundle_name <> cluster_bundle_overrides.bundle_name
                AND a.package_id  = cluster_bundle_overrides.package_id);

-- Then the flag itself. A genuine user disable always leaves a `disabled` or
-- `removed` row behind, so those are matched here and stay off -- under any
-- bundle, not just the one the artifact sits under today, because a package that
-- moved bundles keeps its old row and that row still speaks for the user.
--
-- Deliberately narrower than the runtime pass in `bundles::heal_bundle_activity`,
-- which also frees hidden files from a `disabled` row on the grounds that a
-- dependency the user was never shown cannot have been turned off on purpose.
-- Whether a file is hidden is a manifest fact and there is no manifest here, so
-- those wait for the next bundle apply rather than being guessed at.
UPDATE cluster_artifacts SET enabled = 1
WHERE bundle_name IS NOT NULL
  AND package_id IS NOT NULL
  AND enabled = 0
  AND NOT EXISTS (SELECT 1 FROM cluster_bundle_overrides o
                  WHERE o.cluster_id    = cluster_artifacts.cluster_id
                    AND o.package_id    = cluster_artifacts.package_id
                    AND o.override_type IN ('disabled', 'removed'));
