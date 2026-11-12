//! `atharu version` / `atharu update` (T60): parsing and the refusals that must happen before any pull.
use std::{path::Path, process::Command};

fn run(args: &[&str], repo: &Path) -> (i32, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = atharu::cli::update::run(&args, repo, &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

/// A one-commit repo with no upstream, in a fresh temp dir.
fn temp_repo(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("atharu-update-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f.txt"), "one").unwrap();
    for a in [
        &["init", "-q"][..],
        &["add", "f.txt"],
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "one",
        ],
    ] {
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(a)
                .status()
                .unwrap()
                .success()
        );
    }
    dir
}

#[test]
fn version_prints_semver_and_sha() {
    let (code, out, _) = run(&["version"], Path::new("."));
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
    // Tests run from the git checkout, so build.rs found a sha.
    assert_ne!(v["sha"], "unknown");
    assert!(v["built"].as_str().unwrap().len() == 10);
}

#[test]
fn usage_errors_exit_2() {
    for args in [
        &["version", "x"][..],
        &["update", "--force"],
        &["update", "app"],
    ] {
        let (code, out, err) = run(args, Path::new("/nonexistent"));
        assert_eq!(code, 2, "{args:?}");
        assert!(out.is_empty() && err.contains("usage"), "{args:?}: {err}");
    }
}

#[test]
fn dirty_tree_is_refused_before_pulling() {
    let repo = temp_repo("dirty");
    std::fs::write(repo.join("f.txt"), "edited").unwrap();
    let (code, out, err) = run(&["update"], &repo);
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(
        err.contains("uncommitted changes") && err.contains("f.txt"),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(repo);
}

#[test]
fn failed_pull_is_refused_with_gits_reason() {
    // Clean, but no upstream: the pull fails and the script never runs.
    let repo = temp_repo("nopull");
    std::fs::write(repo.join("untracked.txt"), "x").unwrap();
    let (code, out, err) = run(&["update"], &repo);
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(
        err.contains("git pull failed") && err.contains("fast-forwards"),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(repo);
}
