//! `atharu version` / `atharu update` (T60, T77): parsing, tag picking, up-to-date;
//! `atharu help` (T76); scripts/install.sh building a release tag and scripts/release.sh cutting one
//! (T77, D40, D41), all in temp repos: no real tag, push or gh call ever happens.
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

fn run(args: &[&str], repo: &Path, installed: Option<&str>) -> (i32, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = atharu::cli::update::run(&args, repo, installed, &mut out, &mut err);
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

/// Writes `f.txt` and commits it with `msg`; returns the short sha.
fn commit(dir: &Path, text: &str, msg: &str) -> String {
    std::fs::write(dir.join("f.txt"), text).unwrap();
    git(dir, &["add", "f.txt"]);
    git(dir, &["commit", "-qm", msg]);
    git(dir, &["rev-parse", "--short", "HEAD"])
}

/// A one-commit repo on `main` with no remote, in a fresh temp dir.
fn temp_repo(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("atharu-update-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    commit(&dir, "one", "init");
    dir
}

/// Commits the repo's real `scripts/<name>` into `repo`.
fn add_script(repo: &Path, name: &str) {
    std::fs::create_dir_all(repo.join("scripts")).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("scripts")
            .join(name),
        repo.join("scripts").join(name),
    )
    .unwrap();
    git(repo, &["add", "scripts"]);
    git(repo, &["commit", "-qm", &format!("chore: add {name}")]);
}

fn chmod_x(p: &Path) {
    Command::new("chmod").arg("+x").arg(p).status().unwrap();
}

/// Runs `bash scripts/<name> args` in `repo` with no tty, `tools` first on PATH and git's global config
/// blanked (no signing prompts, a fixed author). → (exit code, stdout, stderr).
fn script(repo: &Path, name: &str, args: &[&str], tools: &Path) -> (i32, String, String) {
    let path = format!("{}:{}", tools.display(), std::env::var("PATH").unwrap());
    let o = Command::new("bash")
        .arg(repo.join("scripts").join(name))
        .args(args)
        .current_dir(repo)
        .stdin(Stdio::null())
        .env("PATH", path)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .env("CARGO_INSTALL_ROOT", tools.join("root"))
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("ATHARU_REPO")
        .env_remove("ATHARU_COAUTHOR")
        .output()
        .unwrap();
    (
        o.status.code().unwrap(),
        String::from_utf8(o.stdout).unwrap(),
        String::from_utf8(o.stderr).unwrap(),
    )
}

#[test]
fn version_prints_semver_tag_and_sha() {
    let (code, out, _) = run(&["version"], Path::new("."), None);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
    // Null in a dev build, the release tag in an installed one: the key is always there.
    assert!(v["tag"].is_null() || v["tag"].as_str().unwrap().starts_with('v'));
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
        &["update", "--version", "v0.1.0"],
    ] {
        let (code, out, err) = run(args, Path::new("/nonexistent"), None);
        assert_eq!(code, 2, "{args:?}");
        assert!(out.is_empty() && err.contains("usage"), "{args:?}: {err}");
    }
}

#[test]
fn update_without_release_tags_exits_1_with_a_hint() {
    let repo = temp_repo("notags");
    // A pre-release alone is no release.
    git(&repo, &["tag", "v0.1.0-rc1"]);
    let (code, out, err) = run(&["update"], &repo, Some("0.1.0"));
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(
        err.contains("no release tags") && err.contains("release.sh") && err.contains("--dev"),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(repo);
}

/// A repo whose install.sh only records its arguments: update's job is picking the tag.
fn update_repo(name: &str) -> PathBuf {
    let repo = temp_repo(name);
    std::fs::create_dir_all(repo.join("scripts")).unwrap();
    std::fs::write(
        repo.join("scripts/install.sh"),
        "echo \"$*\" > \"$(dirname \"$0\")/../args.txt\"\n",
    )
    .unwrap();
    git(&repo, &["add", "scripts"]);
    git(&repo, &["commit", "-qm", "stub"]);
    git(&repo, &["tag", "-a", "v0.1.0", "-m", "x"]);
    commit(&repo, "two", "two");
    git(&repo, &["tag", "-a", "v0.10.0", "-m", "x"]);
    git(&repo, &["tag", "-a", "v0.9.0", "-m", "x"]);
    commit(&repo, "three", "three");
    git(&repo, &["tag", "-a", "v0.11.0-rc1", "-m", "x"]);
    // Uncommitted edits no longer block an update: nothing pulls into the working tree.
    std::fs::write(repo.join("f.txt"), "edited").unwrap();
    repo
}

#[test]
fn update_installs_the_newest_release_tag() {
    let repo = update_repo("newer");
    let (code, out, err) = run(&["update", "--app"], &repo, Some("0.1.0"));
    assert_eq!(code, 0, "{err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["from"], "v0.1.0");
    assert_eq!(v["to"], "v0.10.0");
    let args = repo.join("args.txt");
    assert_eq!(
        std::fs::read_to_string(&args).unwrap().trim(),
        "--version v0.10.0 --app"
    );
    assert_eq!(
        std::fs::read_to_string(repo.join("f.txt")).unwrap(),
        "edited"
    );

    // A dev build always takes the newest tag, and says why.
    std::fs::remove_file(&args).unwrap();
    let (code, out, err) = run(&["update"], &repo, None);
    assert_eq!(code, 0, "{err}");
    assert!(
        err.contains("dev build") && err.contains("v0.10.0"),
        "{err}"
    );
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        (v["from"].as_str(), v["to"].as_str()),
        (Some("dev"), Some("v0.10.0"))
    );
    assert_eq!(
        std::fs::read_to_string(&args).unwrap().trim(),
        "--version v0.10.0"
    );
    let _ = std::fs::remove_dir_all(repo);
}

#[test]
fn update_at_the_newest_tag_is_up_to_date() {
    let repo = update_repo("current");
    for installed in ["0.10.0", "0.11.0-rc1"] {
        let (code, out, err) = run(&["update"], &repo, Some(installed));
        assert_eq!(code, 0, "{err}");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["upToDate"], true, "{installed}");
    }
    assert!(!repo.join("args.txt").exists());
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

/// A fake cargo stands in for the release build (minutes): its "binary" reports the sha, f.txt and env
/// it was built from. Never touches ~/.cargo: CARGO_INSTALL_ROOT is `<tools>/root`.
fn fake_cargo(repo: &Path) -> PathBuf {
    let tools = repo.with_extension("tools");
    let _ = std::fs::remove_dir_all(&tools);
    std::fs::create_dir_all(&tools).unwrap();
    std::fs::write(
        tools.join("cargo"),
        r#"#!/bin/sh
while [ "$1" != --path ]; do shift; done
mkdir -p "$CARGO_INSTALL_ROOT/bin"
printf '#!/bin/sh\necho "%s %s %s %s"\n' "$(git -C "$2" rev-parse --short HEAD)" "$(cat "$2/f.txt")" \
  "$ATHARU_REPO" "$CARGO_TARGET_DIR" > "$CARGO_INSTALL_ROOT/bin/atharu"
chmod +x "$CARGO_INSTALL_ROOT/bin/atharu"
"#,
    )
    .unwrap();
    chmod_x(&tools.join("cargo"));
    tools
}

#[test]
fn install_builds_a_release_tag_never_the_working_tree() {
    let repo = temp_repo("install");
    add_script(&repo, "install.sh");
    let tools = fake_cargo(&repo);
    let installed = || {
        let o = Command::new(tools.join("root/bin/atharu"))
            .output()
            .unwrap();
        String::from_utf8(o.stdout).unwrap().trim().to_string()
    };

    // No release tags yet: refused, pointing at --dev.
    let (code, out, err) = script(&repo, "install.sh", &[], &tools);
    assert_eq!(code, 1, "{err}");
    assert!(out.is_empty());
    assert!(
        err.contains("no release tags") && err.contains("--dev"),
        "{err}"
    );

    git(&repo, &["tag", "-a", "v0.1.0", "-m", "x"]);
    let one = git(&repo, &["rev-parse", "--short", "HEAD"]);
    let two = commit(&repo, "two", "two");
    git(&repo, &["tag", "-a", "v0.2.0", "-m", "x"]);
    let three = commit(&repo, "three", "three");
    git(&repo, &["tag", "-a", "v0.3.0-rc1", "-m", "x"]);
    let four = commit(&repo, "four", "four");
    std::fs::write(repo.join("f.txt"), "edited").unwrap();

    let r = repo.display();
    for (args, want) in [
        (&[][..], format!("{two} two")),
        (&["--version", "v0.1.0"], format!("{one} one")),
        (&["--version", "0.3.0-rc1"], format!("{three} three")),
        (&["--dev"], format!("{four} four")),
    ] {
        let (code, out, err) = script(&repo, "install.sh", args, &tools);
        assert_eq!(code, 0, "{args:?}: {err}");
        assert!(out.is_empty(), "{args:?}: {out}");
        assert_eq!(installed(), format!("{want} {r} {r}/target"), "{args:?}");
        assert_eq!(
            err.contains("uncommitted changes are not included"),
            args == ["--dev"],
            "{args:?}: {err}"
        );
    }
    let (code, _, err) = script(&repo, "install.sh", &["--version", "v9.9.9"], &tools);
    assert_eq!(code, 1);
    assert!(err.contains("no tag v9.9.9"), "{err}");
    let (code, _, _) = script(
        &repo,
        "install.sh",
        &["--dev", "--version", "v0.1.0"],
        &tools,
    );
    assert_eq!(code, 2);

    // The edit survives and every temp worktree is gone.
    assert_eq!(
        std::fs::read_to_string(repo.join("f.txt")).unwrap(),
        "edited"
    );
    assert_eq!(git(&repo, &["worktree", "list"]).lines().count(), 1);
    let _ = std::fs::remove_dir_all(&tools);
    let _ = std::fs::remove_dir_all(repo);
}

/// A repo on main holding copies of the real version files and release.sh, plus a `gh` that must never run.
fn release_repo(name: &str) -> (PathBuf, PathBuf) {
    let repo = temp_repo(name);
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    std::fs::create_dir_all(repo.join("src-tauri")).unwrap();
    for f in [
        "Cargo.toml",
        "Cargo.lock",
        "src-tauri/Cargo.toml",
        "src-tauri/tauri.conf.json",
    ] {
        std::fs::copy(root.join(f), repo.join(f)).unwrap();
    }
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "chore: version files"]);
    add_script(&repo, "release.sh");
    let tools = repo.with_extension("tools");
    let _ = std::fs::remove_dir_all(&tools);
    std::fs::create_dir_all(&tools).unwrap();
    std::fs::write(
        tools.join("gh"),
        format!("#!/bin/sh\ntouch {}/gh-ran\n", tools.display()),
    )
    .unwrap();
    chmod_x(&tools.join("gh"));
    (repo, tools)
}

/// The version values release.sh bumps: both manifests, tauri.conf.json, both Cargo.lock members.
fn versions(repo: &Path) -> Vec<String> {
    let first = |f: &str, prefix: &str| {
        let s = std::fs::read_to_string(repo.join(f)).unwrap();
        s.lines()
            .find_map(|l| l.strip_prefix(prefix))
            .unwrap()
            .trim_end_matches(',')
            .trim_matches('"')
            .to_string()
    };
    let lock = std::fs::read_to_string(repo.join("Cargo.lock")).unwrap();
    let mut v = vec![
        first("Cargo.toml", "version = "),
        first("src-tauri/Cargo.toml", "version = "),
        first("src-tauri/tauri.conf.json", "  \"version\": "),
    ];
    for name in ["atharu", "atharu-app"] {
        let at = lock.find(&format!("name = \"{name}\"\n")).unwrap();
        let line = lock[at..].lines().nth(1).unwrap();
        v.push(
            line.trim_start_matches("version = ")
                .trim_matches('"')
                .into(),
        );
    }
    v
}

#[test]
fn release_refuses_bad_input_and_the_wrong_branch() {
    let (repo, tools) = release_repo("release-refuse");
    let refused = |args: &[&str], code: i32, why: &str| {
        let (c, _, err) = script(&repo, "release.sh", args, &tools);
        assert_eq!(c, code, "{args:?}: {err}");
        assert!(err.contains(why), "{args:?}: {err}");
    };
    refused(&[], 2, "usage");
    refused(&["1.2"], 1, "not semver");
    refused(&["v1.2.x"], 1, "not semver");
    refused(&["--hotfix", "0.2.1"], 1, "runs on release/0.2");
    std::fs::write(repo.join("f.txt"), "edited").unwrap();
    refused(&["0.2.0"], 1, "uncommitted changes");
    git(&repo, &["checkout", "-q", "--", "f.txt"]);
    git(&repo, &["checkout", "-qb", "feat/x"]);
    refused(&["0.2.0"], 1, "cut from main");
    git(&repo, &["checkout", "-q", "main"]);
    git(&repo, &["tag", "-a", "v0.2.0", "-m", "x"]);
    refused(&["0.2.0"], 1, "exists already");
    refused(&["0.1.9"], 1, "not greater than v0.2.0");
    assert_eq!(git(&repo, &["tag", "--list"]), "v0.2.0");
    assert_eq!(versions(&repo), vec![env!("CARGO_PKG_VERSION"); 5]);
    let _ = std::fs::remove_dir_all(&tools);
    let _ = std::fs::remove_dir_all(repo);
}

#[test]
fn release_bumps_tags_branches_and_hotfixes() {
    let (repo, tools) = release_repo("release-flow");
    commit(&repo, "a", "feat(cli): add a");
    commit(&repo, "b", "fix: repair b");
    commit(&repo, "c", "docs: explain c");

    let (code, out, err) = script(&repo, "release.sh", &["0.2.0"], &tools);
    assert_eq!(code, 0, "{err}");
    assert_eq!(versions(&repo), vec!["0.2.0"; 5]);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    assert_eq!(git(&repo, &["rev-parse", "release/0.2"]), head);
    assert_eq!(git(&repo, &["rev-parse", "main"]), head);
    assert_eq!(git(&repo, &["rev-parse", "v0.2.0^{commit}"]), head);
    assert_eq!(
        git(&repo, &["tag", "-l", "--format=%(contents)", "v0.2.0"]),
        "atharu v0.2.0"
    );
    assert_eq!(
        git(&repo, &["log", "-1", "--format=%B"]),
        "chore: release v0.2.0"
    );
    assert!(git(&repo, &["status", "--porcelain"]).is_empty());
    let log = std::fs::read_to_string(repo.join("CHANGELOG.md")).unwrap();
    let date = chrono::Local::now().format("%Y-%m-%d");
    assert_eq!(
        log,
        format!(
            "# Changelog\n\n## v0.2.0 — {date}\n\n### Features\n\n- Add a\n\n### Fixes\n\n- Repair b\n\n\
             ### Under the hood\n\n- Explain c\n- Add release.sh\n- Version files\n\n### Other\n\n- init\n"
        )
    );
    // No tty: nothing is pushed, the commands are printed instead.
    assert!(out.contains("not pushed"), "{out}");
    assert!(
        out.contains("git push origin main release/0.2 v0.2.0"),
        "{out}"
    );
    assert!(
        out.contains("gh release create v0.2.0 --title \"atharu v0.2.0\" --notes-file")
            && out.contains("--verify-tag"),
        "{out}"
    );
    assert!(!tools.join("gh-ran").exists());

    // A patch from main fast-forwards release/0.2; the entry holds only what's new, newest first.
    commit(&repo, "d", "feat: add d");
    let (code, _, err) = script(&repo, "release.sh", &["0.2.1"], &tools);
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        git(&repo, &["rev-parse", "release/0.2"]),
        git(&repo, &["rev-parse", "HEAD"])
    );
    let log = std::fs::read_to_string(repo.join("CHANGELOG.md")).unwrap();
    assert!(
        log.starts_with(&format!(
            "# Changelog\n\n## v0.2.1 — {date}\n\n### Features\n\n- Add d\n\n## v0.2.0"
        )),
        "{log}"
    );

    // Hotfix: committed on release/0.2, cut there, cherry-pick printed for main.
    git(&repo, &["checkout", "-q", "release/0.2"]);
    let fix = commit(&repo, "hot", "fix: hot");
    let (code, _, err) = script(&repo, "release.sh", &["0.2.2"], &tools);
    assert_eq!(code, 1);
    assert!(
        err.contains("cut from main") && err.contains("--hotfix"),
        "{err}"
    );
    let (code, _, err) = script(&repo, "release.sh", &["--hotfix", "0.3.1"], &tools);
    assert_eq!(code, 1);
    assert!(err.contains("runs on release/0.3"), "{err}");
    let (code, out, err) = script(&repo, "release.sh", &["--hotfix", "0.2.2"], &tools);
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        git(&repo, &["rev-parse", "v0.2.2^{commit}"]),
        git(&repo, &["rev-parse", "release/0.2"])
    );
    assert_eq!(versions(&repo), vec!["0.2.2"; 5]);
    assert!(out.contains("git push origin release/0.2 v0.2.2"), "{out}");
    assert!(
        out.contains(&format!(
            "git checkout main && git cherry-pick v0.2.1..{fix}"
        )),
        "{out}"
    );

    // release/0.2 now has commits main lacks: another 0.2.x from main is refused, 0.3.0 opens release/0.3.
    git(&repo, &["checkout", "-q", "main"]);
    let (code, _, err) = script(&repo, "release.sh", &["0.2.3"], &tools);
    assert_eq!(code, 1);
    assert!(err.contains("release/0.2 has commits main lacks"), "{err}");
    let (code, _, err) = script(&repo, "release.sh", &["0.3.0"], &tools);
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        git(&repo, &["rev-parse", "release/0.3"]),
        git(&repo, &["rev-parse", "HEAD"])
    );
    assert!(!tools.join("gh-ran").exists());
    let _ = std::fs::remove_dir_all(&tools);
    let _ = std::fs::remove_dir_all(repo);
}
