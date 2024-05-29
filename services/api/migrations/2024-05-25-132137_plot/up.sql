PRAGMA foreign_keys = off;
CREATE TABLE plot (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    title TEXT,
    rank BIGINT NOT NULL,
    lower_value BOOLEAN NOT NULL,
    upper_value BOOLEAN NOT NULL,
    lower_boundary BOOLEAN NOT NULL,
    upper_boundary BOOLEAN NOT NULL,
    x_axis INTEGER NOT NULL,
    window BIGINT NOT NULL,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE
);
CREATE TABLE plot_branch (
    plot_id INTEGER NOT NULL,
    branch_id INTEGER NOT NULL,
    rank BIGINT NOT NULL,
    FOREIGN KEY (plot_id) REFERENCES plot (id) ON DELETE CASCADE,
    FOREIGN KEY (branch_id) REFERENCES branch (id) ON DELETE CASCADE,
    PRIMARY KEY (plot_id, branch_id)
);
CREATE TABLE plot_testbed (
    plot_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    rank BIGINT NOT NULL,
    FOREIGN KEY (plot_id) REFERENCES plot (id) ON DELETE CASCADE,
    FOREIGN KEY (testbed_id) REFERENCES testbed (id) ON DELETE CASCADE,
    PRIMARY KEY (plot_id, testbed_id)
);
CREATE TABLE plot_benchmark (
    plot_id INTEGER NOT NULL,
    benchmark_id INTEGER NOT NULL,
    rank BIGINT NOT NULL,
    FOREIGN KEY (plot_id) REFERENCES plot (id) ON DELETE CASCADE,
    FOREIGN KEY (benchmark_id) REFERENCES benchmark (id) ON DELETE CASCADE,
    PRIMARY KEY (plot_id, benchmark_id)
);
CREATE TABLE plot_measure (
    plot_id INTEGER NOT NULL,
    measure_id INTEGER NOT NULL,
    rank BIGINT NOT NULL,
    FOREIGN KEY (plot_id) REFERENCES plot (id) ON DELETE CASCADE,
    FOREIGN KEY (measure_id) REFERENCES measure (id) ON DELETE CASCADE,
    PRIMARY KEY (plot_id, measure_id)
);
PRAGMA foreign_keys = on;