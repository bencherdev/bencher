ALTER TABLE project ADD COLUMN deleted BIGINT;
ALTER TABLE organization ADD COLUMN deleted BIGINT;

CREATE INDEX index_organization_deleted ON organization(deleted);
CREATE INDEX index_project_deleted ON project(deleted);
