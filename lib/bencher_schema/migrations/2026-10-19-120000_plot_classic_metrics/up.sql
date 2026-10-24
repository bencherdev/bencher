-- A version 0 pin with no metrics was made by the classic console, which drew `value` and each bound it turned on.
UPDATE plot
SET metrics = CASE
        WHEN lower_value
        AND upper_value THEN '["value","lower_value","upper_value"]'
        WHEN lower_value THEN '["value","lower_value"]'
        WHEN upper_value THEN '["value","upper_value"]'
        ELSE '["value"]'
    END
WHERE metrics IS NULL
    AND project_id IN (
        SELECT id
        FROM project
        WHERE bmf_version = 0
    );
