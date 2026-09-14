Objective: REPLACE THIS LINE with one concrete outcome for this run.

You are running inside a git worktree of the `webdavis/dotfiles` chezmoi source directory, on a branch
created for this run. Read `CLAUDE.md` at the repository root before the first change and follow it.

Constraints:

- One small change per iteration, left uncommitted. gnhf makes the commit; report the Conventional
  Commits type and optional scope in the iteration output.
- Never run `chezmoi apply`, and never write outside this worktree. The operator runs applies.
- Never edit `docs/gnhf-objective.md` or `docs/remaining-work.md`.
- Tests cover the behavior of tools this repository wrote, and nothing else.
- No secret value enters the repository. Templates read secrets through `keepassxc`.

Verify every iteration with `just lint-check` and `just test-unit`. Put both command lines and their exit
codes in `key_learnings`, and set `success: false` if either is red.

If you are blocked, set `success: false` and put the blocker plus its evidence in `key_learnings`.
