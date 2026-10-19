CREATE TABLE linked_repository (
    project_id INTEGER PRIMARY KEY NOT NULL,
    installation_id BIGINT NOT NULL,
    repository_id BIGINT NOT NULL,
    full_name TEXT NOT NULL,
    status INTEGER NOT NULL,
    reason INTEGER,
    user_id INTEGER NOT NULL,
    linked BIGINT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES user (id)
);
CREATE INDEX index_linked_repository_user ON linked_repository(user_id);
-- A webhook delivery names an installation and its repositories, never a project.
CREATE INDEX index_linked_repository_installation ON linked_repository(installation_id);
CREATE INDEX index_linked_repository_repository ON linked_repository(repository_id);
