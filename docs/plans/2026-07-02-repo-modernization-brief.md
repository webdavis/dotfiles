# Notification System + Repo Modernization — Brief for Implementation

**Status:** brief / not yet designed — hand this to a fresh model or session with no memory of the
conversation that produced it. **Repo:** `/Users/stephen/workspaces/Ivy/webdavis/dotfiles` — a chezmoi
dotfiles repo. Read its root `CLAUDE.md` in full before touching anything; it governs code style,
security, and architecture conventions in depth and is not optional reading. Separately,
`private_dot_claude/CLAUDE.md` (the chezmoi source for the *global* `~/.claude/CLAUDE.md`, applying to
every Claude Code project, not just this repo) has a "Tool preferences" section that now states `gh-axi`
is preferred over raw `gh` for all GitHub operations, and `chrome-devtools-axi` is preferred for Chrome
DevTools-based browser automation -- both were installed prior to this brief bein

## Read this first: creative freedom and pushback

**Every recommendation, rationale, or design choice in this document — apart from the Standing Rules
section immediately below — is a starting hypothesis, not a decision, and you are expected to challenge
it if you have a better idea.** This explicitly includes: the Rust precedent mentioned for the
notification rewrite (pick something else entirely if it's the better call), the specific shape described
for the four already-shipped notification fixes, the framing of the spam-bug root cause and how to fix
it, the nvim-overhaul v1/v2/v3 designs themselves (the human explicitly wants these *re-evaluated*, not
rubber-stamped — see that section), the exact mechanics proposed for the branch-combination workflow, and
the ordering/prioritization across the workstreams below. If you disagree with something written here,
say so, explain why, and propose something better — that is the expected behavior, not a deviation from
it. You also have full freedom to choose whatever programming language(s) best fit each piece of work;
nothing in this document locks in a language for anything.

What follows this section is genuinely not up for reinterpretation, because it isn't "a choice made in
this plan" — it's either a standing policy of this repo/user that predates and outlives this initiative,
or a literal, explicit instruction the human gave directly for this work.

## Standing rules (not part of "creative freedom")

- **Investigate before implementing.** Every file this brief describes should be re-read and re-verified
  against the live repo before you touch it — this is a snapshot from one point in time and parts of it
  (especially branch/commit counts) will already be stale by the time you read it.
- **YAGNI constrains *how*, not *whether*, you make a large change.** Nothing here should stop you from
  proposing something as big as a shell replacement (see Workstream 4) if it's genuinely justified — but
  build whatever you build without speculative abstraction beyond what today's actual requirements need.
- **Never patch, fork, or vendor third-party tools.** `openhue`, `terminal-notifier`, `curl`, `jq`,
  moshi's API, Hermes' webhook gateway, `lazy.nvim`, `nushell` itself, `Thaw`, `gh-axi`,
  `chrome-devtools-axi` -- call/configure them through their own supported interfaces. Do not reimplement
  or bundle their internals.
- **Commit discipline.** Conventional Commits, no `Co-Authored-By: Claude` or similar trailer, separate
  logically distinct changes into separate commits, never commit unless explicitly asked that session.
- **Destructive-action gates.** Nothing here should need `rm -rf`, force-push, or bypassing hooks without
  asking first.
- **⚠️ `osquery`'s design and code are off-limits to redesign or edit creatively** -- but its
  *already-written, already-PR'd work is explicitly in scope to incorporate verbatim* (see Workstream 2;
  this is a deliberate refinement of an earlier, stricter instruction). Do not make architecture or logic
  changes to anything under `.chezmoitemplates/osquery/`, `dot_local/bin/executable_osquery-*`, or the
  osquery LaunchAgents/plists -- that's a separate, mostly-finished effort (PR #25, backed by design doc
  PR #24) that the human is driving directly. You ARE authorized, per Workstream 2 below, to take PR
  #25's branch (`feat/osquery-alerter-three-tier`) and combine its already-authored commits into the
  described branch-combination-and-split workflow -- that's organizing and shipping already-written work,
  not redesigning it. If anything about osquery's actual behavior or query logic seems wrong or worth
  improving, note it and flag it to the human -- don't just change it.
- **Explicit git-workflow requirements from the human (see Workstream 2 for full detail):** every PR that
  ships must be a complete, working, self-contained unit -- no PR may ship code that isn't actually wired
  in and used by the time it merges. You may pause and ask the human to run `chezmoi apply` whenever a
  step needs it (never run a bare `chezmoi apply` yourself from automation -- see the
  KeePassXC-gated-template list in this repo's `CLAUDE.md`). Use `gh-axi`, not raw `gh`, for GitHub
  operations (branch/PR management, checks, etc.) -- see `private_dot_claude/CLAUDE.md`'s Tool
  preferences section (the global CLAUDE.md source, not the project-root one).
- **Every piece of new logic gets a unit test**, and tests are discoverable via this repo's `test/`
  directory / `just test` convention (see Workstream 1 for exactly how that reconciles with
  language-native test layouts). This is a direct, explicit requirement from the human, not a design
  choice open to reinterpretation.

______________________________________________________________________

## Table of contents

1. Notification system rewrite (relay + Hue) -- the original ask
1. Branch combination + incremental PR delivery -- a new git/process workstream
1. Whole-repo creative sweep
1. Possible shell replacement: bash -> nushell
1. Install `stonerl/Thaw`
1. Neovim: re-evaluate and finally implement the existing nvim-overhaul design
1. Cross-cutting open questions
1. Reference strategy: essential-feed-case-study (binding on *how*, not *what*)
1. Operational items out of scope

You decide how these relate to each other -- whether they're one giant effort, several independent ones,
or some sequenced combination. Nothing about their order here implies priority.

______________________________________________________________________

## 1. Notification system rewrite (relay + Hue)

### Context

Over one extended session, a series of real bugs were found and partially fixed in this repo's
shell-script-based agent/system notification pipeline ("relay" + Hue light pulses). Several fixes shipped
and are currently live. The human wants those fixes re-examined rather than trusted, two behavioral gaps
closed (a genuinely "progressive" notification flow, and Hue pulses that coordinate with notification
state instead of firing blind), and -- separately and more fundamentally -- the whole pipeline rebuilt in
a more capable language with real async and a rigorous, TDD-discovered, boundary-tested architecture (see
section 8).

### Current state: the pipeline as it exists today

*(Verified by direct file inspection in the session that produced this brief -- re-verify before relying
on any of it.)*

| Path                                            | Purpose                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| ----------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `dot_local/bin/executable_relay.sh`             | Fan-out core. Sends one notification to up to three channels: moshi (phone push), a Hermes webhook -> Discord `#relay`, and a local clickable `terminal-notifier` banner. Gates the phone channel on desk-vs-away presence via `ioreg`/`HIDIdleTime`. Trims long text to a sentence boundary for the phone/local preview; sends the Hermes/Discord copy untrimmed.                                                                        |
| `dot_local/bin/executable_relay-agent.sh`       | Builds the actual notification text from a Claude/Codex hook's JSON stdin payload -- extracts the assistant's last reply from the transcript, optionally hands it to `codex exec` for a short state-classified summary -- then calls `relay.sh`.                                                                                                                                                                                          |
| `dot_local/bin/executable_relay-codex-hooks.sh` | Idempotently merges relay's `Stop`/`PermissionRequest` entries into the live `~/.codex/hooks.json` (no chezmoi-managed source template exists for that file -- this script *is* the source of truth), preserving herdr's own `SessionStart` entry.                                                                                                                                                                                        |
| `dot_local/bin/executable_hue-pulse.sh`         | Pulses a Hue room green (success) or red (failure) through a brightness heartbeat via the `openhue` CLI, then restores every light's prior state. Serializes concurrent callers through an atomic `mkdir` lockfile.                                                                                                                                                                                                                       |
| `dot_local/bin/executable_claude-stop-pulse.sh` | Claude Code's `Stop` hook target for the light: pulses Hue only if the just-finished session ran >=5 minutes (tracked via a `/tmp/claude-session-<id>-start` marker file written by a separate `UserPromptSubmit` hook). **Currently has zero test coverage** -- no `test/claude-stop-pulse.sh` exists.                                                                                                                                   |
| `dot_bashrc.tmpl` (lines ~282-323)              | A parallel, independent notifier for long-running *shell commands* (not agent turns), via `bash-preexec`. >=60s -> local-only `relay.sh --local-only`; >=300s -> full fan-out (`relay.sh` + `hue-pulse.sh`). Skips a hardcoded list of interactive TUIs (\`vim                                                                                                                                                                            |
| `private_dot_claude/modify_settings.json`       | The chezmoi modify-template that (among other things unrelated to notifications) forces Claude Code's `hooks` block: `Stop` -> `claude-stop-pulse.sh` (sync) then `relay-agent.sh done` (async); `Notification[permission_prompt]` -> local `alerter` (sync) then `relay-agent.sh blocked` (async); `PostToolUse[AskUserQuestion]` -> `relay-agent.sh asked` (async); `PostToolUse[ExitPlanMode]` -> `relay-agent.sh plan-ready` (async). |
| `dot_config/relay/private_auth.json.tmpl`       | Renders the 0600 `~/.config/relay/auth.json` (moshi + Hermes secrets, from KeePassXC) that `relay.sh` reads at runtime. Never on argv/env.                                                                                                                                                                                                                                                                                                |

### Existing tests (`test/`)

`relay.sh`, `relay-agent.sh`, `relay-codex-hooks.sh`, `relay-hermes-route.sh`, `hue-pulse-lock.sh` --
five files, all plain executable shell scripts using hand-rolled assertions, no test framework.
`just test` runs every `test/*.sh` via `bash "$t" || exit 1` in a loop -- not gated on the test file's
own executable bit, exits on first failure. Whatever language you choose, `just test` still needs to end
up running your new tests as part of that same loop (a thin wrapper script invoking your language's test
runner is one obvious way to reconcile this with idiomatic test layouts).

### The one existing precedent for a compiled binary in this repo

`dot_local/share/herdr/plugins/herdr-smart-nav/` -- a Rust crate (`Cargo.toml`, `src/main.rs` with inline
`#[cfg(test)] mod tests`), built by
`.chezmoiscripts/run_onchange_after_57-build-herdr-smart-nav-plugin.sh.tmpl` via
`cargo build --release --locked`, run in place (binary stays at `target/release/` inside the plugin dir,
not copied to `~/.local/bin`). Toolchain comes from a dedicated `rustup` bootstrap script
(`run_once_before_20-install-rustup.sh.tmpl`) -- not the Nix flake, not Homebrew. `run_onchange`
re-triggers by hashing the plugin's source files via `{{ include "..." | sha256sum }}` in the script's
own comment. `target/` is gitignored (`dot_local/share/herdr/**/target/`). `*.rs` files are invisible to
`scripts/lint.sh`; a `Cargo.toml` would get swept by the generic `taplo` finder unless excluded. Nothing
currently runs `cargo test`/`clippy`/`fmt` automatically anywhere.

**This is precedent, not mandate.** Pick whatever language genuinely fits best -- Rust, Go, a scripting
language with real async, something else entirely. If you do pick Rust, this is the pattern to mirror for
toolchain provisioning and chezmoi build/link wiring.

### What's wrong / unverified (treat ALL of this as unresolved, including the "fixed" items)

Four things were changed and shipped in the current bash implementation this session. Do not trust
they're fully correct -- re-derive all four from a spec, with tests that would have caught the original
bugs, in whatever the new architecture is:

1. **Redundant header in the notification body.** Used to repeat `state - project` (already in the title)
   ahead of the content, wasting the phone/banner preview space. Changed to lead with the actual
   summary/branch instead.
1. **Mid-sentence truncation on the phone/banner preview.** Long summaries got cut wherever the character
   limit landed. Changed to trim at the last complete sentence within ~260 chars; the Discord/Hermes copy
   stays untrimmed.
1. **No desk-vs-away awareness.** The phone push fired unconditionally. Changed to check macOS input-idle
   time (`ioreg`/`HIDIdleTime`) and skip the phone push below a ~10-min idle threshold -- local banner
   and Discord log fire either way. Chosen because idle time is independent of display/system sleep
   state, composing cleanly with an existing "never sleep" power-management plan for this Mac -- see
   `docs/superpowers/plans/2026-07-01-dresden-never-sleep-power-policy.md` if relevant to your
   presence-detection design.
1. **Hue pulse blocked the Claude `Stop` hook and had no concurrency protection.** Synchronous call (~5s
   hook stall), zero locking, so two near-simultaneous pulses could interleave their `openhue` calls.
   Changed to fire detached, plus an atomic `mkdir`-based lock so concurrent pulses serialize.

### The bug that's NOT yet fixed: repeated, content-less notification spam

Real, needs a structural fix. Root cause, precisely:

**Why every notification looked identical (`state - done`, no content):** `relay-agent.sh` reads the
Claude session's transcript JSONL at the moment the `Stop` hook fires. On a `claude --remote-control`
session driven from the mobile app, the hook fires **before the transcript file has been flushed with the
turn's content** -- extraction finds nothing, falls back to a bare state word. Reproduced directly: the
same extraction against the *finished* transcript returned the correct 251-character reply; at hook-fire
time on the live session it returned empty.

**Why it repeated for hours:** each turn fires its own `Stop` hook independently. A
`claude --remote-control` session left running in a career-side project directory kept receiving small
mobile edits through the previous evening, so it kept firing `Stop` -> notification, once per turn --
each empty for the reason above. Evidence: `~/.hermes/logs/gateway.log` (every relay -> Discord delivery
logged with timestamp + body length) showed dozens of near-identical short messages clustered in bursts,
lining up with that session's own transcript turns.

**What a real fix needs to address (design problem, not a prescribed solution):**

- The transcript-not-yet-flushed race -- wait/retry briefly for content, or find a different source of
  truth for "what just happened" that doesn't race the hook.
- Per-session noise -- should repeated notifications from the same session in a short window coalesce,
  debounce, or rate-limit? What does "the same notification" even mean for dedup purposes? (A real domain
  concept worth naming and testing directly.)
- Prove the fix with a test that reproduces the original failure mode (transcript missing the turn's
  content at fire time) and demonstrates it going green after the fix.

One likely-stale operational note: the specific runaway session (working directory
`career-campaign/luke-morrison-smith`) needed a human to kill it manually and may already be handled.
Don't hunt for that process -- it's a symptom, not the bug.

### New behavior being asked for

**"Progressive" notification system -- concrete spec from the human (verbatim intent, not a summary):**

1. Detect whether the human is at their computer or not.
1. **At the computer** -> deliver via the local desktop channel (today's `terminal-notifier` banner).
   **Away from the computer** -> deliver to the phone via moshi, and *not* to the computer.
   > The human's own wording said "send the notification to my computer via moshi" for the at-computer
   > case. Everywhere else in this system (and everywhere else in this brief) moshi is specifically the
   > phone-push channel, distinct from the local desktop notification. The interpretation above (at
   > computer -> local; away -> phone via moshi) is the reading that's internally consistent with the
   > rest of the architecture -- **confirm this with the human before building it**, in case moshi
   > actually supports a distinct "deliver to this computer" target that isn't documented elsewhere in
   > this brief.
1. **Hue pulse, gated on a *different* presence signal than "at the computer":** if a command or agent
   response takes longer than some threshold to complete, pulse the lights -- green for success, red for
   failure/error -- but **only if the human is physically in the house**. This is not the same check as
   "at the computer" (idle time): a laptop can be at a desk while the human is out, or the human could be
   home without being at that specific machine. This repo already has a Home Assistant integration (the
   `home-assistant` skill) -- that's the obvious existing source for real home-occupancy/presence data,
   rather than inventing a new detection mechanism from scratch. Also note the current implementation is
   asymmetric: the shell long-running-command notifier already pulses red on failure (exit code != 0),
   but the Claude `Stop`-hook path (`claude-stop-pulse.sh`) only ever pulses green -- there's no defined
   notion of "failure" for an agent turn yet (a thrown error? a blocked/stuck state? something else?).
   That gap needs a real answer, not just a threshold tweak.
1. **Every notification, regardless of the human's location, still gets logged to the Hermes `relay`
   webhook -> Discord `#relay`, unconditionally.** This one is not new -- it matches current behavior
   (the Hermes/Discord channel already fires unconditionally) -- but it's now an explicit, permanent
   requirement: the Discord log must never be gated on presence, only the phone/local/Hue channels are.

**You have the human's explicit permission to critically re-evaluate this spec, not just implement it
literally.** Their own words: "The model should reevaluate this notification behavior and try to improve
it if it feels like there are gaps or bad choices." If you find gaps (e.g. what exactly counts as agent
"failure," how reliably "in the house" can actually be detected, what happens when Home Assistant is
unreachable) or think a different design serves the same intent better, say so and propose it -- this is
exactly the kind of thing section "Read this first" above already told you to push back on.

### Requirements (direct from the human, not design choices)

- Real async where it earns its keep -- concurrent channel fan-out with proper timeouts/cancellation, not
  backgrounded shell jobs.
- Everything meaningfully testable in isolation -- the domain logic (should this fire on this channel,
  what does the body say, is this a duplicate) must be unit-testable without a real network call, a real
  `openhue`, or a real filesystem transcript read.
- Every piece of new logic is unit tested, discoverable via `test/` / `just test`.
- Deploys through chezmoi the way everything else here does.

______________________________________________________________________

## 2. Branch combination + incremental PR delivery

### Context and the exact current state (verified, will already be somewhat stale)

Three open PRs exist right now:

| PR  | title                                                                         | head branch                                                         | base                                                                                                      |
| --- | ----------------------------------------------------------------------------- | ------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| #31 | herdr migration, headless Tailscale, weekly Homebrew, and moshi notifications | `feat/cli-agent-tracking-workflow` (**this is the current branch**) | `main`                                                                                                    |
| #25 | feat(osquery): three-tier alerting reshape (page / digest / log-only)         | `feat/osquery-alerter-three-tier`                                   | `main`                                                                                                    |
| #24 | docs(osquery): v2 three-tier alerting design + implementation plans           | `docs/osquery-design`                                               | `main` (already the design doc PR #25 implements -- read for read-only context, don't edit osquery logic) |

**PR #31 / current branch:** GitHub shows 68 files / +7788 / -1578. **Local `HEAD` is 38 commits ahead of
`origin` plus a dirty working tree** (an in-progress, uncommitted Hermes-config-encryption migration --
see section 9) -- local `main...HEAD` is actually 92 files / +9506 / -1908, 129 commits ahead of `main`.
**Commit or otherwise resolve the dirty tree before doing anything else with this branch** -- don't lose
that in-progress work.

**PR #25 (`feat/osquery-alerter-three-tier`):** 45 files / +2264 / -213, 64 commits ahead of `main` --
but its merge-base with `main` is *older* than current `main`'s tip, meaning **this branch needs rebasing
onto current `main` before it can be cleanly combined with anything.**

**File overlap between the two branches:** exactly 2 files -- `.chezmoiignore` and `justfile`. Small, but
real conflict risk to plan for, not assume away.

**No existing precedent** anywhere in this repo's history for a
combine-into-staging-then-split-into-small-PRs workflow -- the established convention is one branch, one
PR, straight to `main`. Treat what follows as genuinely new process for this repo, not something to go
looking for prior art on.

**Use `gh-axi` (not raw `gh`) for every GitHub operation this workstream needs** -- listing/viewing PRs,
creating the combined branch's PR, opening each small follow-up PR, checking CI status, etc.
`private_dot_claude/CLAUDE.md` (global CLAUDE.md source) states this explicitly as of this brief being
written; `gh` itself stays installed underneath as `gh-axi`'s own dependency, never invoked directly.

### The requested workflow

1. Take this branch (PR #31, `feat/cli-agent-tracking-workflow`, including whatever this brief's other
   workstreams add to it) and `feat/osquery-alerter-three-tier` (PR #25), and combine **all** the work
   from both (every feature, nothing dropped) into one integration branch containing the union.
1. Push that combined branch as one PR, but **do not merge it to `main`.** It's a staging artifact, not a
   shippable unit.
1. From that combined superset, carve out a sequence of smaller PRs. Each one must be **a complete unit
   of work that stands on its own** -- fully wired in, fully working, reviewable in isolation. **No PR
   may ship code that isn't actually used** -- no dead code, no half-wired feature waiting on a later PR
   to activate it. This likely means re-ordering/re-splitting commits by feature rather than by original
   authorship order.
1. Ship these small PRs one at a time (or in whatever order makes sense) until every behavior in the
   original giant combined PR has landed via a small PR the human could review quickly.
1. You may pause at any point and ask the human to run `chezmoi apply` (never run a bare one yourself
   from automation -- see the KeePassXC-gated-template list in this repo's `CLAUDE.md`).

### Open question for you

`nvim-overhaul` (section 6) is a third piece of pre-existing, unpushed work in this same repo (10 commits
on its own branch, not yet a PR). Is it in scope for this same combine-and-split effort, or should it
stay a separate, later initiative? The human's instruction named only PR #31 and PR #25 explicitly --
decide whether folding in a third stream helps or just adds risk, and say why.

______________________________________________________________________

## 3. Whole-repo creative sweep

Separately from the notification rewrite and the branch-combination workflow, the human wants a genuine
sweep of the entire dotfiles repo: improve, refactor, or remove **any** code in it, with real creative
license. This repo's own root `CLAUDE.md` (long, detailed, already governs code style, security, and
architecture conventions in depth) is the ground truth for what "improved" means here -- read it before
proposing changes. "Creative license" does not override "investigate before implementing" or "no
unsolicited docs" from the Standing Rules above, and it does not extend to osquery's design/logic (see
the refined guardrail above). Whether this sweep becomes a bounded backlog you hand back for
prioritization, or something you actively execute alongside the other workstreams, is your call -- keep
whatever you ship reviewable as discrete, self-contained PRs either way (section 2's rule applies here
too).

______________________________________________________________________

## 4. Possible shell replacement: bash -> nushell

The human floated, as an example of the scale of change now in scope: replacing bash as the interactive
shell with [nushell](https://www.nushell.sh/book/line_editor.html), including reimplementing all current
keybindings in nushell's line editor. **This is offered as an example of the kind of large, justified
change now authorized -- not a mandate to do it.** If you think it's the right call, you have license to
pursue it; if you think it isn't, say so and why.

This would be one of the highest-blast-radius changes possible in this repo. Before proposing it,
understand exactly what's currently riding on bash specifically:

- `dot_bashrc.tmpl`'s **Bashrc Init Ordering** section (see this repo's `CLAUDE.md`) -- a carefully
  sequenced interactive-only init block (`bash-completion@2`, `bash-preexec` sourced explicitly before
  `atuin init` because of `preexec_functions`/`precmd_functions` registration order, then
  `PROMPT_COMMAND` writers in a specific order: direnv -> starship -> zoxide -> atuin), plus a **Homebrew
  shellenv caching** mechanism built specifically around bash's non-interactive login-shell behavior
  (`bash -lc`, `ssh host cmd`).
- The long-running-command notifier (section 1's table) is built on `bash-preexec`'s
  `preexec_functions`/`precmd_functions` arrays specifically to avoid clobbering atuin's
  `DEBUG`-trap-based recording -- a nushell equivalent needs its own mechanism, not a direct port.
- Atuin, starship, zoxide, direnv, carapace all currently integrate via bash-specific init lines in
  `dot_bashrc.tmpl` -- each has its own nushell integration story (some solid, some not) that needs
  verifying, not assuming.
- All current keybindings (documented in this repo's keybindings config) would need a nushell line-editor
  equivalent.

If you pursue this, treat it as its own large, carefully-tested, incrementally-shippable effort (see
section 2) -- not something folded silently into an unrelated PR.

______________________________________________________________________

## 5. Install `stonerl/Thaw`

[`stonerl/Thaw`](https://github.com/stonerl/Thaw) is a macOS 26+ menu bar manager (a fork of the
discontinued "Ice" project) -- verified via its README: hide/show/organize menu bar items, drag-and-drop
arrangement, "always-hidden" sections revealed on hover/click/scroll, appearance customization (tints,
shadows, borders), hotkeys, and profile support for different menu bar layouts. Installable via Homebrew
(`brew install thaw` or `brew install thaw@beta`) or manual download.

**Flag for you to resolve:** the human asked for this "with useful plugins," but Thaw's README does not
describe a plugin/extension architecture as of this check. Either that's changed since, the human means
something else (useful *profiles*/configuration, perhaps), or it's worth a quick clarification. Don't
invent a plugin system that doesn't exist to satisfy the literal wording.

This repo already has a documented workflow for exactly this kind of addition -- see "Homebrew install
workflow (for AI agents)" in this repo's `CLAUDE.md`: install immediately via `brew install --cask` (or
formula, whichever Thaw actually is), then add it to `.chezmoidata/system_packages_autoinstall.yaml` in
the correct alphabetized list, then remind the human to run `chezmoi apply` when appropriate. Follow that
convention rather than inventing a new one.

______________________________________________________________________

## 6. Neovim: re-evaluate and finally implement the existing nvim-overhaul design

### This already exists -- it is not a fresh design task

There is a dedicated git branch `nvim-overhaul` (present locally and on `origin`) and a worktree at
`~/.paseo/worktrees/1sk17y2x/nvim-overhaul` containing **10 unpushed commits**. Three design generations
already exist:

- `docs/superpowers/specs/2026-05-24-nvim-overhaul-design.md` -- v1 (on `main`)
- `docs/superpowers/specs/2026-06-02-nvim-overhaul-design-v2.md` -- v2, supersedes v1 (on `main`)
- `docs/superpowers/specs/2026-06-02-nvim-overhaul-reassessment.md` -- a 9-agent adversarial review of v1
  (on `main`)
- `docs/superpowers/specs/2026-06-03-nvim-overhaul-design-v3.md` -- v3, supersedes v2, adds a
  `custom_api/` redesign and retires `delegate.lua` (**only on the `nvim-overhaul` branch, not on
  `main`**)
- `docs/research/2026-06-03-nvim-coding-agent-integration.md` -- research backing v3's decision to retire
  `delegate.lua` in favor of `coder/claudecode.nvim` (**only on the `nvim-overhaul` branch**)

Nothing has been implemented. `~/.config/nvim` remains its own standalone git repo
(`git@github.com:webdavis/neovim-config.git`), entirely outside chezmoi, currently 7 commits ahead of its
own `origin/main` with one modified and two untracked files. It runs `nvim v0.12.3`, 52 Lua files
totaling ~6,528 lines, plugin-managed via `lazy.nvim` (not a distribution like LazyVim -- vanilla
`lazy.nvim` with that import commented out).

### What the human is asking for

1. **A complete re-evaluation of nvim-overhaul and the plans there.** Read v1, v2, the reassessment, and
   v3 (the last one only exists on the unpushed branch) in full. Do not assume v3 is correct just because
   it's the latest -- the human explicitly wants this critically re-evaluated, the same way this whole
   document invites pushback on everything in it. If v3's decisions (e.g. retiring `delegate.lua` for
   `coder/claudecode.nvim`) still hold up, say why; if not, propose better.
1. **Migrate the entire Neovim config into this dotfiles repo, tracked by chezmoi** -- flatten
   `~/.config/nvim` (currently the separate `webdavis/neovim-config` repo) into `dot_config/nvim/` here,
   per whatever the re-evaluated design settles on.
1. **Improve the config significantly for the modern development world** -- the existing specs already
   catalog known bugs (~17) and stale plugins (6-8) and target startup-time reduction; treat those as a
   floor, not a ceiling. Research current best practice rather than assuming the 2026-05/06 specs are
   still state-of-the-art.

### Open question for you

Is this its own independent effort, sequenced separately, or does it fold into the same combine-and-split
PR workflow from section 2? The specs already existing across `main` and an unpushed branch is exactly
the kind of situation section 2's workflow was designed for -- but Neovim config work is a genuinely
distinct domain from the shell-notification/osquery work in PR #31/#25. Decide, and say why.

______________________________________________________________________

## 7. Cross-cutting open questions

These are explicitly yours to decide during brainstorming, not the human's to pre-answer:

- **Language for the notification rewrite** (section 1). Fully open -- Rust has precedent in this repo,
  but justify whatever you pick against this project's actual scale and needs.
- **Scope boundary for the notification rewrite** -- does it replace the whole runtime pipeline (relay +
  relay-agent + hue-pulse + claude-stop-pulse, one program, Hue coordinating directly with notification
  state) or start narrower? `relay-codex-hooks.sh` (a one-shot config merger, not a runtime notifier) and
  the `dot_bashrc.tmpl` `bash-preexec` block (must stay bash unless section 4 happens) are worth deciding
  in/out explicitly.
- **"Progressive" notification mechanics** (section 1) -- the human specified the intended *behavior*
  concretely (at-computer -> local, away -> phone, Hue gated on being physically home, Discord always
  logs), but not the *detection mechanism* for either presence signal. Confirm the moshi-target reading
  flagged in section 1, decide how to detect "at the computer" (idle time? something else?) and "in the
  house" (Home Assistant occupancy, most likely), and define what "failure" means for an agent turn
  (currently undefined -- only the shell-command path has a failure concept today).
- **Depth of the whole-repo sweep** (section 3) -- backlog-only vs. actively executed.
- **Whether `nvim-overhaul` (section 6) and/or the shell replacement (section 4) fold into the
  combine-and-split branch workflow (section 2)**, or stay separate, sequenced efforts.
- **Overall sequencing across all six workstreams** -- what order serves fastest, safest, most reviewable
  delivery? The human did not specify an order; don't assume the order they appear in this document is
  the intended priority.

______________________________________________________________________

## 8. Reference strategy: essential-feed-case-study (binding on *how*, not *what*)

This is a hard requirement on the notification rewrite's (and arguably any new substantial code's) design
and testing discipline -- modeled on `essentialdevelopercom/essential-feed-case-study` (a Swift iOS
project on GitHub -- study it directly if useful; what follows is the human's own summary of its
strategy, treat it as authoritative). Its language and platform don't transfer here. Its *discipline*
does, and this section is explicitly **not** one of the things open to pushback -- the practice described
below is required; only its concrete translation into whatever language/architecture you choose is yours
to design.

> ## Software Testing and Design Strategy
>
> The strategy in this repository is a disciplined, specification-driven approach in which **Test-Driven
> Development is used not just to verify behavior but to *discover* the design**. Tests are written first
> from the specifications (narratives, acceptance criteria, and use cases with explicit happy and sad
> paths), and the modular architecture emerges as a consequence of writing code that is easy to test in
> isolation. TDD applied under **SOLID principles** naturally pushes the system toward small,
> single-responsibility components that depend on abstractions, and the resulting structure is what the
> architecture diagram depicts.
>
> ### Design discovered through TDD
>
> The architecture is organized into independent **feature scenes** (a Feed scene and a Comments scene)
> that share the same layered shape but know nothing about each other. Within each scene,
> responsibilities are cleanly separated into distinct layers: networking (API) and persistence (Cache)
> at the boundaries, a plain domain model in the middle (the feed image / comment types), a presentation
> layer, a UI layer, and the platform UI framework at the edge. Dependencies point inward toward the
> domain, and the layers communicate through abstractions rather than concrete types -- a direct
> expression of the dependency-inversion and single-responsibility principles that testing-in-isolation
> forces you to adopt.
>
> Crucially, the feature modules contain no wiring or global state. Instead, a **Composition Root** (the
> app's scene delegate plus per-feature UI composers) is the single place where concrete implementations
> are instantiated and injected together. Because collaboration happens through injected abstractions,
> each unit can be tested with test doubles, and the whole app can be assembled differently for tests
> than for production. This design is favored over tightly coupled, framework-dependent code precisely
> because it keeps the core logic testable, reusable, and platform-independent -- the testing strategy
> and the modular design reinforce each other.
>
> ### Strong separation of test types by scope
>
> Reflecting that modularity, tests are split into distinct targets by level and speed rather than lumped
> into one suite. Fast, isolated **unit tests** form the broad base, organized by feature and layer
> (networking, caching, presentation, shared infrastructure). **Integration tests** live in their own
> target and deliberately exercise the real persistence stack. **End-to-end tests** that talk to a live
> backend are isolated in yet another target. This favors clarity of failure and fast feedback -- the
> failing target signals the responsible layer -- and quarantines the slow, network-dependent tests so
> they never destabilize the fast suite.
>
> ### Testing through boundaries with test doubles
>
> Unit and presentation logic are tested in isolation using **test doubles** (spies and stubs) at the
> system's boundaries. This is favored over hitting real frameworks or infrastructure in most tests
> because it keeps them fast, deterministic, and flake-free, while also enforcing the abstraction-based
> design that makes the code framework-agnostic in the first place.
>
> ### Acceptance tests over broad UI automation
>
> At the top of the pyramid sit a small number of **acceptance tests** that drive the fully composed app
> through the composition root and swap out only the outermost boundaries (a network client toggled
> between online/offline, and a real store). A single test validates an entire user scenario --
> connectivity, offline cache fallback, empty states, cache expiration, and navigation. This is favored
> over large numbers of slow, brittle end-to-end UI-automation tests, because it exercises real
> composition and integration while staying fast and reliable.
>
> ### Snapshot testing for UI verification
>
> UI is validated with **snapshot testing** -- committed reference images compared against rendered views
> across states and appearances (e.g., light/dark). This is favored over asserting on low-level view
> properties or relying on manual visual inspection, giving precise, repeatable regression detection
> while remaining deterministic.
>
> ### Automated quality safeguards in every test
>
> Cross-cutting checks are standardized through shared helpers: **automatic memory-leak tracking**
> asserts objects deallocate after each test (catching retain cycles as a matter of course rather than
> via separate profiling), and **localization tests** ensure user-facing strings are properly localized.
> Embedding these into ordinary runs is favored over occasional manual audits.
>
> ### CI as the enforcement mechanism
>
> All of this is enforced automatically on every pull request through **parallel CI pipelines that run
> the suite on more than one platform**, with the **Thread Sanitizer enabled**. Running on multiple
> platforms both enforces the framework-independence of the core logic and widens coverage, while
> sanitizer-enabled runs catch concurrency and race-condition bugs during normal testing rather than
> leaving them to production.
>
> ### In short
>
> TDD under SOLID is used to *drive out* a modular, layered, dependency-injected design -- independent
> feature scenes wired only at a composition root -- rather than to merely test a pre-existing one. On
> top of that design, the strategy favors many fast, isolated, framework-independent unit tests over slow
> integrated ones; test doubles at boundaries over real infrastructure; a few high-level acceptance tests
> over sprawling brittle UI automation; snapshot comparison over manual visual checks; and continuous,
> sanitizer-backed, multi-platform CI over manual verification.

Concretely, translate this to the notification system as something like (your call on exact shape, this
is illustrative, not prescriptive):

- **Tests discover the design, not the reverse.** Write the test for "does this notification get sent to
  the phone channel when the user is away" before the code that decides it, and let the need for that
  test in isolation force the presence-check into its own testable unit behind an abstraction.
- **Independent "scenes" with no shared state, wired only at a composition root.** Separate the "decide
  what to send and where" domain logic from each channel's delivery mechanism (moshi HTTP client, Hermes
  HTTP client + HMAC signer, local OS notification, Hue light controller), with a single place that wires
  concrete implementations for production and swaps in test doubles for tests.
- **Boundaries get test doubles, not real I/O, in the fast unit suite.** No real `curl`, `openhue`, or
  filesystem transcript read in the bulk of your tests -- those go behind an interface/trait/protocol
  with a fake for tests.
- **Tests organized by level and speed**, matching this repo's needs at its scale: a broad base of fast
  unit tests, and a small number of higher-level tests exercising the fully-wired composition against
  fake versions of only the outermost boundaries (a fake moshi/Hermes HTTP server, a fake Hue
  controller). A full CI sanitizer/multi-platform matrix is very likely overkill for this project's scale
  -- say so explicitly if you agree, rather than cargo-culting it in.
- **No untested logic.** `claude-stop-pulse.sh`'s session-duration gate has zero test coverage today --
  exactly the kind of gap this discipline should make impossible to leave behind.

______________________________________________________________________

## 9. Operational items -- explicitly out of scope for whatever you implement

These need the human directly and should not be silently resolved:

- A live runaway session was killed manually during the debugging session (or needs to be, if somehow
  still running) -- not a code change, don't go hunting for it.
- A live osquery configuration edit made during that session was reverted at the human's request; the
  refined guardrail above (Standing Rules) governs osquery going forward.
- **The current branch's working tree is dirty right now** with an in-progress, uncommitted effort to
  migrate `~/.hermes/config.yaml` to a fully chezmoi-tracked, age-encrypted single source of truth
  (possible files: `dot_hermes/encrypted_private_config.yaml.age`, `.chezmoidata/hermes.yaml`, new
  `run_onchange`/`run_after` scripts, `test/hermes-config-encrypted.sh`, `test/hermes-config-routes.sh`).
  Resolve/commit this before any branch-combination work (section 2) -- do not lose it. Also: once that
  migration lands, `dot_hermes/modify_private_config.yaml.tmpl` and `private/relay-hermes-route.yq` are
  slated for deletion -- check whether that's already happened before assuming those two files still work
  the way this brief's section 1 describes the Hermes route mechanism.
- **The `~/.claude/skills/` cleanup and `gh-axi`/`chrome-devtools-axi` install** (see
  `private_dot_claude/CLAUDE.md`'s Tool preferences and the fact both are now installed) were
  prerequisite setup done before this brief was written -- not something for you to redo, just context
  for why those tools are already available and preferred.
