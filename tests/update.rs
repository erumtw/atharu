//! `atharu version` / `atharu update` (T60): parsing and the refusals that must happen before any pull;
//! `atharu help` and scripts/install.sh building the last commit (T76).
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

fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(args)
        .output()
        .unwrap();
    assert!(o.status.success(), "git {args:?}");
    String::from_utf8(o.stdout).unwrap().trim().to_string()
}

/// A one-commit repo with no upstream, in a fresh temp dir.
fn temp_repo(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("atharu-update-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f.txt"), "one").unwrap();
    git(&dir, &["init", "-q"]);
    git(&dir, &["add", "f.txt"]);
    git(&dir, &["commit", "-qm", "one"]);
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

/// The real binary: help and argument errors are decided in main/config, before any DB.
fn bin(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_atharu"))
        .args(args)
        .env("ATHARU_DB", "/nonexistent/never-opened.db")
        .output()
        .unwrap();
    (
        o.status.code().unwrap(),
        String::from_utf8(o.stdout).unwrap(),
        String::from_utf8(o.stderr).unwrap(),
    )
}

const ALL: [&str; 11] = [
    "log", "epic", "note", "evidence", "today", "show", "tags", "dump", "version", "update", "help",
];

#[test]
fn help_prints_every_command_and_exits_0() {
    for args in [
        &["help"][..],
        &["--help"],
        &["-h"],
        &["--db", "x.db", "-h"],
        &["-h", "--db", "x.db"],
    ] {
        let (code, out, err) = bin(args);
        assert_eq!(code, 0, "{args:?}: {err}");
        assert!(err.is_empty(), "{args:?}: {err}");
        assert!(out.contains("serve http://127.0.0.1:7070"), "{out}");
        for c in ALL {
            assert!(out.contains(&format!("atharu {c}")), "{args:?} lacks {c}");
        }
    }
}

#[test]
fn unknown_argument_exits_2_with_the_help_table() {
    let (code, out, err) = bin(&["--nope"]);
    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert!(err.contains("unknown argument \"--nope\""), "{err}");
    assert!(err.contains(atharu::cli::HELP), "{err}");
}

#[test]
fn install_builds_the_last_commit_not_the_dirty_tree() {
    let repo = temp_repo("install");
    std::fs::create_dir_all(repo.join("scripts")).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/install.sh"),
        repo.join("scripts/install.sh"),
    )
    .unwrap();
    git(&repo, &["add", "scripts"]);
    git(&repo, &["commit", "-qm", "two"]);
    let sha = git(&repo, &["rev-parse", "--short", "HEAD"]);
    std::fs::write(repo.join("f.txt"), "edited").unwrap();

    // A fake cargo stands in for the release build (minutes): its "binary" reports the tree,
    // sha and env it was built from. Never touches ~/.cargo: CARGO_INSTALL_ROOT is temp.
    let tmp = repo.with_extension("tools");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(
        tmp.join("cargo"),
        r#"#!/bin/sh
while [ "$1" != --path ]; do shift; done
mkdir -p "$CARGO_INSTALL_ROOT/bin"
printf '#!/bin/sh\necho "%s %s %s %s"\n' "$(git -C "$2" rev-parse --short HEAD)" "$(cat "$2/f.txt")" \
  "$ATHARU_REPO" "$CARGO_TARGET_DIR" > "$CARGO_INSTALL_ROOT/bin/atharu"
chmod +x "$CARGO_INSTALL_ROOT/bin/atharu"
"#,
    )
    .unwrap();
    Command::new("chmod")
        .arg("+x")
        .arg(tmp.join("cargo"))
        .status()
        .unwrap();
    let path = format!("{}:{}", tmp.display(), std::env::var("PATH").unwrap());
    let o = Command::new("bash")
        .arg(repo.join("scripts/install.sh"))
        .env("PATH", path)
        .env("CARGO_INSTALL_ROOT", tmp.join("root"))
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("ATHARU_REPO")
        .output()
        .unwrap();
    let err = String::from_utf8(o.stderr).unwrap();
    assert!(o.status.success(), "{err}");
    assert!(o.stdout.is_empty());
    assert!(
        err.contains(&format!(
            "note: building {sha}; uncommitted changes are not included"
        )),
        "{err}"
    );
    let built = Command::new(tmp.join("root/bin/atharu")).output().unwrap();
    assert_eq!(
        String::from_utf8(built.stdout).unwrap().trim(),
        format!("{sha} one {r} {r}/target", r = repo.display())
    );
    // The edit survives and the temp worktree is gone.
    assert_eq!(
        std::fs::read_to_string(repo.join("f.txt")).unwrap(),
        "edited"
    );
    assert_eq!(git(&repo, &["worktree", "list"]).lines().count(), 1);
    let _ = std::fs::remove_dir_all(&tmp);
    let _ = std::fs::remove_dir_all(repo);
}
