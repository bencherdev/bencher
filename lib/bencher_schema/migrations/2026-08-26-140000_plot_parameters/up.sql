-- plot
-- The variants a pinned plot draws.
--
-- `parameters` is the filter over variants: the SQLite JSONB encoding of a JSON
-- array of partial parameters, OR across the array and subset match within each entry.
-- It is the same value the perf query's `parameters` takes, so a plot pins the
-- view it was pinned from.
--
-- NULL is match all, which is what every plot that predates this migration draws,
-- so every existing row carries NULL and nothing about it moves. It is declared
-- `BLOB` to match the SQLite representation of the `Jsonb` SQL type, the same as
-- `threshold.parameters` and `variant.parameters`.
--
-- SQLite drops every index a table owns along with the table, so the plot index is
-- recreated after the swap.
PRAGMA foreign_keys = off;

DROP INDEX IF EXISTS index_plot_project_created;

CREATE TABLE up_plot (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    rank BIGINT NOT NULL,
    title TEXT,
    lower_value BOOLEAN NOT NULL,
    upper_value BOOLEAN NOT NULL,
    lower_boundary BOOLEAN NOT NULL,
    upper_boundary BOOLEAN NOT NULL,
    x_axis INTEGER NOT NULL,
    y_axis INTEGER NOT NULL,
    window BIGINT NOT NULL,
    parameters BLOB,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE
);

INSERT INTO up_plot(
        id,
        uuid,
        project_id,
        rank,
        title,
        lower_value,
        upper_value,
        lower_boundary,
        upper_boundary,
        x_axis,
        y_axis,
        window,
        parameters,
        created,
        modified
    )
SELECT id,
    uuid,
    project_id,
    rank,
    title,
    lower_value,
    upper_value,
    lower_boundary,
    upper_boundary,
    x_axis,
    y_axis,
    window,
    NULL,
    created,
    modified
FROM plot;

DROP TABLE plot;

ALTER TABLE up_plot
    RENAME TO plot;

CREATE INDEX index_plot_project_created ON plot(project_id, created);

PRAGMA foreign_keys = on;
