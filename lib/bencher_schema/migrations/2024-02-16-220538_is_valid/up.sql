-- organization
UPDATE organization
SET slug = uuid
WHERE LENGTH(slug) > 64;
-- project
UPDATE project
SET slug = uuid
WHERE LENGTH(slug) > 64;
-- branch
UPDATE branch
SET name = uuid
WHERE LENGTH(name) > 256;
UPDATE branch
SET slug = uuid
WHERE LENGTH(slug) > 64;
-- testbed
UPDATE testbed
SET slug = uuid
WHERE LENGTH(slug) > 64;
-- benchmark
UPDATE benchmark
SET slug = uuid
WHERE LENGTH(slug) > 64;
-- measure
UPDATE measure
SET slug = uuid
WHERE LENGTH(slug) > 64;
-- statistic
UPDATE statistic
SET window = null
WHERE window < 1;
-- user
UPDATE user
SET slug = uuid
WHERE LENGTH(slug) > 64;