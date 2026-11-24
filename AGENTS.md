Agent instructions belong in this file alone.

## Commands

- "install <url>": track the tool in this repo so the next `chezmoi apply` installs it.
- "remove <tool>": stop tracking the tool in this repo and remove it from this Mac.
- Decide which kind the tool is, then follow that kind's Install or Remove steps.

## Command line Tool or App

### Package manager provides it

- Install: add it under its package manager in `.chezmoidata/system_packages_autoinstall.yaml`. A Homebrew tap goes in both `taps` and `trusted_taps`.
- Remove:
  1. Remove its entry. For Homebrew-specific packages, check if the package has a tap. If it does, then remove the `taps` and `trusted_taps` lines for that package, unless another listed package still uses it.
  2. Uninstall it with that package manager. Homebrew does this itself on the next apply, unless the apply log shows `error[bulk-cleanup-refused]`.

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
  - Claude Code, Codex or omp: its marketplace and its `<plugin>@<marketplace>` id. An omp plugin goes under `omp.plugins` and is also listed for Claude Code.
  - Hermes: its catalog name.
- Remove:
  1. Remove its id, and its marketplace when no plugin of it remains.
  2. Uninstall it:
     - Claude Code: `claude plugin uninstall <plugin>@<marketplace>`.
     - Codex: `codex plugin remove <plugin>@<marketplace>`.
     - omp: `omp plugin uninstall <plugin>@<marketplace>`.
     - Hermes plugin: `hermes plugins remove <name>`.
  3. A removed marketplace: `claude plugin marketplace remove <name>` or `codex plugin marketplace remove <name>`.
  4. Hermes hub skill: `hermes -p <profile> skills uninstall <name> --yes` in each profile that had it.

### omp extension from npm

- Install: an entry under `omp.extensions` in `.chezmoidata/agent-plugins.yaml`, with its npm name as `package` and the tools that run unasked in its `allow` list.
- Remove:
  1. Remove its entry.
  2. `omp plugin uninstall <package>`.
- Tool that must ask first: its name under `omp.ask` in `.chezmoidata/agent-plugins.yaml`. A tool is named under `allow` or `ask`, never both.
- Each apply rewrites the approval rules in omp's config from these lists, so a rule set by hand is dropped.

### Any other agent skill installer

- Install: add its entry under its harness in `.chezmoidata/agent-skills.yaml`. An entry under `media` loads only in media sessions.
- Remove:
  1. Remove its entry.
  2. Remove the installed skill:
     - npx entry: `npx skills remove <skill> -g` for each skill `~/.agents/.skill-lock.json` lists for its repo.
     - clawhub entry: `trash ~/.agents/skills/<skill>`.
  3. `trash` its links in `~/.claude/skills/`, in `~/.omp/agent/skills/` and in each Hermes profile under its `hermes:` key.

### Each skill reaches each harness once

- Codex loads every skill in `~/.agents/skills/`, so such a skill reaches Codex through that folder alone.
- omp loads the standalone skills linked into `~/.omp/agent/skills/`, which are the ones Claude Code gets.
- `just lint` refuses a skill or plugin that reaches a harness twice.

## Broccoli work tooling

`.chezmoidata/broccoli-tools.yaml` lists Broccoli packages, skills and MCP servers. The keys `tech_support` and `engineering` each match a plugin folder under `dot_config/agents/`.

Follow the steps under "Command line Tool or App" and "Agent Skills and Plugins", with these changes:

- Package manager provides it: the entry goes under `broccoli.packages`.
- Ships its own installer: add an installer to the script under `.chezmoiscripts/` that installs the `npx` entries of `.chezmoidata/broccoli-tools.yaml`. Shape it and call it like the installers already in that script, and keep it from editing shell startup files, as they are.
- Skill: an `npx` entry under the key of the folder that loads it. An entry with no path after the repo installs every skill in that repo. That script removes any skill whose entry is gone, so a removed entry needs no other step.
- MCP server: an entry under that key's `mcp_servers`, with the tools that must ask first in its `ask` list. To remove one, clear any login with `/mcp` in a session that loads the folder, then delete its entry. The next apply drops the server and its ask rules. To keep an entry but stop loading its server, set `enabled: false` on it. A header that carries a secret gives its short name as `secret` and the part to read as `field`.

### omp

`sbs` and `sbe` start a Broccoli session on omp's `broccoli` profile. `sbs --claude` and `sbe --claude` start it on Claude Code, with the same plugin folders. `broccoli.omp` lists what the omp profile loads beyond those folders:

- Plugin: an entry under `broccoli.omp.plugins`, shaped as in `agent-plugins.yaml`. To remove one, delete its entry, then run `omp --profile broccoli plugin uninstall <plugin>@<marketplace>`.
- Extension from npm: an entry under `broccoli.omp.extensions`, with its npm name as `package`. To remove one, delete its entry, then run `omp --profile broccoli plugin uninstall <package>`.
- Skill: its name under `auto` or `manual` in `broccoli.omp.tech_support`, or in `broccoli.omp.engineering` when only an engineering session needs it. A skill named in neither list is absent from the session.
- A skill whose plain name another plugin holds is named with its prefix, as `omp --profile broccoli skill list` prints it.
- Shell command that runs unasked: its command words under `broccoli.omp.bash.allow`, as in `pylon issues get`. Name only commands that change nothing. A glob the session must refuse goes under `broccoli.omp.bash.deny`.
- `dot_config/agents/pylon-guard/` asks before any other `pylon` command in every permission mode, through an omp extension and a Claude Code hook that share one policy. Broccoli Claude sessions deny the claude.ai Pylon connector.

## Neovim

### Plugin

- Install:
  1. Write its lazy.nvim spec in `dot_config/nvim/lua/plugins/`:
     1. A file named for a topic rather than a plugin takes the plugins of that topic.
     2. Any other plugin gets its own file, named after its repo without `.nvim`.
     3. Set `version` only when the README asks for it.
  2. `chezmoi apply` that file.
  3. `nvim --headless "+Lazy! install" +qa` clones it and adds its lock line.
- Remove:
  1. Remove its spec:
     - File holds only this plugin: `chezmoi destroy --force` it.
     - File holds others too: delete its table, then `chezmoi apply` the file.
  2. `nvim --headless "+Lazy! clean" +qa` deletes the clone and its lock line.
- Lock file:
  - `~/.config/nvim/lazy-lock.json` is a link to `dot_config/nvim/lock/lazy-lock.json`, so lazy.nvim writes plugin versions straight into this repo.
  - The next `chezmoi apply` publishes a changed lock file to `main` in a commit of its own. Leave the file out of other commits.
  - Undo an update: put the earlier lock back with `git restore dot_config/nvim/lock/lazy-lock.json`, or with `git restore --source <commit> dot_config/nvim/lock/lazy-lock.json` once the update is published and pulled. Then `:Lazy restore`.
- Plugin that ships in the repo of a cargo-git program:
  1. Set `nvim_plugin: true` on its entry in `.chezmoidata/system_packages_autoinstall.yaml`. Each apply then keeps a checkout under `~/.local/share/nvim/paired/` on the commit the program was built from.
  2. Build its spec on `require("custom_api.paired_plugin").spec(<name>)`, as `dot_config/nvim/lua/plugins/pwe.lua` does. It gets no lock line.
  3. Other specs name it by its plain name in `dependencies`, and use it only when `is_present(<name>)`, because the checkout is absent until an apply has built the program.

### Config

- Options, keymaps and autocmds: `dot_config/nvim/lua/config/`.
- Our own Lua modules: `dot_config/nvim/lua/custom_api/`.

## Secrets

- `.chezmoidata/secrets.yaml` names every vault entry the dotfiles read: a short name, then the entry's title. An entry read for an attribute or an attached file is a block with its `title`, and each one under `attributes` or `attachments` as a short name and its real name.
- A template reads one with `includeTemplate "secret.tmpl" (list "<name>" "<field>")`. `Password`, `UserName` and `URL` read those fields. Any other word is a short name the entry lists.
- `just lint` refuses a template that reads the vault any other way.
- `just check-keepass` checks that the vault holds each named entry.

## Editing

- A deployed file never mentions how it was produced or where its values come from.
- Edit a file's source in this repo, never its deployed copy. Find the source with `chezmoi source-path <target>`.
- Run `just lint` before handing work over.
- Run `just ship` before opening a PR.

## Applying Changes

- After the owner runs `just apply`, read `~/.local/state/chezmoi-apply/latest.apply.log`.
