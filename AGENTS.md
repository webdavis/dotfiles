## Workflows

- "install <url>": track the tool so the next `chezmoi apply` installs it. The decision tree is:
  - Provided by a package manager: `.chezmoidata/system_packages_autoinstall.yaml`.
  - Provides its own installer (for example, `curl <url>/install.sh`): a script under `.chezmoiscripts/` runs it
  - An agent plugin, or a skill a Claude Code or Codex marketplace plugin offers: `.chezmoidata/agent-plugins.yaml`
  - Any other agent skill (`npx`, `clawhub`, etcetera): `.chezmoidata/agent-skills.yaml`

## Agent Skills and Plugins

- A skill reaches each harness once:
  - Codex loads everything in `~/.agents/skills/`, so a skill installed there for any tool (Hermes included) must not
    also reach Codex as a plugin.
  - `scripts/treefmt/agent-data-validate.sh` rejects `.chezmoidata/agent-plugins.yaml` and
    `.chezmoidata/agent-skills.yaml` when a repository listed in them would reach one harness twice.

## Rules

- After the owner runs `just apply`, read `~/.local/state/chezmoi-apply/latest.apply.log`.
