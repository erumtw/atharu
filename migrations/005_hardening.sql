-- One rating per topic per day (the page already behaves so; now the DB enforces it).
-- Older duplicates go first, keeping the latest row of each (topic, day), or the index can't be built.
DELETE FROM depth_ratings
 WHERE rowid NOT IN (SELECT MAX(rowid) FROM depth_ratings GROUP BY topic, day);
CREATE UNIQUE INDEX depth_ratings_topic_day ON depth_ratings(topic, day);
