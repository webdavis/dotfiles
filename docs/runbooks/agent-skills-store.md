# Agent skills and plugins

One list says which skills and plugins every agent gets: `.chezmoidata/agent_skills_and_plugins.yaml`.
The apply installs whatever is missing from it. Weekly uu lanes update what is installed and print what
changed. Nothing is ever removed for you: taking something off the list leaves it installed.

## Where things live

- `~/.agents/skills/<name>`: every skill, as a real folder. Codex reads this folder directly.
- Claude Code (`~/.claude/skills`) and the Hermes profiles get links into it. The apply makes the links
  from the list.
- Hermes hub skills live inside their Hermes profile, and Hermes owns them.
- Plugins stay where each tool keeps them: Claude Code, Codex and Hermes install their own.

## The list, section by section

| Section                        | Installed by                     | Updated by                                                   |
| ------------------------------ | -------------------------------- | ------------------------------------------------------------ |
| `skills.from_github`           | the apply (`npx skills add`)     | the `npx-skills` lane                                        |
| `skills.from_clawhub`          | the apply (`clawhub install`)    | the `clawhub-skills` lane                                    |
| `skills.vendored`              | the apply (`dot_agents/skills/`) | you, by hand; `vendored-skills-check` says when              |
| `skills.app_owned`             | the app itself                   | `cua-driver-skills` for Cua Driver; composio updates its own |
| `hermes.profiles.*.hub_skills` | the apply                        | the `hermes-skills` lane                                     |
| `claude_code`, `codex` plugins | the apply                        | `claude-plugins`, `codex-plugins`                            |
| Hermes plugins                 | the apply                        | `hermes-plugins`                                             |

`claude_code.skip_skills` lists skills Claude Code does not get a link for, because a plugin already
brings them (clean-code, pns) or it is not wanted there. `off_until_enabled` (Claude Code) and
`enabled: false` (Codex) keep a plugin installed but off.

## The weekly lanes

| Lane                    | What it does                                                                   |
| ----------------------- | ------------------------------------------------------------------------------ |
| `npx-skills`            | `npx skills add <repo> --skill … --agent codex -g -y`, once per repository     |
| `clawhub-skills`        | `clawhub update <name>`, once per skill                                        |
| `cua-driver-skills`     | `cua-driver skills update`; does nothing when Cua Driver is not installed      |
| `vendored-skills-check` | compares each vendored skill with its upstream                                 |
| `claude-plugins`        | refreshes the marketplaces, then `claude plugin update <id>` per plugin        |
| `codex-plugins`         | `codex plugin marketplace upgrade`, which also refreshes installed plugins     |
| `hermes-plugins`        | `hermes plugins update <name>` for each plugin installed from git              |
| `hermes-skills`         | `hermes -p <profile> skills update <name>` per hub skill, skipping `held` ones |

The scripts are in `dot_local/libexec/uu/`. Each prints one line per change
(`grilling: 1a2b3c4 → 4d5e6f7`, `tdd: added`) and nothing when nothing changed. Problems go to stderr as
`error[kind]: message`. Exit codes: 0 done, 75 try later, 100 needs you, 1 failed.

`just update-skills` runs `uu run skills`, the `npx-skills`, `clawhub-skills` and `cua-driver-skills`
lanes in that order.

`npx-skills` and `clawhub-skills` hold the lock `~/.agents/.skills.lock` while they run. If another run
holds it, they exit 75 and try again next time.

## Only when asked

A skill in `skills.on_demand` loads only when you call it by name.

- Claude Code: `private_dot_claude/modify_settings.json` sets it to user-invocable only.
- Codex: its `agents/openai.yaml` gets `policy.allow_implicit_invocation: false`, merged in with `yq`.
  The file's original text is kept in a last line starting `# uu-original-openai:`. `npx-skills` and
  `clawhub-skills` set it again after every update, because an update replaces the folder. Vendored
  skills carry their own file, and `cua-driver` is left alone because the app owns it.
- ClawHub skills get a `.clawhubignore` listing `agents/openai.yaml`, so that file does not count as a
  local edit when ClawHub updates the skill.

## When a lane needs you

- **`vendored-skills-check` prints `name: old → new`:** the upstream moved. Compare it with the copy in
  `dot_agents/skills/<name>`, bring over what you want, then set that skill's `last_compared` to the new
  hash. `moshi` is a fork on purpose, so only port what fits.
- **`clawhub-skills` says `local-changes`:** the installed copy differs from every release. Look at the
  folder, then reinstall it or keep your edit. The lane never passes `--force`.
- **`claude-plugins` says `needs-approval`:** the plugin wants to run a new install command. Read it, and
  if you trust it, run the command the lane printed. The lane never accepts it for you.
- **`hermes-skills` says `blocked`:** Hermes refused the update after its security scan. Look at why
  before you do anything; the lane never forces it. Set `held: true` on the entry to skip it meanwhile.

## Adding a skill

1. Pick its section. A GitHub repository goes under `from_github`, beside its repo. A ClawHub skill goes
   under `from_clawhub` with its slug. Anything else is committed under `dot_agents/skills/<name>` and
   added to `vendored`, with an `upstream` when there is one to watch.
1. Add it to `on_demand` if it should load only when asked.
1. Add it to each Hermes profile that should carry it.
1. Apply. The apply installs it and makes the links.

To remove one, take it off the list and delete the installed copy by hand.

## Special cases

- **Graphify** keeps its Claude Code skill outside the store. `~/.claude/skills/graphify` is a normal
  chezmoi link to `~/.local/share/graphify/claude/skills/graphify`, and the `uv-graphify-skill` lane
  refreshes it after `uv`. To repair it by hand:

  ```bash
  /usr/bin/env CLAUDE_CONFIG_DIR="$HOME/.local/share/graphify/claude" \
    "$HOME/.local/bin/graphify" install --platform claude
  ```

- **babysit for Hermes** is a real folder in chezmoi, `private_dot_hermes/private_skills/babysit/`, and
  not in the store. Claude Code and Codex get their own copy from the babysitter plugin.
