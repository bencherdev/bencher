PRAGMA foreign_keys = off;
CREATE TABLE up_alert (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    boundary_id INTEGER NOT NULL,
    status INTEGER NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (boundary_id) REFERENCES boundary (id)
);
INSERT INTO up_alert(
        id,
        uuid,
        boundary_id,
        status,
        modified
    )
SELECT id,
    uuid,
    (
        SELECT id
        FROM boundary
        WHERE boundary.uuid = alert.uuid
        LIMIT 1
    ), 1,(
        SELECT unixepoch()
    )
FROM alert;
DROP TABLE alert;
ALTER TABLE up_alert
    RENAME TO alert;
PRAGMA foreign_keys = on;