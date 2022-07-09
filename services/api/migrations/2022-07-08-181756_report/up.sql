-- https://sqlite.org/datatype3.html#storage_classes_and_datatypes
-- https://www.sqlite.org/autoinc.html
CREATE TABLE adapter (
    id INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE
);
INSERT INTO adapter (name)
VALUES("json"),
    ("rust");
CREATE TABLE report (
    id INTEGER PRIMARY KEY NOT NULL,
    project TEXT,
    testbed TEXT,
    adapter_id INTEGER NOT NULL,
    start_time DATETIME NOT NULL,
    end_time DATETIME NOT NULL,
    FOREIGN KEY (adapter_id) REFERENCES adapter (id)
);