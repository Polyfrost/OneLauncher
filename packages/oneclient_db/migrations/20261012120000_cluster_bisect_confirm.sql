ALTER TABLE cluster_bisect_mods ADD COLUMN name TEXT;
ALTER TABLE cluster_bisect_mods ADD COLUMN pinned_round INTEGER;
ALTER TABLE cluster_bisect_mods ADD COLUMN disable_on_finish INTEGER NOT NULL DEFAULT 0;

ALTER TABLE cluster_bisect_sessions ADD COLUMN confirmed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE cluster_bisect_sessions ADD COLUMN finishing INTEGER NOT NULL DEFAULT 0;
