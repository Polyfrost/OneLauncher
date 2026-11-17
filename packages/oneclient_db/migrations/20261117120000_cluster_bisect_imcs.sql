ALTER TABLE cluster_bisect_sessions ADD COLUMN search TEXT;
ALTER TABLE cluster_bisect_sessions DROP COLUMN confirmed;
ALTER TABLE cluster_bisect_sessions DROP COLUMN round;
ALTER TABLE cluster_bisect_mods DROP COLUMN cleared_round;
ALTER TABLE cluster_bisect_mods DROP COLUMN pinned_round;
