//! `atharu version` and `atharu update` (D28, D40): no DB, so main runs them before opening one.
//! Update = fetch tags on the checkout the binary was built from and, when a newer release tag
//! exists, `scripts/install.sh --version <tag>`, which builds that tag in a temp worktree.
//! The working tree is never pulled or touched, so uncommitted edits there don't block it.
use crate::build_info;
use serde_json::{Value, json};
use std::{io::Write, path::Path, process::Command};

const USAGE: &str = "usage:
  atharu version
  atharu update [--app] [--docker]   (ATHARU_REPO overrides the checkout it reads tags from)";

/// The checkout to read tags from: `ATHARU_REPO` at run time (e.g. the clone moved), else the one baked in
/// by install.sh (it builds from a temp worktree, so CARGO_MANIFEST_DIR is gone by then), else the source dir.
pub fn repo() -> String {
    std::env::var("ATHARU_REPO").unwrap_or_else(|_| {
        option_env!("ATHARU_REPO")
            .unwrap_or(env!("CARGO_MANIFEST_DIR"))
            .to_string()
    })
}

/// This binary's release version, `None` for a dev build (not built from a release tag).
pub fn installed() -> Option<&'static str> {
    build_info::TAG.map(|_| build_info::VERSION)
}

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

/// "1.2.3" or "v1.2.3-rc1" → ((1, 2, 3), is a final release); a final release outranks its pre-releases.
fn semver(s: &str) -> Option<((u64, u64, u64), bool)> {
    let s = s.strip_prefix('v').unwrap_or(s);
    let (core, pre) = s.split_once('-').map_or((s, false), |(c, _)| (c, true));
    let mut n = core.split('.').map(|p| p.parse::<u64>().ok());
    let v = (n.next()??, n.next()??, n.next()??);
    n.next().is_none().then_some((v, !pre))
}

fn update(
    repo: &Path,
    installed: Option<&str>,
    flags: &[String],
    err: &mut dyn Write,
) -> Result<Value, (i32, String)> {
    if let Err((_, why)) = git(repo, &["fetch", "--tags", "--quiet"]) {
        let _ = writeln!(err, "warning: using local tags; {why}");
    }
    // Same pick as install.sh: newest by semver, pre-releases only when pinned by hand.
    let tags = git(repo, &["tag", "--list", "v[0-9]*", "--sort=-v:refname"])?;
    let newest = tags
        .lines()
        .find(|t| semver(t).is_some_and(|(_, release)| release))
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
            if semver(v) >= semver(newest) {
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
        .args(["--version", newest])
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

#[cfg(test)]
mod tests {
    use super::semver;

    #[test]
    fn semver_orders_numerically_and_final_after_pre() {
        assert!(semver("v0.10.0") > semver("v0.9.9"));
        assert!(semver("0.3.0") > semver("v0.3.0-rc1"));
        assert_eq!(semver("v1.2"), None);
        assert_eq!(semver("v1.2.3.4"), None);
    }
}
