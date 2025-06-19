DELETE FROM user
WHERE slug = '';
DELETE FROM organization
WHERE slug = '';
DELETE FROM project
WHERE slug = '';
DELETE FROM branch
WHERE slug = '';
DELETE FROM testbed
WHERE slug = '';
DELETE FROM benchmark
WHERE slug = '';
DELETE FROM measure
WHERE slug = '';