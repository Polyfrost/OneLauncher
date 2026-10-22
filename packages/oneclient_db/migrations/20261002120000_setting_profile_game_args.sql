-- Extra arguments appended to the Minecraft command line, after the version's
-- own game arguments. NULL inherits the global profile like `launch_args`.
ALTER TABLE `setting_profiles` ADD COLUMN `game_args` TEXT;
