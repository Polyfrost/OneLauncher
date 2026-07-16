-- Track the OS process behind a session so a restarted launcher can tell a
-- still-running game from one that exited while the launcher was closed.
-- `pid_started_at` is the process start time (unix seconds), used to reject
-- a recycled pid that now belongs to an unrelated process.
ALTER TABLE game_sessions ADD COLUMN pid INTEGER;
ALTER TABLE game_sessions ADD COLUMN pid_started_at INTEGER;
