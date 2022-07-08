-- https://sqlite.org/datatype3.html#storage_classes_and_datatypes
-- https://www.sqlite.org/autoinc.html
CREATE TABLE report (
    id INTEGER PRIMARY KEY NOT NULL,
    project TEXT,
    testbed TEXT,
    start_time DATETIME,
    end_time DATETIME NOT NULL
);