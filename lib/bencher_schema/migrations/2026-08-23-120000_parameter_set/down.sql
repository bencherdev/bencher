-- parameter
-- The reverse of the rename, and just as much a metadata only operation.
ALTER TABLE parameter
    RENAME COLUMN "set" TO parameters;
