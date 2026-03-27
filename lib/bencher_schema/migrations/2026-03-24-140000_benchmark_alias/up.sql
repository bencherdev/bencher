CREATE TABLE benchmark_alias (
    id INTEGER PRIMARY KEY NOT NULL,
    project_id INTEGER NOT NULL,
    benchmark_id INTEGER NOT NULL,
    alias TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (benchmark_id) REFERENCES benchmark (id) ON DELETE CASCADE,
    UNIQUE (project_id, alias)
);
CREATE INDEX index_benchmark_alias_benchmark ON benchmark_alias (benchmark_id);
