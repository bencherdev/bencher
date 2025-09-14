CREATE TABLE sso (
    id INTEGER PRIMARY KEY NOT NULL,
    organization_id INTEGER NOT NULL,
    domain TEXT NOT NULL UNIQUE,
    created BIGINT NOT NULL,
    FOREIGN KEY (organization_id) REFERENCES organization (id) ON DELETE CASCADE
);