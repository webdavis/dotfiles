# PR Merge

Merge the current PR with a merge commit, delete the remote branch, switch back to main, and pull
latest. Every GitHub operation runs through `npx -y gh-axi`; the bare `gh` CLI is never invoked
directly.

## Steps

1. Run `git rev-parse --abbrev-ref HEAD` for the current branch, then
   `npx -y gh-axi pr list --head <branch>` to get its PR number.
1. Merge it, substituting the real number and branch into both lines:

```
SUBJECT="Merge pull request #<number> from webdavis/<branch> (#<number>)"
npx -y gh-axi pr merge <number> --merge --delete-branch --subject "$SUBJECT"
```

1. Run `git checkout main`.
1. Run `git pull`.
1. Report success, quoting the merge commit subject, or the specific failure mode if any step fails.

## Safeguards

- Bail if there's no PR associated with the current branch.
- Bail if working tree isn't clean (report what's uncommitted).
- Do not force-merge over failing checks, report them and wait for user direction.
- The merge method is a merge commit. Do not substitute another method, and do not drop `--subject`;
  the trailing `(#<number>)` is the convention this repository's history follows.
