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
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    description TEXT,
    url TEXT,
    public BOOLEAN NOT NULL,
    FOREIGN KEY (owner_id) REFERENCES user (id)
);
CREATE TABLE branch (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id),
    UNIQUE(project_id, slug)
);
CREATE TABLE version (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    branch_id INTEGER NOT NULL,
    number INTEGER NOT NULL,
    hash TEXT,
    FOREIGN KEY (branch_id) REFERENCES branch (id),
    UNIQUE(branch_id, number)
);
CREATE TABLE testbed (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    os_name TEXT,
    os_version TEXT,
    runtime_name TEXT,
    runtime_version TEXT,
    cpu TEXT,
    ram TEXT,
    disk TEXT,
    FOREIGN KEY (project_id) REFERENCES project (id),
    UNIQUE(project_id, slug)
);
CREATE TABLE benchmark (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id),
    UNIQUE(project_id, name)
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
    user_id INTEGER NOT NULL,
    version_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    adapter_id INTEGER NOT NULL,
    start_time BIGINT NOT NULL,
    end_time BIGINT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES user (id),
    FOREIGN KEY (version_id) REFERENCES version (id),
    FOREIGN KEY (testbed_id) REFERENCES testbed (id),
    FOREIGN KEY (adapter_id) REFERENCES adapter (id)
);
CREATE TABLE latency (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    lower_variance BIGINT NOT NULL,
    upper_variance BIGINT NOT NULL,
    duration BIGINT NOT NULL
);
CREATE TABLE throughput (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    lower_events DOUBLE NOT NULL,
    upper_events DOUBLE NOT NULL,
    unit_time BIGINT NOT NULL
);
CREATE TABLE min_max_avg (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    min DOUBLE NOT NULL,
    max DOUBLE NOT NULL,
    avg DOUBLE NOT NULL
);
CREATE TABLE perf (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    report_id INTEGER NOT NULL,
    benchmark_id INTEGER NOT NULL,
    -- at least one should not be null
    latency_id INTEGER,
    throughput_id INTEGER,
    compute_id INTEGER,
    memory_id INTEGER,
    storage_id INTEGER,
    --
    FOREIGN KEY (report_id) REFERENCES report (id),
    FOREIGN KEY (benchmark_id) REFERENCES benchmark (id),
    FOREIGN KEY (latency_id) REFERENCES latency (id),
    FOREIGN KEY (throughput_id) REFERENCES throughput (id),
    FOREIGN KEY (compute_id) REFERENCES min_max_avg (id),
    FOREIGN KEY (memory_id) REFERENCES min_max_avg (id),
    FOREIGN KEY (storage_id) REFERENCES min_max_avg (id),
    UNIQUE(report_id, benchmark_id)
);
-- https://en.wikipedia.org/wiki/Standard_score
CREATE TABLE z_score (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    -- sample size, if null use the entire population
    sample_size INTEGER,
    -- min allowed negative standard deviation, if null don't compare
    min_deviation REAL,
    -- max allowed positive standard deviation, if null don't compare
    max_deviation REAL
);
-- https://en.wikipedia.org/wiki/Student's_t-test
CREATE TABLE t_test (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    -- sample size, for Two-sample equal variance (homoscedastic) and (heteroscedastic)
    -- Paired if null
    sample_size INTEGER,
    -- one-tailed left (false), one-tailed right (true), two-tailed (null)
    tail BOOLEAN,
    -- confidence interval
    confidence_interval REAL NOT NULL,
);
CREATE TABLE threshold (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    branch_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    -- at least one should not be null
    z_score_id INTEGER,
    t_test_id INTEGER,
    --
    FOREIGN KEY (branch_id) REFERENCES branch (id),
    FOREIGN KEY (testbed_id) REFERENCES testbed (id),
    FOREIGN KEY (z_score_id) REFERENCES z_score (id),
    FOREIGN KEY (t_test_id) REFERENCES t_test (id),
    UNIQUE(branch_id, testbed_id)
);
CREATE TABLE alert (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    threshold_id INTEGER NOT NULL,
    perf_id INTEGER NOT NULL,
    -- only one should not be null
    z_score_id INTEGER,
    t_test_id INTEGER,
    --
    FOREIGN KEY (threshold_id) REFERENCES threshold (id),
    FOREIGN KEY (perf_id) REFERENCES perf (id),
    FOREIGN KEY (z_score_id) REFERENCES z_score (id),
    FOREIGN KEY (t_test_id) REFERENCES t_test (id),
    UNIQUE(z_score_id, perf_id)
);