-- App preferences that aren't credentials (spec "Looks"): one row per key. config.toml stays for secrets.
CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
