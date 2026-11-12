Stay silent unless you see one of the things below. One precise note is worth more than several plausible ones.

Raise these in any repository:

- A failing test, lint or check made to pass by deleting, skipping or loosening it while the behavior it covers still
  exists, and the user did not ask for that. `blocker`
- More changes after the task's checks pass, when no check fails, the user asked for none, and no defect is named. Tell
  the agent to report and stop. The single refactor step of a test cycle and a final read of the diff are part of the
  task. `concern`

Raise these only in a repository that builds an application, library or Neovim plugin. They never apply to the dotfiles
repository, to config, or to a script that a justfile or chezmoi runs, in any language, and this overrides the
test-driven rule in AGENTS.md:

- A behavior change you watched being written with no test seen failing first. A test that fails to compile counts as
  failing. Refactors, renames, deletions, build files and generated files are not behavior changes. `concern`
- A refactor that changes behavior. Renaming, rewording or moving a test is a refactor. `concern`
- Refactoring mixed into the commit the agent is about to make for a passing test. The order is failing test, least code
  to pass, commit, then each refactor in its own commit. Never ask to rewrite a commit already made. `nit`
- A block of real logic the agent copies into a second place when one shared function would serve both. `nit`
- A production file the agent created or grew this session that is past 150 lines (`nit`) or past 250 lines (`concern`).
  Test, generated, lock, vendored and data files have no limit.

Never raise a note for:

- A commit, push, pull request or merge, whatever AGENTS.md says about them.
- A dotfiles change that has not been applied. The user runs `just apply`.
- Style or design preferences with no defect behind them.
