-- Up wrote exactly these metrics from the bound toggles, so a version 0 pin that holds them draws every metric again.
UPDATE plot
SET metrics = NULL
WHERE metrics = CASE
        WHEN lower_value
        AND upper_value THEN '["value","lower_value","upper_value"]'
        WHEN lower_value THEN '["value","lower_value"]'
        WHEN upper_value THEN '["value","upper_value"]'
        ELSE '["value"]'
    END
    AND project_id IN (
        SELECT id
        FROM project
        WHERE bmf_version = 0
    );
