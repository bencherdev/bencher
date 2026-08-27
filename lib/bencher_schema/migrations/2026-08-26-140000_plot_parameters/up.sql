-- plot
-- The grid points a pinned plot draws.
--
-- `parameters` is the filter over grid points: the SQLite JSONB encoding of a JSON
-- array of parameter sets, OR across the array and subset match within each set.
-- It is the same value the perf query's `parameters` takes, so a plot pins the
-- view it was pinned from.
--
-- NULL is match all, which is what every plot that predates this migration draws,
-- so every existing row carries NULL and nothing about it moves. It is declared
-- `BLOB` to match the SQLite representation of the `Jsonb` SQL type, the same as
-- `threshold.parameters` and `parameter."set"`.
ALTER TABLE plot
    ADD COLUMN parameters BLOB;
