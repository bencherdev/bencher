-- https://sqlite.org/datatype3.html#storage_classes_and_datatypes
-- https://www.sqlite.org/autoinc.html
CREATE TABLE user (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    email TEXT NOT NULL UNIQUE
);
CREATE TABLE project (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    owner_id INTEGER NOT NULL,
    owner_default BOOLEAN NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    url TEXT,
    FOREIGN KEY (owner_id) REFERENCES user (id),
    UNIQUE(owner_id, name)
);
CREATE TABLE testbed (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    os_name TEXT,
    os_version TEXT,
    cpu TEXT,
    ram TEXT,
    disk TEXT
);
CREATE TABLE adapter (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL UNIQUE
);
INSERT INTO adapter (uuid, name)
VALUES("55e0af45-48df-420e-a0eb-134ea1e806db", "json"),
    ("6a4a11f9-682c-43c2-9385-fde30a14350b", "rust");
CREATE TABLE report (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project TEXT,
    --  project_id INTEGER NOT NULL,
    testbed_id INTEGER,
    adapter_id INTEGER NOT NULL,
    start_time DATETIME NOT NULL,
    end_time DATETIME NOT NULL,
    -- FOREIGN KEY (project_id) REFERENCES project (id),
    FOREIGN KEY (testbed_id) REFERENCES testbed (id),
    FOREIGN KEY (adapter_id) REFERENCES adapter (id)
);