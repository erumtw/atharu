//! Bakes the git sha, release tag and build date into the binary for `atharu version` / `update`.
//! Not a git checkout (the Docker build copies no .git) → the vars stay unset and read "unknown".
use std::process::Command;

fn cmd(prog: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(prog).args(args).output().ok()?;
    let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (out.status.success() && !s.is_empty()).then_some(s)
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if let Some(sha) = cmd("git", &["rev-parse", "--short", "HEAD"]) {
        println!("cargo:rustc-env=ATHARU_GIT_SHA={sha}");
        // Unset on a commit that isn't a release tag: `update` reads that as a dev build (D40).
        if let Some(tag) = cmd(
            "git",
            &["describe", "--tags", "--exact-match", "--match", "v[0-9]*"],
        ) {
            println!("cargo:rustc-env=ATHARU_GIT_TAG={tag}");
        }
        // --git-path, not ".git/HEAD": also right in a worktree, where .git is a file.
        let head = cmd("git", &["symbolic-ref", "-q", "HEAD"]);
        for r in ["HEAD"].into_iter().chain(head.as_deref()) {
            if let Some(p) = cmd("git", &["rev-parse", "--git-path", r]) {
                println!("cargo:rerun-if-changed={p}");
            }
        }
    }
    // ponytail: date of the last build-script run (new commit or clean build), not of every relink.
    if let Some(day) = cmd("date", &["-u", "+%Y-%m-%d"]) {
        println!("cargo:rustc-env=ATHARU_BUILT={day}");
    }
}
