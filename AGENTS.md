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
  2. Uninstall it with that package manager, and `brew untap` a removed tap.

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
  2. Uninstall it:
     - Claude Code: `claude plugin uninstall <plugin>@<marketplace>`.
     - Codex: `codex plugin remove <plugin>@<marketplace>`.
     - Hermes plugin: `hermes plugins remove <name>`.
  3. A removed marketplace: `claude plugin marketplace remove <name>` or `codex plugin marketplace remove <name>`.
  4. Hermes hub skill: `hermes -p <profile> skills uninstall <name> --yes` in each profile that had it.

### Any other agent skill installer

- Install: add its entry under its harness in `.chezmoidata/agent-skills.yaml`.
- Remove:
  1. Remove its entry.
  2. Remove the installed skill:
     - npx entry: `npx skills remove <skill> -g` for each skill `~/.agents/.skill-lock.json` lists for its repo.
     - clawhub entry: `trash ~/.agents/skills/<skill>`.
  3. `trash` its links in `~/.claude/skills/` and in each Hermes profile under its `hermes:` key.

### Each skill reaches each harness once

- Codex loads every skill in `~/.agents/skills/`, so such a skill reaches Codex through that folder alone.
- `just lint` refuses a skill or plugin that reaches a harness twice.

## Broccoli work tooling

Broccoli-specific tooling lives in two files:

- `.chezmoidata/broccoli-tools.yaml`: this contains the packages, skills and MCP servers to install, and the plugin folders they belong to.
- `.chezmoiscripts/run_onchange_after_18-install-broccoli-tools.sh.tmpl`: this installs everything the data file lists, other than the Homebrew entries, which `.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl` installs, and runs the installers and logins that have no entry in the data file.

The steps under Command line Tool or App and under Agent Skills and Plugins apply, with these changes:

- Package manager provides it: the same entry, under `broccoli.packages`.
- Ships its own installer: an `install_<tool>` function in the Broccoli script, called from `main`, instead of a script of its own.
- Skill: an npx entry under the folder that loads it, `tech_support` or `engineering`. The script prunes a removed one.
- MCP server: an entry under that folder's `mcp_servers`; tools that must prompt first go in its `ask` list.

## Neovim

### Plugin

- Install:
  1. Write its lazy.nvim spec in `dot_config/nvim/lua/plugins/`:
     1. A file named for a topic rather than a plugin takes the plugins of that topic.
     2. Any other plugin gets its own file, named after its repo without `.nvim`.
     3. Set `version` only when the README asks for it.
  2. `chezmoi apply` that file.
  3. `nvim --headless "+Lazy! install" +qa` clones it and adds its lock line.
  4. `chezmoi re-add ~/.config/nvim/lazy-lock.json`.
- Remove:
  1. Remove its spec:
     - File holds only this plugin: `chezmoi destroy --force` it.
     - File holds others too: delete its table, then `chezmoi apply` the file.
  2. `nvim --headless "+Lazy! clean" +qa` deletes the clone and its lock line.
  3. `chezmoi re-add ~/.config/nvim/lazy-lock.json`.

### Config

- Options, keymaps and autocmds: `dot_config/nvim/lua/config/`.
- Our own Lua modules: `dot_config/nvim/lua/custom_api/`.

## Editing

- A deployed file never mentions how it was produced or where its values come from.
- Edit a file's source in this repo, never its deployed copy. Find the source with `chezmoi source-path <target>`.
- Run `just lint` before handing work over.
- Run `just ship` before opening a PR.

## Applying Changes

- After the owner runs `just apply`, read `~/.local/state/chezmoi-apply/latest.apply.log`.
