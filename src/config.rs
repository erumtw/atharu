use std::path::{Path, PathBuf};

pub enum Command {
    Serve,
    Dump,
    /// Agent CLI: the subcommand and everything after it (see `cli`).
    Cli(Vec<String>),
}

pub struct Config {
    pub host: String,
    pub port: u16,
    pub db: PathBuf,
}

impl Config {
    /// Env (`ATHARU_HOST` / `ATHARU_PORT` / `ATHARU_DB`) sets defaults; `--db PATH` wins over env.
    pub fn from_env_and_args(
        args: impl Iterator<Item = String>,
    ) -> Result<(Command, Config), String> {
        let env = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
        let mut cfg = Config {
            host: env("ATHARU_HOST", "127.0.0.1"),
            port: env("ATHARU_PORT", "7070")
                .parse()
                .map_err(|_| "ATHARU_PORT must be a port number".to_string())?,
            db: PathBuf::from(env("ATHARU_DB", "data/atharu.db")),
        };
        let mut cmd = Command::Serve;
        let mut args = args;
        while let Some(a) = args.next() {
            match a.as_str() {
                c if crate::cli::COMMANDS.contains(&c) => {
                    // The CLI parses its own flags; `--db` must come before the subcommand.
                    cmd = Command::Cli(std::iter::once(a).chain(args).collect());
                    break;
                }
                "dump" => cmd = Command::Dump,
                "-db" | "--db" => cfg.db = args.next().ok_or("--db needs a path")?.into(),
                other => {
                    return Err(format!(
                        "unknown argument {other:?}; usage: atharu [--db PATH] [dump | log | epic | today | show]"
                    ));
                }
            }
        }
        Ok((cmd, cfg))
    }

    /// Folder holding the DB; dump.sql and config.toml live next to it.
    pub fn data_dir(&self) -> &Path {
        self.db
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
    }
}
