CREATE TABLE cluster_flags (
    cluster_id INTEGER NOT NULL,
    key TEXT NOT NULL,
    value TEXT,
    PRIMARY KEY (cluster_id, key),
    FOREIGN KEY (cluster_id) REFERENCES clusters (id) ON DELETE CASCADE
);

INSERT INTO cluster_flags (cluster_id, key, value)
SELECT cluster_id, 'reset-unfinished', NULL
FROM (
    SELECT CAST(substr(id, length('reset-unfinished:') + 1) AS INTEGER) AS cluster_id
    FROM applied_migrations
    WHERE id LIKE 'reset-unfinished:%'
)
WHERE cluster_id IN (SELECT id FROM clusters);

INSERT INTO cluster_flags (cluster_id, key, value)
SELECT cluster_id, 'bundle-java-override', major
FROM (
    SELECT
        CAST(substr(rest, 1, instr(rest, ':') - 1) AS INTEGER) AS cluster_id,
        substr(rest, instr(rest, ':') + 1) AS major,
        max(applied_at)
    FROM (
        SELECT substr(id, length('bundle-java-override:') + 1) AS rest, applied_at
        FROM applied_migrations
        WHERE id LIKE 'bundle-java-override:%'
    )
    GROUP BY cluster_id
)
WHERE cluster_id IN (SELECT id FROM clusters);

DELETE FROM applied_migrations
WHERE id LIKE 'reset-unfinished:%' OR id LIKE 'bundle-java-override:%';
