# PR Merge

Squash-merge the current PR, delete the remote branch, switch back to main, and pull latest.

## Steps

1. Run `gh pr merge --squash --delete-branch` for the current PR.
1. Run `git checkout main`.
1. Run `git pull`.
1. Report success or the specific failure mode if any step fails.

## Safeguards

- Bail if there's no PR associated with the current branch.
- Bail if working tree isn't clean (report what's uncommitted).
- Do not force-merge over failing checks, report them and wait for user direction.
