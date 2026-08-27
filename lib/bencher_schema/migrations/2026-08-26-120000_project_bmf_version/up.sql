-- project
-- The highest BMF payload version the project accepts.
--
-- Every project starts at 0, the version every payload that ingests today is
-- written in, so the gate refuses nothing until a server admin raises it.
ALTER TABLE project
    ADD COLUMN bmf_version INTEGER NOT NULL DEFAULT 0;
