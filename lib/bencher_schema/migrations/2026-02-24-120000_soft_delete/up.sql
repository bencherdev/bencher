ALTER TABLE project ADD COLUMN deleted BIGINT;
ALTER TABLE organization ADD COLUMN deleted BIGINT;

CREATE INDEX index_organization_not_deleted ON organization(id) WHERE deleted IS NULL;
CREATE INDEX index_project_not_deleted ON project(id) WHERE deleted IS NULL;
