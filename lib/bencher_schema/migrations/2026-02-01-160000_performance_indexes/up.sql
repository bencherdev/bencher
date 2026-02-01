-- Index for head_version queries by head_id (used in clone_versions JOIN)
DROP INDEX IF EXISTS index_head_version_head;
CREATE INDEX index_head_version_head ON head_version(head_id);

-- Index for threshold queries by branch_id (used in from_start_point)
DROP INDEX IF EXISTS index_threshold_branch;
CREATE INDEX index_threshold_branch ON threshold(branch_id);
