-- What a release says it needs alongside it.
--
-- The providers hand this out with every version, but until now it was read
-- once at install time and thrown away, which left the launcher unable to
-- answer the reverse question — "what in this cluster depends on that?" —
-- without going back to the network for every installed package. Disabling a
-- library is exactly that question, and it has to be answerable offline and
-- fast enough to sit in front of a toggle.
--
-- Rows are keyed by the release that declares them, so they cascade away with
-- the artifact like the rest of a release's metadata.
CREATE TABLE provider_release_dependencies (
    provider INTEGER NOT NULL,
    project_id TEXT NOT NULL,
    version_id TEXT NOT NULL,
    -- Modrinth pins either a project, a version or both; CurseForge only ever
    -- names the project. The id the provider left out is stored as '' rather
    -- than NULL: SQLite treats NULLs in a primary key as distinct, so a
    -- nullable column here would let the same dependency be inserted twice.
    dependency_project_id TEXT NOT NULL DEFAULT '',
    dependency_version_id TEXT NOT NULL DEFAULT '',
    -- `DependencyKind::as_str`: required, optional, incompatible, embedded.
    kind TEXT NOT NULL,
    PRIMARY KEY (provider, project_id, version_id, dependency_project_id, dependency_version_id),
    FOREIGN KEY (provider, project_id, version_id)
        REFERENCES provider_releases (provider, project_id, version_id) ON DELETE CASCADE
);
-- The reverse lookup: everything that names this project as a dependency.
CREATE INDEX provider_release_dependencies_target_idx
    ON provider_release_dependencies (provider, dependency_project_id);

-- When this release's dependency list was last written, so "no rows" can be
-- told apart from "never asked". A version that genuinely declares nothing is
-- the common case, and without this marker it would be re-fetched from the
-- provider on every pass. NULL means the release predates this table.
ALTER TABLE provider_releases ADD COLUMN dependencies_synced_at TEXT;
