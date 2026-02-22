PRAGMA foreign_keys = off;

CREATE TABLE up_testbed (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    spec_id INTEGER,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    archived BIGINT,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (spec_id) REFERENCES spec (id) ON DELETE SET NULL,
    UNIQUE(project_id, name),
    UNIQUE(project_id, slug)
);

INSERT INTO up_testbed(
        id,
        uuid,
        project_id,
        name,
        slug,
        spec_id,
        created,
        modified,
        archived
    )
SELECT id,
    uuid,
    project_id,
    name,
    slug,
    null,
    created,
    modified,
    archived
FROM testbed;

DROP TABLE testbed;

ALTER TABLE up_testbed
    RENAME TO testbed;

CREATE INDEX index_testbed_project_created ON testbed(project_id, created);

-- report: add spec_id column
CREATE TABLE up_report (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    user_id INTEGER,
    project_id INTEGER NOT NULL,
    head_id INTEGER NOT NULL,
    version_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    spec_id INTEGER,
    adapter INTEGER NOT NULL,
    start_time BIGINT NOT NULL,
    end_time BIGINT NOT NULL,
    created BIGINT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES user (id),
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (head_id) REFERENCES head (id),
    FOREIGN KEY (version_id) REFERENCES version (id),
    FOREIGN KEY (testbed_id) REFERENCES testbed (id),
    FOREIGN KEY (spec_id) REFERENCES spec (id) ON DELETE SET NULL
);

INSERT INTO up_report(
        id,
        uuid,
        user_id,
        project_id,
        head_id,
        version_id,
        testbed_id,
        spec_id,
        adapter,
        start_time,
        end_time,
        created
    )
SELECT r.id,
    r.uuid,
    r.user_id,
    r.project_id,
    r.head_id,
    r.version_id,
    r.testbed_id,
    (SELECT j.spec_id FROM job j WHERE j.report_id = r.id LIMIT 1) AS spec_id,
    r.adapter,
    r.start_time,
    r.end_time,
    r.created
FROM report r;

DROP TABLE report;

ALTER TABLE up_report
    RENAME TO report;

CREATE INDEX index_report_testbed_end_time ON report(testbed_id, end_time);
CREATE INDEX index_report_version ON report(version_id, end_time);
CREATE INDEX index_report_project_end_time ON report(project_id, end_time);
CREATE INDEX index_report_project_created ON report(project_id, created);
CREATE INDEX index_report_version_testbed ON report(version_id, testbed_id);
CREATE INDEX index_report_spec ON report(spec_id);
CREATE INDEX index_testbed_spec ON testbed(spec_id);

PRAGMA foreign_keys = on;
