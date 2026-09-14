//! What git's own answer says about the checkout an event fired in. The
//! fixtures are `git rev-parse --path-format=absolute --git-common-dir
//! --show-toplevel --abbrev-ref HEAD` as git 2.55.0 answered it, measured
//! 2026-09-14 in a linked worktree and in a detached checkout, with the
//! directories renamed to neutral ones: this crate is installed on machines
//! that are not this one, so no fixture names a path only its author has.

use super::checkout_of;

#[test]
fn the_repository_comes_off_the_common_directory_so_a_worktree_is_not_the_project() {
    let checkout = checkout_of(
        "/repos/dotfiles/.git\n\
         /worktrees/dotfiles/feat-sender-header\n\
         feat/sender-header\n",
    );
    assert_eq!(checkout.repository, "dotfiles");
    assert_eq!(checkout.branch, "feat/sender-header");
}

#[test]
fn a_detached_head_takes_the_worktree_directory_name_for_its_branch_slot() {
    let checkout = checkout_of(
        "/tmp/probe/myrepo/.git\n\
         /tmp/probe/myrepo\n\
         HEAD\n",
    );
    assert_eq!(checkout.branch, "myrepo");
}

/// THE FLAGS THEMSELVES, against a real git. A mistyped option answers
/// nothing, which reads exactly like a directory outside a repository, so the
/// parse above would keep passing while every header lost its project.
#[test]
fn a_real_repository_answers_its_own_name_and_branch() {
    let root = std::env::temp_dir()
        .canonicalize()
        .expect("the canonical temp directory")
        .join(format!("pns-git-{}", std::process::id()))
        .join("fixture-repo");
    std::fs::create_dir_all(&root).expect("the fixture repository");
    let git = |arguments: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(arguments)
            .output()
            .expect("git runs");
    };
    git(&["init", "--initial-branch", "trunk", "--quiet"]);
    // A COMMIT, because `rev-parse` refuses an unborn HEAD outright and one
    // refusal empties the whole answer; see `git_checkout`.
    git(&[
        "-c",
        "user.email=fixture@example.invalid",
        "-c",
        "user.name=fixture",
        "commit",
        "--quiet",
        "--allow-empty",
        "-m",
        "first",
    ]);
    let cwd = root.to_string_lossy().into_owned();
    assert_eq!(
        super::git_checkout(&cwd),
        super::Checkout {
            repository: "fixture-repo".to_string(),
            branch: "trunk".to_string(),
        }
    );
    let _ = std::fs::remove_dir_all(root.parent().expect("the fixture parent"));
}
