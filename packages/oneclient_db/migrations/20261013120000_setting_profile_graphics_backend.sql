-- The graphics backend written to options.txt before launch. NULL inherits the
-- global profile, and a NULL global leaves options.txt untouched.
ALTER TABLE `setting_profiles` ADD COLUMN `graphics_backend` TEXT;
