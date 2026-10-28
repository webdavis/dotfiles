Agent instructions belong in this file alone.

## Commands

- "install <url>": track the tool in this repo so the next `chezmoi apply` installs it.
- "remove <tool>": stop tracking the tool in this repo and remove it from this Mac.
- Decide which kind the tool is, then follow that kind's Install or Remove steps.

## Command line Tool or App

### Package manager provides it

- Install: add it under its package manager in `.chezmoidata/system_packages_autoinstall.yaml`. A Homebrew tap goes in both `taps` and `trusted_taps`.
- Remove:
  1. Remove its entry, and for a tap both `taps` and `trusted_taps` lines.
  2. Uninstall it with that package manager (`brew uninstall`, `brew untap`, `cargo uninstall`, `uv tool uninstall`, `npm uninstall -g`).

### A group in `.chezmoidata/system_packages_autoinstall.yaml` with its own script

- Install: add an entry shaped like its neighbors to the group that builds this kind of tool.
- Remove:
  1. Remove its entry.
  2. Read the script under `.chezmoiscripts/` that reads that group, and undo what it did with the tool's own uninstall, unlink or remove command.
  3. `trash` each leftover path the script names.

### Ships its own installer

- Install: add a script under `.chezmoiscripts/` that runs it.
- Remove:
  1. Read its script for what it installed and where.
  2. Run the uninstaller the tool documents.
  3. Leftover paths the script or docs name: `trash` the user's, list root's for the owner.
  4. Script installs only this tool: `trash` it. Script installs others too: remove this tool from it.

## Agent Skills and Plugins

### Marketplace or Hermes plugin or skill

- Install, in `.chezmoidata/agent-plugins.yaml` under its harness:
  - Claude Code or Codex: its marketplace and its `<plugin>@<marketplace>` id.
  - Hermes: its catalog name.
- Remove:
  1. Remove its id, and its marketplace when no plugin of it remains.
  2. Claude Code: `claude plugin uninstall <plugin>@<marketplace>`. Codex: `codex plugin remove <plugin>@<marketplace>`. Hermes plugin: `hermes plugins remove <name>`.
  3. A removed marketplace: `claude plugin marketplace remove <name>` or `codex plugin marketplace remove <name>`.
  4. Hermes hub skill: `hermes -p <profile> skills uninstall <name> --yes` in each profile that had it.

### Any other agent skill installer

- Install: add its entry under its harness in `.chezmoidata/agent-skills.yaml`.
- Remove:
  1. Remove its entry.
  2. npx entry: `npx skills remove <skill> -g` for each skill `~/.agents/.skill-lock.json` lists for its repo. clawhub entry: `trash ~/.agents/skills/<skill>`.
  3. `trash` its links in `~/.claude/skills/` and in each Hermes profile under its `hermes:` key.

### Each skill reaches each harness once

- Codex loads every skill in `~/.agents/skills/`, so such a skill reaches Codex through that folder alone.
- `just lint` refuses a skill or plugin that reaches a harness twice.

## Neovim Plugin

- Install:
  1. Write its lazy.nvim spec in `dot_config/nvim/lua/plugins/`. A file named for a topic rather than a plugin takes the plugins of that topic. Any other plugin gets its own file, named after its repo without `.nvim`. Set `version` only when the README asks for it.
  2. `chezmoi apply` that file.
  3. `nvim --headless "+Lazy! install" +qa` clones it and adds its lock line.
  4. `chezmoi re-add ~/.config/nvim/lazy-lock.json`.
- Remove:
  1. File holds only this plugin: `chezmoi destroy --force` it. File holds others too: delete its table, then `chezmoi apply` the file.
  2. `nvim --headless "+Lazy! clean" +qa` deletes the clone and its lock line.
  3. `chezmoi re-add ~/.config/nvim/lazy-lock.json`.

## Neovim Settings

- Options, keymaps and autocmds: `dot_config/nvim/lua/config/`.

## My Custom Neovim API

- Our own Lua modules: `dot_config/nvim/lua/custom_api/`.

## Editing

- Edit the source in this repo, never its target under `~`. `chezmoi source-path <target>` names the source.
- Run `just lint` before handing work over.
- Run `just ship` before opening a PR.

## Applying Changes

- After the owner runs `just apply`, read `~/.local/state/chezmoi-apply/latest.apply.log`.

## Git and GitHub

- Worktrees and branches:
  - Use `wt switch --create <branch>` to create a branch connected to a worktree.
  - Isolate pieces of work within their own worktree. Unrelated work should be done in separate worktrees.
- Commits:
  - One commit per behavior change. History reads as what changed, never as how the work went.
  - Commit a behavior change once it is done.
  - Feedback on a commit that main does not hold yet, including an edit the owner makes by hand, is folded into that commit, never into a new one:
    - Use `git commit --amend` for the latest commit.
    - Use `git commit --fixup <commit>` and `git rebase --autosquash` for an older commit.
    - If the branch has already been pushed, then use `git push --force-with-lease` to push the updates.
  - Write a commit body only when it adds to the subject line. Keep it short.
  - The body holds only the why and what the diff does not show. No trailers, links or sign-offs that name the tool, model or session that wrote the commit.
- Pull Requests:
  - Opening a PR, merging it, and setting auto-merge are three separate requests. Do one only when the owner asks for it by name. A request to open a PR is not a request to merge it.
  - Opening: one PR per piece of work. Unrelated work gets its own PR.
  - Fixing: fix an open PR by editing or renaming it, and keep the same PR.
  - Merging:
    1. Use `wt switch main` to move to the `main` branch checkout.
    2. Then use `npx -y gh-axi pr merge <number> --rebase --delete-branch` to merge the PR and delete the remote branch.
    3. Then use `wt remove <branch>` to remove the worktree and the local branch.
