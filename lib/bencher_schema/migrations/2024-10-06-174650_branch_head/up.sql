DROP INDEX IF EXISTS index_branch_head;
CREATE INDEX index_branch_head ON branch(uuid, project_id, head_id);