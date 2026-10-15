ALTER TABLE `clusters` ADD COLUMN `kind` INTEGER NOT NULL DEFAULT 0;
ALTER TABLE `clusters` ADD COLUMN `user_created` INTEGER NOT NULL DEFAULT 0;
ALTER TABLE `clusters` ADD COLUMN `description` TEXT;
ALTER TABLE `clusters` ADD COLUMN `tags` TEXT NOT NULL DEFAULT '[]';
ALTER TABLE `clusters` ADD COLUMN `cover_path` TEXT;
CREATE INDEX `clusters_user_created_idx` ON `clusters` (`user_created`);
