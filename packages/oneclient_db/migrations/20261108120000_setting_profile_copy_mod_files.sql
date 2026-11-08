-- Copy mods into the instance instead of linking them. NULL inherits the global
-- profile, which is off.
ALTER TABLE setting_profiles ADD COLUMN copy_mod_files INTEGER;
