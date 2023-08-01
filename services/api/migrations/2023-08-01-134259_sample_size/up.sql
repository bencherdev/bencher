--  min sample size
UPDATE statistic
SET min_sample_size = 2
WHERE min_sample_size < 2;
--  max sample size
UPDATE statistic
SET max_sample_size = 2
WHERE max_sample_size < 2;