# Git hooks

All four hooks live in the **user-wide** hooks dir (`core.hooksPath = ~/.config/git/hooks`, set in
`dot_gitconfig.tmpl`), so they apply to every repo. Three of them are dispatchers that do nothing unless
the repository tracks its own hook of the same name under `.githooks/`.

## prepare-commit-msg: user-wide AI commit messages

`dot_config/git/hooks/executable_prepare-commit-msg` pipes the full staged diff (no truncation,
`git diff --cached --diff-algorithm=histogram`) to `claude -p --model=sonnet` under a 30-second
`timeout`, and prepopulates the commit editor with the returned Conventional Commits message (subject,
optional body, optional footers). The invocation also passes `CLAUDECODE=`, `MAX_THINKING_TOKENS=0`,
`--no-session-persistence`, `--tools=''`, `--disable-slash-commands`, `--setting-sources=''` and a
`--system-prompt`, so the call carries no session, no tools and no repo settings.

It bails, leaving the message alone, when git already supplied a message source (`-m`, `-F`, merge,
squash, template), during a merge (`MERGE_HEAD`), a cherry-pick (`CHERRY_PICK_HEAD`) or a rebase
(`rebase-merge` / `rebase-apply`), when the staged diff is empty, and when `SKIP_AI_COMMIT` is set to any
non-empty value. It chains to a repo-local `.git/hooks/prepare-commit-msg` when that file is executable.
It never blocks a commit; worst case the editor opens with an empty message.

The 30-second budget was raised from 10s because sonnet is slower than the haiku it replaced and the diff
is no longer truncated.

A per-repo `core.hooksPath` override would shadow this hook, which is why the per-repo checks below use
dispatchers rather than an override. **Do not reintroduce Git LFS here**: `git lfs install` writes
exactly such an override, and this repo tracks no LFS (large file storage) files.

## pre-commit: per-repo fast gate (unit tests plus secret scan)

`dot_config/git/hooks/executable_pre-commit` runs in every repo but only acts when the repository tracks
an executable `.githooks/pre-commit`, which it then `exec`s.

This repo's `.githooks/pre-commit` runs `just test-unit` (the unit suite only), then
`gitleaks git --staged --no-banner --redact`, which blocks any staged plaintext secret. Gitleaks is
provisioned as a Homebrew formula, and that stage is skipped when the binary is absent. Both must pass; a
failure blocks the commit.

Lint drift and the other suites are deliberately outside the commit loop: lint runs at pre-push, the full
suite at CI or via `just ship`. There is no install step, since the dispatcher is user-wide and the repo
hook is committed with its executable bit.

## pre-push: per-repo lint gate only

`dot_config/git/hooks/executable_pre-push` mirrors the pre-commit dispatcher and also forwards git's
stdin ref list. This repo's `.githooks/pre-push` runs `just lint-check` (the standalone treefmt drift
gate, ~16s uncached; nix left this repo on 2026-08-05) and nothing else. Standalone treefmt has no
dry-run mode, so a red gate has also already written the fixes into the working tree: stage them and push
again. **CI is the authority on the test suite.** Run the suite locally on demand with `just ship`.

### Why the suite left this hook

It used to run `just test` as well. Measured once, on 2026-07-29, a push cost 7m20s to 7m37s, of which
about 6m30s was integration plus end-to-end plus test-system; integration alone took 216s of that. It
runs its `.sh` tests one at a time and its `.bats` files under `bats --jobs 4`. Those seconds are one
reading on one day, not a tracked figure: no test pins them, and the suite keeps growing. CI then ran the
identical recipe 10 to 12 minutes later, and the commit hook had already run the unit camp, so one suite
ran three times per push and every round of rework paid for all three.

It also could not do the job it was there for: this hook tests the WORKING TREE while CI tests the
COMMIT, and on PR #116 the local gate passed while CI failed, on an edit that was never staged.

**What this narrowed:** every push used to run the whole suite locally, whatever the branch. CI runs on
pull requests and on pushes to `main`, so a push to a topic branch with no open pull request now runs the
suite nowhere. None of this touches branch protection, which is weaker than "cannot merge red" anyway:
`lint` is a required status check on `main`, so a red pull request is blocked there, but `enforce_admins`
is off, so a repository administrator can merge one red regardless. Widening the workflow's `push`
trigger to every branch was weighed and rejected: `push` and `pull_request` both fire once a branch has a
pull request, so it would run two identical macOS jobs per push for the whole life of every branch, to
cover the window before a pull request exists. `just ship` covers that window on demand instead.

## post-commit: per-repo knowledge-graph rebuild

`dot_config/git/hooks/executable_post-commit` mirrors the pre-commit dispatcher. Nothing global runs
graphify anymore; a repo that wants a knowledge-graph rebuild after each commit opts in by tracking its
own `.githooks/post-commit`. The old global-by-default design (graphify inlined in the dispatcher with a
`.githooks/no-graphify` opt-out marker) is gone.

**This repo opts in.** Its `.githooks/post-commit` launches `graphify update .` detached (a full rebuild
measures about 2s here; the detach keeps any rebuild off the commit path) with `PYTHONHASHSEED=0` for
reproducible clustering, logging to `~/.local/log/graphify/dotfiles-post-commit.log`. It exits 0 without
doing anything when `graphify` is not on PATH, during a rebase, merge or cherry-pick, when the commit
touched only the map (loop prevention), and when `GRAPHIFY_SKIP_HOOK` is exactly `1`.

`graphify-out/graph.json` is the **committed map** (the rest of `graphify-out/` stays gitignored); the
rebuilt map appears as an unstaged change to fold into the next commit. Cross-branch merges of the map
union-merge via the `graphify-union` driver (`.gitattributes` plus `[merge "graphify-union"]` in
`dot_gitconfig.tmpl`); without the driver git falls back to a normal conflict, resolvable by regenerating
with `graphify update .`.

## Bypassing

`git commit --no-verify` skips `pre-commit` and `prepare-commit-msg` for one commit. It does **not** skip
`post-commit`, which git never gates on `--no-verify`; use `GRAPHIFY_SKIP_HOOK=1` for that.
