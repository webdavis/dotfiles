# Agent tooling: OpenSpec

[OpenSpec](https://github.com/Fission-AI/OpenSpec) is a spec-driven-development CLI shared by every
harness on this machine. Only its GLOBAL configuration is tracked here: `dot_config/openspec/config.json`
deploys to `~/.config/openspec/config.json`, which is the one file the CLI reads for machine-wide
settings (`XDG_CONFIG_HOME` wins when set, and `dot_bashrc.tmpl` sets it to `~/.config`). Three of its
five top-level keys hold the shipped defaults and sit in the file anyway, so the posture reads off one
screen: `featureFlags`, `profile: core` (the six core workflows: propose, explore, apply, update, sync,
archive) and `delivery: both` (skills plus slash commands, so Claude Code gets both spellings while Codex
and hermes take the skills). `telemetry.enabled: false` is the one deliberate departure, and it is the
durable half of a pair: `dot_bashrc.tmpl` also exports `OPENSPEC_TELEMETRY=0`, which is checked ahead of
the file and so covers a fresh machine before the file exists, while the file covers every caller that
never sources the managed shell. `completionTipSeen: true` is there to stop drift. The CLI writes that
key itself on its first run with a TTY, so shipping it keeps the deployed file byte-identical to source
and a no-op apply quiet. Everything else the CLI may write there is telemetry bookkeeping that an
opted-out install never mints (`telemetry.anonymousId` is minted on the send path only), so no `modify_`
template is needed and none exists.

Project setup stays in the project, and this checkout has no `openspec/` root on purpose. Neither the
global config nor any generated instruction file needs one: specs belong to the repository they describe.
In an owning repository, run `openspec init --tools claude,codex,hermes` for the three harnesses this
repo manages. Measured at 1.13.0: that writes only OpenSpec's own files (`.claude/skills/`,
`.claude/commands/opsx/`, `.agents/skills/` for Codex, `.hermes/skills/`, and the `openspec/` root with
its `config.yaml`) and leaves `CLAUDE.md` and `AGENTS.md` byte-identical, so no harness has to be skipped
to protect the two chezmoi-rendered instruction files. Re-measure that before trusting a much newer CLI,
since appending to an instruction file is the kind of behavior an init flow can regain in a release.
hermes needs one more step per project: it loads skills from `~/.hermes/skills` only, so the project's
`.hermes/skills` has to be added to `skills.external_dirs` in `~/.hermes/config.yaml`. That key is
hermes's own: since 2026-09-14 the file is a chezmoi modify-template that declares only the webhook
routes and the ElevenLabs voice id, so `skills.external_dirs` is set with `hermes config set` and passes
through every apply untouched.

Upgrading the CLI and refreshing a project are separate jobs. The weekly uu npm lane (`[lanes.npm]` in
`dot_config/uu/private_config.toml.tmpl`) does the first, since it upgrades every globally installed npm
package. `@fission-ai/openspec` sits in the fnm node package list in
`.chezmoidata/system_packages_autoinstall.yaml` with no version pin, so nothing pulls it back to a fixed
version after the lane moves it. An apply can move it too: the fnm block of
`run_onchange_before_10-system-packages.sh.tmpl` re-runs `npm install -g` for already-installed packages
as its update pass, so a version bump is not the weekly lane's alone. Nothing re-runs `openspec update`,
which is the command that refreshes a project's GENERATED instruction files, and no `chezmoi apply` runs
it either. Run it by hand, in the project, after either path lands a new template generation, or after
changing `profile` or `delivery` in the tracked config, since both of those choices are baked into the
generated per-project files at init time.
