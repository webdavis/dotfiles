Objective: REPLACE THIS LINE with one concrete outcome for this run.

You are running inside a git worktree of the `webdavis/dotfiles` chezmoi source directory, on a branch
created for this run. Read `CLAUDE.md` at the repository root before the first change and follow it.

Constraints:

- One small change per iteration, committed with its own conventional-commits message.
- Never run `chezmoi apply`, and never write outside this worktree. The operator runs applies.
- Never edit `docs/gnhf-objective.md` or `docs/remaining-work.md`.
- Tests cover the behavior of tools this repository wrote, and nothing else.
- No secret value enters the repository. Templates read secrets through `keepassxc`.

Verify every iteration with `just lint-check` and `just test-unit`, and record both command lines with
their exit codes in the iteration notes. Do not report an iteration as successful without them.

If you are blocked, commit nothing and write the blocker plus the evidence for it into `notes.md`.
