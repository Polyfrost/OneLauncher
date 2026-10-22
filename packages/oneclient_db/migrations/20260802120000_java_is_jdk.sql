-- Whether a recorded runtime is a development kit rather than a bare runtime
-- image. Probing for it means spawning the java process, so it is remembered
-- alongside the version instead. Rows written before this migration default to
-- 0 and are corrected the next time they are probed.
ALTER TABLE `java_versions` ADD COLUMN `is_jdk` INTEGER NOT NULL DEFAULT 0;
