use std::path::{Path, PathBuf};

pub enum Command {
    Serve,
    Dump,
    /// `help` / `-h` / `--help`: print `cli::HELP`, no DB.
    Help,
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
                "help" | "-h" | "--help" => return Ok((Command::Help, cfg)),
                "-db" | "--db" => cfg.db = args.next().ok_or("--db needs a path")?.into(),
                other => {
                    return Err(format!("unknown argument {other:?}\n{}", crate::cli::HELP));
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

/// `data/config.toml`: optional integrations. Every key is optional; a missing file = all off.
/// Values are credentials (secret iCal address, Jira API token): never `Debug`, never logged.
#[derive(Default, serde::Deserialize)]
pub struct FileConfig {
    /// Google Calendar → Settings → your calendar → "Secret address in iCal format".
    pub ical_url: Option<String>,
    /// Jira Cloud, e.g. `https://acme.atlassian.net`. With `jira_email` + `jira_token` (API token).
    pub jira_base_url: Option<String>,
    pub jira_email: Option<String>,
    pub jira_token: Option<String>,
}

/// All three Jira keys, or none: a half-filled config is "off", not an error.
pub struct Jira<'a> {
    pub base_url: &'a str,
    pub email: &'a str,
    pub token: &'a str,
}

impl FileConfig {
    /// A broken file must not stop the server: say which file and why, carry on with everything off.
    /// `message()`, not Display: Display quotes the offending source line, which may hold the URL.
    pub fn load(path: &Path) -> FileConfig {
        let Ok(text) = std::fs::read_to_string(path) else {
            return FileConfig::default();
        };
        match toml::from_str::<FileConfig>(&text) {
            Ok(mut c) => {
                for v in [
                    &mut c.ical_url,
                    &mut c.jira_base_url,
                    &mut c.jira_email,
                    &mut c.jira_token,
                ] {
                    *v = v.take().filter(|u| !u.trim().is_empty());
                }
                c
            }
            Err(e) => {
                eprintln!("{}: ignored, {}", path.display(), e.message());
                FileConfig::default()
            }
        }
    }
}

impl FileConfig {
    pub fn jira(&self) -> Option<Jira<'_>> {
        Some(Jira {
            base_url: self.jira_base_url.as_deref()?.trim_end_matches('/'),
            email: self.jira_email.as_deref()?,
            token: self.jira_token.as_deref()?,
        })
    }
}

/// Env var the Mac app sets to its pointer file's path; Settings › About offers "Change" only when it is set.
pub const DB_POINTER_ENV: &str = "ATHARU_DB_POINTER";
/// The pointer's name in the Mac app's data dir: one line, the DB path Settings › About picked.
pub const DB_POINTER: &str = "db-path";

/// The Mac app's DB (D52): `ATHARU_DB` > the `db-path` pointer Settings writes > the shared
/// `~/atharu/data/atharu.db` when it exists (the file the browser, CLI and agents use) > the app's own folder
/// (pre-T85 behaviour, so a fresh Mac without the repo still starts).
pub fn resolve_db(env: Option<PathBuf>, app_dir: &Path, home: Option<&Path>) -> PathBuf {
    if let Some(p) = env.filter(|p| !p.as_os_str().is_empty()) {
        return p;
    }
    if let Ok(s) = std::fs::read_to_string(app_dir.join(DB_POINTER))
        && !s.trim().is_empty()
    {
        return s.trim().into();
    }
    match home.map(|h| h.join("atharu/data/atharu.db")) {
        Some(shared) if shared.is_file() => shared,
        _ => app_dir.join("atharu.db"),
    }
}

/// A DB path typed into Settings: absolute (a leading `~` expands), ends in `.db`, and is an existing file or
/// sits in an existing folder (SQLite creates the file, not its folders). `Err` is the copy the form shows.
pub fn check_db_path(input: &str, home: Option<&Path>) -> Result<PathBuf, String> {
    let s = input.trim();
    let p = match (s.strip_prefix('~'), home) {
        (Some(rest), Some(h)) if rest.is_empty() || rest.starts_with('/') => {
            h.join(rest.trim_start_matches('/'))
        }
        _ => PathBuf::from(s),
    };
    if !p.is_absolute() {
        return Err("Use a full path, starting with / or ~/.".into());
    }
    if p.extension().is_none_or(|e| e != "db") {
        return Err("Pick a file ending in .db.".into());
    }
    if p.exists() {
        if !p.is_file() {
            return Err("That's a folder, not a .db file.".into());
        }
    } else if let Some(dir) = p.parent().filter(|d| !d.is_dir()) {
        return Err(format!("No folder {}. Create it first.", dir.display()));
    }
    Ok(p)
}

/// Point the Mac app at `db` from its next start. Temp file + rename: a crash mid-write can't leave a half
/// path that the next start would open as a new, empty DB.
pub fn write_db_pointer(pointer: &Path, db: &Path) -> std::io::Result<()> {
    if let Some(dir) = pointer.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = pointer.with_extension("tmp");
    std::fs::write(&tmp, db.as_os_str().as_encoded_bytes())?;
    std::fs::rename(tmp, pointer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("atharu-dbpath-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn db_precedence() {
        let (app, home) = (tmp("app"), tmp("home"));
        let own = app.join("atharu.db");
        assert_eq!(resolve_db(None, &app, Some(&home)), own, "nothing set");
        assert_eq!(resolve_db(None, &app, None), own, "no HOME");

        let shared = home.join("atharu/data/atharu.db");
        std::fs::create_dir_all(shared.parent().unwrap()).unwrap();
        std::fs::write(&shared, "").unwrap();
        assert_eq!(
            resolve_db(None, &app, Some(&home)),
            shared,
            "shared beats own"
        );

        let picked = home.join("other.db");
        write_db_pointer(&app.join("db-path"), &picked).unwrap();
        assert_eq!(
            resolve_db(None, &app, Some(&home)),
            picked,
            "pointer beats shared"
        );
        assert!(!app.join("db-path.tmp").exists());

        let env = PathBuf::from("/env/x.db");
        assert_eq!(
            resolve_db(Some(env.clone()), &app, Some(&home)),
            env,
            "env wins"
        );
        assert_eq!(
            resolve_db(Some("".into()), &app, Some(&home)),
            picked,
            "empty env ignored"
        );
    }

    #[test]
    fn db_path_checks() {
        let home = tmp("check");
        std::fs::write(home.join("a.db"), "").unwrap();
        std::fs::create_dir_all(home.join("dir.db")).unwrap();
        let ok = |s: &str| check_db_path(s, Some(&home));
        assert_eq!(
            ok("~/a.db").unwrap(),
            home.join("a.db"),
            "existing, ~ expanded"
        );
        assert_eq!(
            ok(" ~/new.db ").unwrap(),
            home.join("new.db"),
            "new file, folder exists"
        );
        assert_eq!(
            ok("rel/a.db").unwrap_err(),
            "Use a full path, starting with / or ~/."
        );
        assert_eq!(
            ok("~bob/a.db").unwrap_err(),
            "Use a full path, starting with / or ~/."
        );
        assert_eq!(ok("~/a.txt").unwrap_err(), "Pick a file ending in .db.");
        assert_eq!(
            ok("~/dir.db").unwrap_err(),
            "That's a folder, not a .db file."
        );
        assert!(ok("~/nope/x.db").unwrap_err().starts_with("No folder "));
    }
}
