//! `atharu version` and `atharu update` (D28, D40): no DB, so main runs them before opening one.
//! Update = fetch tags on the checkout the binary was built from and, when a newer release tag
//! exists, `scripts/install.sh --version <tag>`, which builds that tag in a temp worktree.
//! The working tree is never pulled or touched, so uncommitted edits there don't block it.
use crate::{build_info, updater};
use serde_json::{Value, json};
use std::{io::Write, path::Path, process::Command};

const USAGE: &str = "usage:
  atharu version
  atharu update [--app] [--docker]   (ATHARU_REPO overrides the checkout it reads tags from)";

pub use crate::updater::{installed, repo};

/// Same exit codes as `cli::run`: 0 JSON on `out`, 1 refused, 2 usage.
/// `installed` is passed in (see [`installed`]) so tests can play any version.
pub fn run(
    args: &[String],
    repo: &Path,
    installed: Option<&str>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let res = match args.split_first() {
        Some((c, rest)) if c == "version" && rest.is_empty() => Ok(version()),
        Some((c, rest)) if c == "update" => match rest
            .iter()
            .find(|a| !["--app", "--docker"].contains(&a.as_str()))
        {
            Some(bad) => Err((2, format!("update takes no {bad:?}\n{USAGE}"))),
            None => update(repo, installed, rest, err),
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
        "version": build_info::VERSION,
        "tag": build_info::TAG,
        "sha": build_info::SHA,
        "built": build_info::BUILT,
    })
}

fn update(
    repo: &Path,
    installed: Option<&str>,
    flags: &[String],
    err: &mut dyn Write,
) -> Result<Value, (i32, String)> {
    // Same pick as install.sh: newest by semver, pre-releases only when pinned by hand.
    let newest = updater::newest_noting(repo, &mut |why| {
        let _ = writeln!(err, "warning: using local tags; {why}");
    })
    .map_err(|why| (1, why))?
    .ok_or_else(|| {
            (
                1,
                format!(
                    "no release tags in {} yet: scripts/release.sh X.Y.Z cuts one; scripts/install.sh --dev builds the last commit",
                    repo.display()
                ),
            )
        })?;
    let from = match installed {
        Some(v) => {
            if !updater::is_newer(Some(v), &newest) {
                return Ok(json!({ "upToDate": true, "version": format!("v{v}") }));
            }
            format!("v{v}")
        }
        None => {
            let _ = writeln!(
                err,
                "this is a dev build (not from a release tag): installing {newest}"
            );
            "dev".to_string()
        }
    };
    // The script prints to stderr itself, so stdout stays one JSON document.
    let status = Command::new("bash")
        .arg(repo.join("scripts/install.sh"))
        .args(["--version", newest.as_str()])
        .args(flags)
        .status()
        .map_err(|e| (1, format!("can't run scripts/install.sh: {e}")))?;
    if !status.success() {
        return Err((
            1,
            format!(
                "scripts/install.sh --version {newest} failed ({status}); {from} is still installed"
            ),
        ));
    }
    Ok(json!({ "repo": repo.display().to_string(), "from": from, "to": newest }))
}
