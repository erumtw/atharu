//! `atharu version` and `atharu update` (D28): no DB, so main runs them before opening one.
//! Update = `git pull --ff-only` on the checkout the binary was built from, then
//! `scripts/install.sh`, which rebuilds and reinstalls from source.
use serde_json::{Value, json};
use std::{io::Write, path::Path, process::Command};

const USAGE: &str = "usage:
  atharu version
  atharu update [--app] [--docker]   (ATHARU_REPO overrides the checkout it pulls)";

/// The checkout this binary was built from; `ATHARU_REPO` wins (e.g. the clone moved).
pub fn repo() -> String {
    std::env::var("ATHARU_REPO").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_string())
}

/// Same exit codes as `cli::run`: 0 JSON on `out`, 1 refused, 2 usage.
pub fn run(args: &[String], repo: &Path, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let res = match args.split_first() {
        Some((c, rest)) if c == "version" && rest.is_empty() => Ok(version()),
        Some((c, rest)) if c == "update" => match rest
            .iter()
            .find(|a| !["--app", "--docker"].contains(&a.as_str()))
        {
            Some(bad) => Err((2, format!("update takes no {bad:?}\n{USAGE}"))),
            None => update(repo, rest),
        },
        _ => Err((2, USAGE.to_string())),
    };
    match res {
        Ok(v) => {
            let _ = writeln!(out, "{v}");
            0
        }
        Err((code, msg)) => {
            let _ = writeln!(err, "{msg}");
            code
        }
    }
}

fn version() -> Value {
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "sha": option_env!("ATHARU_GIT_SHA").unwrap_or("unknown"),
        "built": option_env!("ATHARU_BUILT").unwrap_or("unknown"),
    })
}

/// `git -C repo …` → trimmed stdout, or git's own stderr as the refusal.
fn git(repo: &Path, args: &[&str]) -> Result<String, (i32, String)> {
    let o = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| (1, format!("can't run git: {e}")))?;
    if !o.status.success() {
        let why = String::from_utf8_lossy(&o.stderr).trim().to_string();
        return Err((
            1,
            format!("git {} failed in {}: {why}", args[0], repo.display()),
        ));
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn update(repo: &Path, flags: &[String]) -> Result<Value, (i32, String)> {
    // Untracked files don't block a fast-forward; only edits to tracked files would be lost or merged.
    let dirty = git(repo, &["status", "--porcelain", "--untracked-files=no"])?;
    if !dirty.is_empty() {
        return Err((
            1,
            format!(
                "{} has uncommitted changes; commit or stash them, then run atharu update again:\n{dirty}",
                repo.display()
            ),
        ));
    }
    let from = git(repo, &["rev-parse", "--short", "HEAD"])?;
    git(repo, &["pull", "--ff-only", "--quiet"]).map_err(|(c, m)| {
        (
            c,
            format!("{m}\nupdate only fast-forwards: rebase or merge local commits by hand"),
        )
    })?;
    let to = git(repo, &["rev-parse", "--short", "HEAD"])?;
    // The script prints to stderr itself, so stdout stays one JSON document.
    let status = Command::new("bash")
        .arg(repo.join("scripts/install.sh"))
        .args(flags)
        .status()
        .map_err(|e| (1, format!("can't run scripts/install.sh: {e}")))?;
    if !status.success() {
        return Err((
            1,
            format!("scripts/install.sh failed ({status}); pulled {from} → {to}"),
        ));
    }
    Ok(json!({ "repo": repo.display().to_string(), "from": from, "to": to }))
}
