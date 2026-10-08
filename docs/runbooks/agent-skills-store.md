# Agent skills and plugins

Two files say what every agent gets. `.chezmoidata/agent-plugins.yaml` lists, per harness, the
marketplaces and the plugins installed from them. `.chezmoidata/agent-skills.yaml` lists the skills,
grouped by the command that installs them. The apply installs whatever is missing; the weekly uu lanes
update what is installed and print what changed. Nothing is ever removed for you: taking something off a
list leaves it installed, so trash the leftover by hand.

Every template that reads the files goes through `.chezmoitemplates/agent-derived.json.tmpl`, which holds
the derivation rules once: a marketplace's name is the text after the `@` in its first plugin id; Claude
Code reaches the `claude` and `codex_and_claude` groups and Codex the `codex` and `codex_and_claude`
groups; `anthropics/claude-plugins-official` ships inside Claude Code and is never added; store skill
names come from the skills CLI's lock file (`~/.agents/.skill-lock.json`) for npx entries, from the slug
for ClawHub, from the store folders for the generated ElevenLabs set, and from `skill:` for the `but` and
`apps` entries. `treefmt` runs `scripts/treefmt/agent-data-validate.sh` over both files and refuses a
plugin or skill that would reach one harness twice.

## Where things live

- `~/.agents/skills/<name>`: every store skill, as a real folder. Codex reads this folder directly.
- Claude Code (`~/.claude/skills`) gets links into it: the skills CLI makes them for npx entries, the
  apply makes them for the rest. Hermes profiles get links for the `elevenlabs`, `but` and `apps` entries
  that name them, and their own copies of npx entries that name them (`--agent hermes-agent`).
- Hermes hub skills (`skills.harnesses.hermes.<profile>`) live inside their profile, and Hermes owns
  them. Every profile but `default` has opted out of the bundled skills, so it carries only its list.
- Plugins stay where each tool keeps them: Claude Code, Codex and Hermes install their own. In Codex a
  plugin's skill is `$<plugin>:<skill>`; in Claude Code it is `/<plugin>:<skill>`.

## The files, section by section

| Section                                                 | Installed by                                | Updated by                                               |
| ------------------------------------------------------- | ------------------------------------------- | -------------------------------------------------------- |
| `plugins.harnesses.claude`, `codex`, `codex_and_claude` | the apply (`claude plugin`, `codex plugin`) | `claude-plugins`, `codex-plugins`                        |
| `plugins.harnesses.hermes`                              | the apply (`hermes plugins install`)        | `hermes-plugins`                                         |
| `skills.harnesses.*.npx`                                | the apply (`npx skills add`)                | `npx-skills`, `npx-skills-codex`, `npx-skills-<profile>` |
| `skills.harnesses.*.clawhub`                            | the apply (`clawhub install`)               | `clawhub-skills`                                         |
| `skills.harnesses.*.elevenlabs`, `but`                  | the apply (`generate-tool-skills.sh`)       | `tool-skills`                                            |
| `skills.harnesses.*.apps`                               | the app itself                              | `cua-driver-skills`                                      |
| `skills.harnesses.hermes.<profile>`                     | the apply (`hermes skills install`)         | `hermes-skills`                                          |
| `skills.harnesses.claude_desktop`                       | `just claude-desktop-skills`                | you, by rerunning it                                     |

Three keys on a skill entry: `on_demand: true` makes it load only when asked (Claude Code sets
`user-invocable-only`; Codex gets `policy.allow_implicit_invocation: false` in the skill's
`agents/openai.yaml`, written by `~/.config/uu/scripts/mark-on-demand-skills.sh` on the apply and after
every update). `agent: lara` names the subagent the skills are for: Claude Code lists them `name-only`,
so they cost the main session their names and nothing more, and the `lara` subagent preloads the two
entry skills. `hermes: [lara]` lists the Hermes profiles that get it from the store. Codex cannot scope a
skill to one of its agents (a custom agent's own `skills.config` can only disable more), so plain Codex
sees lara's skills too.

## The weekly lanes

| Lane                | What it does                                                                          |
| ------------------- | ------------------------------------------------------------------------------------- |
| `npx-skills`        | `npx skills add <repo> --skill '*' --agent claude-code --agent codex`, per repository |
| `npx-skills-codex`  | the same for the `codex` group, `--agent codex` only                                  |
| `npx-skills-lara`   | the same with `HERMES_HOME` set to lara's profile and `--agent hermes-agent`          |
| `clawhub-skills`    | `clawhub update --all`, or `clawhub update <name>` per skill when that finds none     |
| `tool-skills`       | `generate-tool-skills.sh`: regenerates the ElevenLabs set and the gitbutler skill     |
| `cua-driver-skills` | `cua-driver skills update`; does nothing when Cua Driver is not installed             |
| `skills-on-demand`  | marks the on-demand skills only-when-asked in Codex again after the updates           |
| `claude-plugins`    | refreshes the marketplaces, then `claude plugin update <id>` per plugin               |
| `codex-plugins`     | `codex plugin marketplace upgrade`, which also refreshes installed plugins            |
| `hermes-plugins`    | `hermes plugins update <name>` for each plugin                                        |
| `hermes-skills`     | `hermes -p <profile> skills update` for each profile with hub skills                  |

Each lane but `tool-skills` and `skills-on-demand` is a uu builtin; those two run scripts under
`dot_config/uu/scripts/`. Each prints one line per change and nothing when nothing changed. Problems go
to stderr as `error[kind]: message`. Exit codes: 0 done, 75 try later, 100 needs you, 1 failed.
`just update-skills` runs `uu run skills`, the skills group, in order.

## When a lane needs you

- **`clawhub-skills` says `local-changes`:** the installed copy differs from every release. Look at the
  folder, then reinstall it or keep your edit. The lane never passes `--force`.
- **`claude-plugins` says `needs-approval`:** the plugin wants to run a new install command. Read it, and
  if you trust it, run the command the lane printed.
- **`hermes-skills` says `blocked`:** Hermes refused the update after its security scan. Look at why
  before you do anything; the lane never forces it.
- **`tool-skills` says `generate-failed`:** run `elevenlabs generate-skills --output-dir /tmp/x` or
  `but skill install --path ~/.agents/skills/gitbutler` by hand; the ElevenLabs CLI needs
  `elevenlabs auth login` once.

## Adding something

- A plugin: add its `owner/repo` and `<plugin>@<marketplace>` ids under the harness group that should
  have it. If the upstream names its Claude and Codex marketplaces differently, make one entry under
  `claude` and one under `codex`.
- An npx skill: add `repo: owner/repo[/path]` under the group's `npx` list, with `on_demand`, `agent` or
  `hermes` as needed. Every skill under that repo or path is installed.
- A Hermes hub skill: add its identifier (`hermes skills search <name>` prints it) under the profile.
- A Claude Desktop skill: add it under `claude_desktop` and rerun `just claude-desktop-skills`.

Then apply. To remove one, take it off the list and trash the installed copy by hand.

## Special cases

- **Graphify** keeps its Claude Code skill outside the store. `~/.claude/skills/graphify` is a normal
  chezmoi link to `~/.local/share/graphify/claude/skills/graphify`, and the `graphify` lane refreshes it
  after `uv` when its version is older than the graphify program's.
- **babysit for Hermes** is a real folder in chezmoi, `private_dot_hermes/private_skills/babysit/`, and
  not in the store. Claude Code and Codex get their own copy from the babysitter plugin.
- **`humanizer`** is bundled with Hermes, so the default profile carries it with no line in the data.
