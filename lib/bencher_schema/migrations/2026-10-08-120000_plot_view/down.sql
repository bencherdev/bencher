-- A plot loses its metrics, hidden and focused lines, and layout, and draws with the defaults again.
PRAGMA foreign_keys = off;

CREATE TABLE down_plot (
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

INSERT INTO down_plot(
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
    parameters,
    created,
    modified
FROM plot;

DROP TABLE plot;

ALTER TABLE down_plot
    RENAME TO plot;

CREATE INDEX index_plot_project_created ON plot(project_id, created);

PRAGMA foreign_keys = on;
