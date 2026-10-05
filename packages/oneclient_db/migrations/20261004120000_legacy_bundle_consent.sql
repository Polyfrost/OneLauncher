INSERT OR IGNORE INTO cluster_bundle_overrides (cluster_id, bundle_name, package_id, override_type)
SELECT id, '*', '*', 'enabled'
FROM clusters
WHERE kind = 0 AND (mc_version = '26.3' OR mc_version LIKE '26.3.%');
