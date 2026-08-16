-- Baseline schema. Confirms migration infrastructure and records the baseline.
CREATE TABLE schema_meta (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    note TEXT NOT NULL
);

INSERT INTO schema_meta (note) VALUES ('verdant-backend baseline');