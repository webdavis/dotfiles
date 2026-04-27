______________________________________________________________________

## title: Adding Declarative macOS Defaults Management to a chezmoi+bash+Nix-flake Stack on macOS 26 Tahoe date: 2026-04-26 mode: deep audience: dotfiles maintainer with a locked-in bash + chezmoi + KeePassXC + Nix-flake-per-project toolchain target_os: macOS 26.2 (Tahoe), Apple Silicon status: final

# Adding Declarative macOS Defaults Management to a chezmoi+bash+Nix-flake Stack on macOS 26 Tahoe

## Executive Summary

The cleanest way to add `defaults`-based macOS settings management to the user's existing
chezmoi+bash+Nix-flake stack is the **native chezmoi pattern**: a single
`.chezmoidata/macos_defaults.yaml` data file consumed by a
`.chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl` runner, paired with a sibling
`dot_local/bin/macos-defaults-drift.sh` script wired into the justfile for on-demand drift checking. This
pattern is a near-perfect mirror of the user's already-shipping `system_packages_autoinstall.yaml` +
`run_onchange_before_10-system-packages.sh.tmpl` Brewfile workflow, requires no new tooling, hashes the
rendered template content for per-machine idempotency [1], runs in plain bash (the user's locked-in
shell), and integrates with the existing lint/format pipeline at zero cost.

The two competing alternatives both add cost without paying it back. **dsully/macos-defaults** (Rust,
YAML-driven) is the most polished third-party tool in the niche — it ships a single static binary,
supports `currentHost` and per-domain `kill` lists, and has dedicated drift-detection plumbing — but its
`--dry-run` flag was silently broken across all tagged releases until a fix landed in HEAD on 2026-04-26
(no release yet) [2], it has bus-factor 1, and it adds a runtime dependency for what is fundamentally a
30-line bash loop [3]. **nix-darwin's `system.defaults` module** exposes ~211 typed keys [4] but requires
a Nix install, sudo, a `system.primaryUser` declaration [5], and currently has at least four open macOS
26 Tahoe regression issues (#1513, #1544, #1577, #1621) [6,7,8,9]; running `darwin-rebuild` solely to
flip Dock keys is the definition of overkill, and zero public examples exist of anyone driving it from
chezmoi for that purpose [10].

A non-negotiable caveat applies to *every* approach: macOS 26 Tahoe has tightened TCC (Transparency,
Consent, Control) and SIP enforcement to the point where ~30% of "make a fresh Mac feel like home"
settings — Bluetooth, Remote Login, Screen Sharing, Full Disk Access grants — can no longer be flipped
via `defaults write` regardless of the wrapper [11]. Those require signed `.mobileconfig` profiles or
manual System Settings clicks, and any automation that pretends otherwise will fail silently. Honest
scope: this recommendation handles the ~70% of settings that `defaults` still owns cleanly, and
explicitly defers the rest to a documented manual setup checklist.

## Introduction

### Scope

This report evaluates approaches for adding declarative, idempotent, version-controlled macOS `defaults`
management to an existing chezmoi-based dotfiles repository. The user's hard requirements are: (1)
bootstrappable on a freshly-minted macOS 26 system with no manual steps beyond the chezmoi init they
already perform; (2) version-controlled in git and pushable to GitHub; (3) idempotent — re-runs must not
break or destructively re-apply already-applied settings, ideally with drift detection; (4) one-file
editing ergonomics for adding or removing a setting.

The user's toolchain is locked: bash for scripts (no zsh/fish), chezmoi (>=2.62.3) with KeePassXC
integration for secrets, Nix flakes per-project (not nix-darwin for system management), and a justfile +
`scripts/lint.sh` invoking shellcheck/shfmt/mdformat/nixfmt/taplo/jq/yq via
`nix develop .#run --command`. The recommendation must respect this toolchain — wholesale migration to
nix-darwin is explicitly off the table per user instruction.

### Methodology

Eight parallel WebSearches and three parallel deep-dive sub-agents gathered evidence across five
evaluation axes: (a) the canonical bash approach (mathiasbynens/dotfiles `.macos`); (b) declarative
third-party tools (dsully/macos-defaults, koenrh/deft, RATIU5/fjrd, zero-sh/apply-user-defaults); (c)
nix-darwin's `system.defaults` module catalog and bootstrap cost; (d) chezmoi's native `run_onchange_*`
and `.chezmoidata/` primitives; (e) macOS 26 Tahoe-specific quirks around TCC, SIP, and the `defaults`
command. Sub-agents returned structured evidence as `{claim, evidence_quote, source_url, confidence}`
blocks. Major claims required 3+ independent sources where available; single-source claims (the dsully
`--dry-run` fix landing today, the negative finding that no public chezmoi+nix-darwin defaults-only
precedent exists) are flagged inline.

### Key assumptions

The recommendation assumes (a) the user maintains a small number (2-5) of macOS machines, not a fleet —
making MDM/Jamf overkill; (b) `defaults` settings are opt-in policy the user wants enforced across
machines, not just bootstrap-once-and-forget; (c) drift detection should be on-demand
(`just defaults-drift`), not continuous monitoring (no daemon); (d) the user is willing to maintain a
starter list of ~30 settings curated for their workflow rather than mirror a 200+-key catalog wholesale;
(e) settings requiring sudo (system-wide preferences in `/Library/Preferences/`) or TCC-gated paths will
be deferred to a separate documented manual checklist rather than bundled into chezmoi automation.

## Main Analysis

The seven findings below evaluate each viable approach against the user's four hard constraints
(fresh-Mac bootstrappable, git-tracked, idempotent + drift-aware, one-file-edit ergonomics) and against
the locked-in toolchain (bash, chezmoi+KeePassXC, Nix flakes per-project, justfile lint pipeline).
Findings 1-3 compare the candidate tools head to head; Finding 4 addresses the drift-detection gap;
Findings 5-6 cover macOS 26-specific quirks and privilege-boundary constraints; Finding 7 lays out the
concrete recommended file structure with a starter setting list.

### Finding 1: The native chezmoi pattern is the cleanest fit

The user's existing repository already demonstrates the exact pattern that should be used for `defaults`
management. The `.chezmoidata/system_packages_autoinstall.yaml` file declares Homebrew packages (taps,
formulae, casks, mas), and `.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl` consumes that
data to render a Brewfile and run `brew bundle --cleanup` whenever the data changes. This pattern is
mechanically identical to what `defaults` management needs: a versioned declarative data file, a
templated runner script, and chezmoi's hash gate handling idempotency.

### How chezmoi's run_onchange hash gate works

chezmoi's official documentation states unambiguously that "if the script is a template, the content is
hashed after template execution" [1]. The hash is stored in chezmoi's persistent state in the
`entryState` bucket keyed by target name; on subsequent invocations the script is only re-run if its
post-template-rendering SHA-256 differs from the stored value [12]. Naming conventions and source-state
attributes (the `run_onchange_`, `before_`/`after_`, and ordering-numeral semantics) are documented at
the source-state-attributes reference [45], and `.chezmoidata/` is documented as a special directory
whose YAML/TOML/JSON contents merge into the root template-data dictionary in lexical order [44]. This
means a template wrapped in `{{ if eq .chezmoi.os "darwin" }}...{{ end }}` produces an empty rendered
body on Linux (which chezmoi correctly treats as "no script to run" — the maintainer confirmed this is
reliable and intentional in discussion #4555 [13]) and a populated body on macOS (which produces a
different hash per machine if any per-machine data is interpolated, but stays stable across reruns on the
same machine).

Critically, this hash is taken *after* the template engine substitutes `.chezmoidata/macos_defaults.yaml`
values into the script body. So the runner script's text changes whenever the YAML data file changes,
which means editing the YAML — adding a setting, removing one, changing a value — automatically triggers
re-execution on the next `chezmoi apply`. There is no need for the runner to maintain its own
change-detection logic; chezmoi already does it.

### Why run_onchange beats run_once here

chezmoi's maintainer twpayne explicitly recommends `run_onchange_` over `run_once_` for system
configuration: "Generally speaking, you should use run_onchange\_ script unless you know that you have a
good reason to use a run_once\_ script" [14]. The reason is that `run_once_` records each unique
content-hash it has ever seen and refuses to re-execute any of them. So if a user changes a Dock setting
from `false` to `true`, then later changes their mind and reverts to `false`, `run_once_` will see "I've
already run a version with `false`" and skip — leaving the actual machine state at `true`.
`run_onchange_` doesn't have this problem because it only compares against the *last* successfully-run
hash, not the entire history.

For settings management this distinction is load-bearing. The user will edit `macos_defaults.yaml` over
time, and revisions will sometimes revert prior values. `run_onchange_` will correctly re-apply each new
revision; `run_once_` would silently skip reverts.

### Real-world precedent: cweagans/dotfiles

The closest precedent for the data-driven approach is **cweagans/dotfiles**, which keeps per-domain JSONC
files under `~/.config/macos/` and a runner template iterates them via `glob` with `jq` [15]. The runner
extracts a `restart` command from the JSON and `eval`s it to handle the `killall Dock`-style step. Five
other production chezmoi repositories use simpler variants — posquit0/dotfiles puts ~200 lines of literal
`defaults write` calls into `darwin/run_onchange_after_02_configure_macos_defaults.sh` (no template, OS
gating via subdirectory name) [16]; timriley/dotfiles uses a template guarded by
`{{ if (eq .chezmoi.os "darwin") -}}` for a small focused script [17]; smasato/dotfiles uses numeric
prefixes (`10-defaults`) for intra-`after_` ordering and a work-machine boolean for selective application
[18]; liby/dotfiles is the smallest example at ~15 lines with a runtime OS gate [19]; felixjung/dotfiles
wraps everything in `{{ if eq .chezmoi.os "darwin" }}` similarly [20].

The pattern with the most longevity is the one that *separates data from code*. Inlining 200
`defaults write` calls into a single bash script (the posquit0 / mathiasbynens approach) puts pressure on
the script as both data and execution. When you want to add or remove a setting, you grep through
bash-quoted strings; when you want to know what's set, you read code instead of data; when you want to
share a starter list with someone else, you have to disentangle their preferences from the executable
scaffolding. The cweagans-style data file plus thin runner reverses this: the YAML is the source of
truth, the runner is a stable ~30-line consumer that rarely changes, and adding a setting is a one-line
YAML edit.

### Why this matches the user's existing mental model

The user's `.chezmoidata/system_packages_autoinstall.yaml` already lives by this principle. Adding a
Homebrew formula is a one-line YAML edit; the runner script never changes. The proposed
`macos_defaults.yaml` + runner mirrors this exactly, lowering the cognitive cost of the new pattern to
near-zero — the user already knows how to maintain it because they already maintain the Brewfile
equivalent.

The lint pipeline absorbs the new file at zero cost: `yq` already runs over `.chezmoidata/*.yaml` (per
the user's CLAUDE.md and `scripts/lint.sh`), shellcheck already lints chezmoi script templates via the
`CI=1 chezmoi execute-template` render-then-shellcheck pattern (per the user's CLAUDE.md "Template
Shellcheck Workaround" section), and shfmt already formats them. No new lint targets, no justfile
additions for the runner itself.

### Idempotency without `defaults read`

A natural impulse is to make the runner script "smart" — read the current value with `defaults read`,
compare to the desired value, only write on mismatch. This is not done in the wild. A sample of eight
production chezmoi macOS-defaults repositories (posquit0, timriley, smasato, liby, samyakbardiya,
felixjung, jgoguen, fhemberger) shows that none use `defaults read` to skip writes; all call
`defaults write` directly [21]. The reasons are sound: `defaults` is overwrite-by-default and writes are
extremely cheap (microseconds, no network, no fork-exec overhead worth measuring); the read-then-write
logic introduces an extra subprocess call per setting plus error handling that doubles the runner's
complexity; chezmoi's hash gate already prevents the runner from executing at all unless the YAML data
has changed, so the cost of "wasted" writes is paid at most once per YAML edit anyway. Trust the wrapper,
keep the script dumb.

### Sketch of the runner

A minimal viable runner template, in plain bash, looks like this:

```bash
#!/usr/bin/env bash
{{- if eq .chezmoi.os "darwin" }}
set -euo pipefail

# Ensure System Settings can't fight us.
osascript -e 'tell application "System Settings" to quit' || true

{{- range .macos.defaults }}
defaults write {{ .domain | quote }} {{ .key | quote }} -{{ .type }} {{ .value | quote }}
{{- end }}

# Restart affected processes so changes take effect immediately.
{{- range .macos.killall }}
killall {{ . | quote }} 2>/dev/null || true
{{- end }}
{{- end }}
```

The companion data file `macos_defaults.yaml` contains nothing but two lists — a list of
`{domain, key, type, value}` records and a list of process names to `killall` after — both of which are
pure declarative data that the user edits without thinking about bash quoting or template syntax. This is
the entire mechanism.

### Finding 2: dsully/macos-defaults — promising but premature for this user

Of the third-party tools in this niche, dsully/macos-defaults is by far the most polished. It's a single
static Rust binary distributed as a Homebrew bottle for Apple Silicon [22], parses YAML input (the author
explicitly rejected TOML for being verbose with deeply nested maps) [23], and exposes a clean
three-subcommand CLI: `apply`, `dump`, `completions`. Its YAML schema handles the things a hand-rolled
bash script gets wrong: per-document `kill: ["Dock", "cfprefsd"]` lists for process restarts (only fired
if values actually changed) [24], a `current_host: bool` toggle that resolves paths under
`~/Library/Preferences/ByHost/{domain}.{HW-UUID}.plist` correctly [25], a `sudo: bool` flag for
`/Library/Preferences/` writes, automatic detection of sandboxed-container plists (it falls back to
`~/Library/Containers/{domain}/Data/Library/Preferences/` if present), and pre-write backups to
`{path}.prev` for every modified plist. The schema also supports two power-user markers: `"!": {}` inside
a dict for overwrite-mode (delete keys not specified — the README explicitly warns this is dangerous),
and `"..."` inside arrays for splice-in-existing-values semantics treating arrays as sets [22].

### Why it's the *almost*-right tool

Drift detection is one of the user's stated requirements, and dsully has plumbing for it. The `apply`
subcommand accepts `--exit-code N`, which exits N if any value changed (and was written), 0 otherwise
[26]. Wired into a pre-commit hook or a `just defaults-drift` target, this gives you "tell me if my live
system has drifted from declared state" with a single command. Combined with `--dry-run`, it would be
exactly the closed-loop drift checker the user wants without writing one from scratch.

### Why it's not yet the right tool

The `--dry-run` flag was a defined CLI option but silently ignored across every tagged release: the tool
wrote changes regardless of `-d`, making it unsafe for read-only drift detection. This was filed as issue
#10 ("--dry-run flag silently ignored: apply writes changes regardless ... the tool reports what it would
change but actually applies the same changes, making dry-run mode unsafe for read-only drift detection")
and the fix landed in HEAD on 2026-04-26 — *the same day this report was written* [2]. There is no tagged
release yet that contains the fix; the Homebrew bottle still ships 0.3.0 from 2025-11-09 with the bug
intact. So as of today, the user has two options for trying the tool: (a) install the Homebrew version
and accept that drift-checking is silently broken; (b)
`cargo install --git https://github.com/dsully/macos-defaults` to get HEAD, accepting that they're now
pinned to an unreleased commit that may contain other unreleased changes and may not survive future
force-pushes.

The maintenance pattern compounds the concern. The repo has 78 stars, 2 forks, and contributor activity
is dsully (32 commits) followed by mmorella-dev (2), dependabot (1), pasteley (1) — bus-factor 1 by every
measure [27]. Releases land roughly once per year (v0.0.1 2023-07, 0.1.0 2024-03, 0.1.1 + 0.2.0 2024-09,
0.3.0 2025-11), so the gap between "fix lands in HEAD" and "fix in a brew bottle" can plausibly be 3-12
months. For a user who needs working drift detection now, that's a long wait against a known bug.

The repository also documents internal followup-work markers that hint at incompleteness: a comment in
`defaults.rs` flags the sudo backup path as not-yet-handled, and a branch for resolving paths under
`/Library/Preferences/` directly is currently commented out (lines 117-123 of `defaults.rs` are dead
code). The sudo path works in practice but the internal handling is acknowledged as incomplete.

### Why it adds cost the user doesn't currently pay

The runner script in Finding 1 is 30 lines of bash. The user already has shellcheck/shfmt linting bash,
and the project already gates everything through `nix develop`. Adopting dsully/macos-defaults adds a new
runtime dependency (the Rust binary), a new maintenance surface (track upstream releases, install via
Homebrew tap or `cargo install`, handle the case where the brew tap fork drifts from upstream), and a new
failure mode (the binary itself misbehaves) — for the privilege of not writing 30 lines of bash. The
cost-benefit is clearly underwater for this user's stack.

If the calculus changes — the user wants to switch all macOS defaults to a typed YAML schema that catches
typos at apply-time (`serde(deny_unknown_fields)` is enforced [22]), or wants the `current_host` / `kill`
semantics handled automatically, or wants the `.prev` backup-on-write safety net — then dsully becomes
much more attractive. None of those are current asks. Revisit annually; commit to native bash now.

### Other tools in the niche, briefly

**zero-sh/apply-user-defaults** is the direct prior-art for dsully — Rust, YAML, 73 stars, last pushed
2023-08-24, effectively dormant [28]. **koenrh/deft** (Go, 0 stars, last push 2026-04-16) has a dedicated
`diff` subcommand that's working today but is brand-new with no track record [29]. **RATIU5/fjrd** (Go,
TOML input, 1 star, beta status) supports loading config from a GitHub repo URL (`fjrd username/repo`)
which is interesting but immature [30]. **g0t4/mcp-server-macos-defaults** is an MCP server exposing
`defaults` to LLMs — not file-driven, not relevant [31]. **kevinSuttle/macOS-Defaults** (1.4k stars) is a
well-known imperative shell-script collection forked from the mathiasbynens canon, last pushed 2020-03 —
dormant. **jwbargsten/defbro** is single-purpose (sets default browser only). **Ansible's
`community.general.osx_defaults` module** is mature and idempotent, with recent updates adding
dict-merging support [49] — but Ansible's playbook + inventory + facts apparatus is heavyweight for a
single-machine personal-Mac context, and importing Ansible solely to manage defaults is the same overkill
argument that disqualifies nix-darwin.

The competitive landscape supports the conclusion: dsully is the leader of a small pack, and the leader
has a critical bug fixed only today with no release yet. None of the alternatives are mature enough to
recommend in 2026 for production use over native bash.

### Finding 3: nix-darwin is overkill and currently friction-heavy on macOS 26

nix-darwin's `system.defaults` module is the most catalog-complete declarative tool by a wide margin. The
module exposes ~211 typed `mkOption` keys across 23 sub-namespace files [4], with the largest namespaces
being `NSGlobalDomain` (53 keys), `dock` (44 keys), `trackpad` (22 keys), `finder` (21 keys), and
`WindowManager` (12 keys, including Stage Manager and tiling). For users already invested in Nix for
system management, this is a powerful proposition — type-checked configuration, evaluation-time error
catching, and integration with the broader nix-darwin ecosystem (services, LaunchAgents, packages).

For a user *not* otherwise on Nix for system management, the proposition collapses under bootstrap
weight.

### Bootstrap cost on a fresh Mac

Standalone use of nix-darwin's `system.defaults` requires: (a) a Nix implementation installed (Nix or
Lix; the project's README recommends the Lix installer for new users [32]); (b) a flake — the maintainers
explicitly state "we recommend that beginners use flakes to manage their nix-darwin configurations" [32];
(c) `sudo nix run nix-darwin/master#darwin-rebuild -- switch` to perform initial install, after which
subsequent rebuilds use `sudo darwin-rebuild switch` [32]; (d) a `system.primaryUser` declaration without
which any user-scope default (dock/finder/NSGlobalDomain/etc.) triggers an assertion failure [33]; (e)
`sudo` on every rebuild — recent versions of nix-darwin run all activation as root and per-user defaults
are written via `launchctl asuser "$(id -u -- ${user})" sudo --user=${user} --` [34].

The minimum viable defaults-only flake is roughly:

```nix
{
  inputs.darwin.url = "github:nix-darwin/nix-darwin";
  outputs = { darwin, ... }: {
    darwinConfigurations.dresden = darwin.lib.darwinSystem {
      modules = [{
        system.primaryUser = "stephen";
        system.stateVersion = 6;
        nixpkgs.hostPlatform = "aarch64-darwin";
        system.defaults.dock.autohide = true;
        # ...
      }];
    };
  };
}
```

That's ~1 GB of Nix store on a fresh Mac, plus 300-500 MB of nix-darwin + nixpkgs evaluation cache, plus
a passwordless sudoers entry for `darwin-rebuild` if you want chezmoi to invoke it without prompting —
all to gain a slightly nicer way to write `defaults write com.apple.dock autohide -bool true`. The
cost-benefit is severely lopsided for defaults-only use.

### Idempotency model and the activation gap

`system.defaults` runs `defaults write` unconditionally on every `darwin-rebuild switch` — there is no
diff/skip logic [34]. Drift correction is implicit (settings are unconditionally re-asserted) but there
is no diff/warn/dry-run plumbing. More importantly, settings written via `defaults write` often don't
take effect until logout, restart, or manual `cfprefsd` kill; this is a long-standing open issue (#658,
since 2023) tracking a missing `activateSettings -u` call that would make new settings take effect
immediately [35]. The activation script does `killall -qu <primaryUser> Dock` after dock changes (lines
154-157 of `defaults-write.nix`) but does NOT restart `cfprefsd`, `Finder`, `SystemUIServer`, or call
`activateSettings -u`. A specific instance: trackpad natural-scrolling settings
(`com.apple.swipescrolldirection`) are written correctly but don't take behavioral effect until
`cfprefsd` is manually restarted [36]. The recent migration of activation to root-only execution further
broke the previous `activateSettings` workarounds for user defaults, leaving open issue #1475 unresolved
[37].

A custom bash runner avoids both problems trivially: a single
`killall cfprefsd Dock Finder SystemUIServer` line at the end of the script handles activation; the
chezmoi hash gate handles drift correction.

### The ByHost gap

A documented limitation: nix-darwin has no declarative mechanism for the `defaults -currentHost` (ByHost)
domain [38]. This is a real gap — settings like ControlCenter Bluetooth visibility
(`com.apple.controlcenter Bluetooth -int 18`), per-keyboard remaps in `~/Library/Preferences/ByHost/`,
and Spotlight per-host indexing live in this domain. nix-darwin users today fall back to
`system.activationScripts` with raw `defaults -currentHost write` commands. So even if you adopt
nix-darwin, you'll still write some bash for ByHost settings — defeating part of the proposition.

### macOS 26 Tahoe friction

Filtering nix-darwin's open issues for "Tahoe" or "macOS 26" returns at least four currently-open or
recently-closed regressions: #1513 (firmlink stitching warning on root volume) [9], #1544
(`darwin-rebuild: command not found` on fresh Tahoe installs because PATH isn't refreshed) [7], #1572
(trackpad settings not applied without manual cfprefsd kill) [36], #1577 (`$TMPDIR` / `/tmp` symlink
behavior changed on macOS 26, breaking `nix flake update` — workaround:
`services.nix-daemon.tempDir = "/private/tmp"`) [8]. Issue #1621 (Tahoe modified `/etc/zshrc` and
`/etc/zprofile`, blocking activation with "Unexpected files in /etc" error) was fixed in nix-darwin 25.05
[6] — recent, but indicative of how often Tahoe changes break nix-darwin's assumptions.

For a user not otherwise running Nix, this is a non-trivial bootstrap risk on a brand-new macOS 26
machine — the exact scenario the user wants to support cleanly.

### No precedent for chezmoi-orchestrated defaults-only nix-darwin

A targeted GitHub code search for `"darwin-rebuild" chezmoi language:Shell` returns zero results [10].
The dominant pattern in the wild is to use chezmoi *and* nix-darwin in parallel, with chezmoi handling
templated dotfiles and nix-darwin handling system + packages [39]. No one has documented chezmoi
orchestrating `darwin-rebuild` for defaults-only use, which means this would be unblazed-trail territory:
the user would be the first to figure out the chezmoi-script-runs-darwin-rebuild dance, the first to
debug failure modes, and the first to maintain the integration as both projects evolve.

### Verdict

nix-darwin remains the right tool *if and when* the user adopts Nix for system management broadly —
packages, services, LaunchAgents, dev shells. As a defaults-only tool added incrementally to a
chezmoi+bash stack, it imports too much weight: a Nix install, sudo gates, system.primaryUser ceremony,
the activation gap for ByHost, and four open Tahoe-specific regressions to watch. Defer until the broader
Nix-for-system decision is on the table.

### Finding 4: Drift detection requires DIY for the native approach — and that's fine

The user's stated requirement #3 is "idempotent ... should detect drift (someone toggled a setting via
System Settings UI) and report or restore." chezmoi's `run_onchange_*` mechanism handles half of that
natively: re-running `chezmoi apply` on an unchanged YAML file is a no-op (the script body's hash hasn't
changed, so the script doesn't execute). What chezmoi does *not* do is detect the inverse case — that the
YAML still says one thing but the live system now says another because the user toggled a setting in
System Settings.

For that, a sibling read-only script is needed. The good news: it's ~50 lines of bash, sharing the same
data file as the runner.

### Drift-checker design

The drift checker reads `.chezmoidata/macos_defaults.yaml`, iterates each declared setting, calls
`defaults read <domain> <key>` for each, normalizes both the expected and actual values to a comparable
form (handling the bool `true` vs `1` mismatch, integer-vs-string coercion, etc.), and prints a unified
diff of declared-vs-actual. It exits non-zero if any drift is detected, making it suitable as a justfile
target (`just defaults-drift`) and optionally as a pre-commit hook to nag on commits.

Because the YAML data lives outside the chezmoi source tree (under `.chezmoidata/` it's data, not a
target file), this script can read it directly without going through chezmoi. The cleanest implementation
is to use `yq` (already in the user's `nix develop .#run` environment per CLAUDE.md) to extract entries:

```bash
#!/usr/bin/env bash
set -euo pipefail
DATA="${HOME}/.local/share/chezmoi/.chezmoidata/macos_defaults.yaml"
drift=0
while IFS=$'\t' read -r domain key type expected; do
  actual=$(defaults read "$domain" "$key" 2>/dev/null) || actual="<unset>"
  # Normalize bools: defaults stores true/false as 1/0
  case "$type" in
    bool|boolean)
      [[ "$expected" == "true" || "$expected" == "yes" ]] && expected=1
      [[ "$expected" == "false" || "$expected" == "no" ]] && expected=0
      ;;
  esac
  if [[ "$actual" != "$expected" ]]; then
    printf '%-40s %-40s expected=%s actual=%s\n' "$domain" "$key" "$expected" "$actual"
    drift=1
  fi
done < <(yq -r '.macos.defaults[] | [.domain, .key, .type, .value] | @tsv' "$DATA")
exit $drift
```

This script is a sibling of the runner, lives at `dot_local/bin/executable_macos-defaults-drift.sh`
(chezmoi's `executable_` prefix sets `+x`), and reads the same source-of-truth YAML. Adding a setting to
YAML automatically extends both the runner's apply set and the drift checker's check set — no
double-bookkeeping.

### Wiring into the justfile

The user's justfile already has short single-letter targets (`l` for lint, `d` for diff, `a` for apply).
A `just D` target for drift-check fits the pattern:

```just
D:
  nix develop .#run --command ~/.local/bin/macos-defaults-drift.sh
```

If desired, the checker can be wired into the user's existing pre-commit hook so commits to
`.chezmoidata/macos_defaults.yaml` trigger a drift check before allowing the commit (with an env-var
bypass for intentional overrides).

### What about restore-on-drift?

The user's requirement says "report or restore." The native pattern handles restore implicitly: running
`chezmoi apply` re-runs the runner script if the YAML hash differs from the last successful run, which
re-applies every setting. So the workflow for drift remediation is:

1. `just D` — see what's drifted.
1. Decide: was the drift accidental (revert), or did you actually want this setting changed permanently
   (update YAML)?
1. If revert: `just a` (chezmoi apply) won't re-run the runner because the YAML is unchanged. Workaround:
   `chezmoi apply --refresh-externals` or, more reliably,
   `touch .chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl && chezmoi apply` to bump the
   hash. (Or simpler: a `just defaults-reapply` target that calls `defaults write` directly via the same
   data file.)
1. If update: edit YAML, commit, `chezmoi apply` runs the runner with the new value.

Step 3 is mildly awkward — chezmoi correctly skips re-runs when content hasn't changed, but drift
correction needs an unconditional re-run. The cleanest fix is a separate
`dot_local/bin/macos-defaults-apply.sh` script (parallel to the drift checker) that's called directly by
`just defaults-apply` and bypasses chezmoi entirely. This makes the apply path explicit and removes the
"why didn't chezmoi apply re-run" footgun.

### Why dsully's `--exit-code N` isn't materially better

dsully/macos-defaults' drift-detection plumbing (assuming the HEAD `--dry-run` fix gets a release) gives
you `macos-defaults -d apply --exit-code 2 ~/.config/macos-defaults/` as a one-liner that exits 2 if
drift detected. The native bash script described above accomplishes the same thing in 50 lines that the
user owns and can modify. The dsully version is shorter at the call site but requires installing and
maintaining the binary; the bash version is longer at the source but lives in the same repo as everything
else and can be modified instantly without waiting for an upstream release. For a user who values "ease
of modification" (their stated requirement #4), the bash version wins.

### Limitations of the drift checker

The native checker handles the common case (single-value scalars: bools, ints, strings) cleanly. Arrays
and dicts are harder — `defaults read` returns multi-line plist syntax that requires
`plutil -convert json -o -` round-tripping for clean comparison. For the starter list of ~30 settings
(Finding 7), nearly all values are scalars; arrays are rare (Dock persistent-apps is a notable
exception). A v1 checker can warn-and-skip for non-scalar types and grow array/dict handling on demand —
the YAGNI principle applies.

### Finding 5: macOS 26 Tahoe-specific quirks affect every approach

macOS 26 Tahoe (released 2025-09-15 [48], currently 26.2 with build 25C56 on the user's machine)
introduced several changes that affect any `defaults`-based automation regardless of wrapper. Apple's
developer release notes [47] and the most recent security-content advisory [50] provide the canonical
version-by-version delta. Some are tightening enforcement, some are deprecation timelines, some are just
cosmetic state-management changes — but all are worth flagging in advance to avoid time-wasting debugging
sessions later.

### TCC tightening: the 30% that defaults can no longer touch

The most consequential change for automation: the perimeter of what `defaults write` (and `systemsetup`,
and `launchctl load`) can affect has shrunk meaningfully. Empirically: "Every approach you'll find
documented online — defaults write for Bluetooth, systemsetup for Remote Login, launchctl load for Screen
Sharing — hits a wall on Tahoe, either through TCC restrictions, SIP blocking launchd modifications,
silent failure, or state that doesn't survive a reboot" [11]. For the user's use case, this means:

- **Bluetooth toggles** via `defaults write com.apple.Bluetooth ...` may write the plist but not change
  behavior.
- **Remote Login** (`systemsetup -setremotelogin`) is now TCC-gated: invoking from Terminal requires
  Terminal to have Full Disk Access pre-granted, which itself requires manual System Settings clicks.
- **Screen Sharing** state via `launchctl load` may be gated by SIP.
- **Full Disk Access grants themselves** — the entries in System Settings > Privacy & Security > Full
  Disk Access — cannot be modified from the command line at all on macOS 26.1+ (the UI now only accepts
  `.app` bundles, not arbitrary binaries) [40].

The honest workaround is to keep these out of automation entirely. The user's chezmoi runner should only
manage `defaults`-friendly settings (Dock, Finder, keyboard, trackpad, screenshots, etc.) and a separate
`docs/MACOS_MANUAL_SETUP.md` should document the things that genuinely require manual System Settings
clicks — Full Disk Access grants for terminals, Remote Login enable, screen recording permissions,
accessibility permissions, etc. Pretending these can be automated leads to scripts that silently fail on
every fresh install.

A more sophisticated path is to use signed `.mobileconfig` configuration profiles, which can grant TCC
permissions and toggle restricted settings — but those require either a paid Apple Developer ID for
signing or manual install confirmation in System Settings, which moves the goalposts but doesn't
eliminate manual clicks. For a single-user personal Mac, the manual checklist is the right answer; for a
fleet, an MDM is the right answer; in between, configuration profiles signed with a self-signed cert are
an option but add operational complexity not justified at this user's scale.

### Apple Intelligence, Siri, Keyboard moved to declarative configurations

Apple's enterprise documentation states: "New declarative configurations for Apple Intelligence, External
Intelligence, Siri, and Keyboard settings are available, replacing the restrictions for these settings in
the com.apple.applicationaccess profile, which are now deprecated" [41]. Deprecation timelines: Apple
typically gives 1-2 years before removal. For chezmoi automation this matters because some settings that
*used* to be controllable via `defaults write com.apple.applicationaccess` now require declarative
configuration profiles. The user's starter list should avoid these areas (Apple Intelligence
enable/disable, Siri toggles); manage them manually.

### Software Update payload deprecation

"Software update management using mobile device management commands, restrictions, the
com.apple.SoftwareUpdate payload, and queries is deprecated and will be removed next year, with software
updates being managed and enforced using only declarative software update management going forward" [41].
For a non-MDM user this is a non-issue, but if the user ever wanted to script "auto-install macOS
updates", the path forward is declarative profiles, not `defaults write`.

### Rosetta deprecation warnings start in 26.4

"Starting in macOS Tahoe 26.4, users will be notified when they launch apps that use Rosetta that they
will not open in a future release of macOS" [41]. Not a defaults issue — but worth noting for the user's
broader bootstrap planning. Any x86_64 Homebrew bottles in the Brewfile will start showing Rosetta
warnings in late 2026, and will fail outright in macOS 27 or 28.

### IKEv2 algorithm deprecations

"Algorithms DES, 3DES, SHA1-96, and SHA1-160, as well as Diffie-Hellman groups less than 14, are no
longer supported for IKEv2 VPNs" [41]. Irrelevant to defaults management, but worth knowing if the user
ever sets up a VPN configuration.

### Hidden menu icon preference

A single user-discovered preference can disable most menu item icons in Apple's first-party apps,
restoring a cleaner pre-Tahoe look. The exact key isn't documented officially but is circulating in macOS
power-user circles. If the user wants this in their starter list, they'll need to grep the user's
preferences after toggling manually once. (This is a recurring pattern for Tahoe-specific settings — many
haven't been catalogued in macos-defaults.com yet.)

### /etc/zshrc and /etc/zprofile changed

macOS 26 modified `/etc/zshrc` and `/etc/zprofile`. nix-darwin tripped on this with "Unexpected files in
/etc, aborting activation" until 25.05 fixed the file-recognition logic [6]. For the user (who runs bash,
not zsh) this is irrelevant, but worth noting if anyone in the future audits the recommendation in the
context of a zsh-based stack.

### Practical impact on the recommendation

These quirks don't change the recommended pattern (native chezmoi + bash runner). They do change which
settings should be in the YAML data file. The starter list in Finding 7 deliberately avoids TCC-gated,
declarative-profile-required, and deprecated areas, sticking to settings that are stable across the macOS
24 → 26 transition and likely to remain stable into 27.

### Finding 6: SIP, sudo, and TCC boundaries — what to keep out of automation

Three privilege-related boundaries determine which settings belong in chezmoi automation versus a manual
checklist:

### sudo-required settings

`pmset` (power management) requires sudo; `systemsetup` requires admin privileges [42]. Settings written
to `/Library/Preferences/` (system-wide) require sudo; settings written to `~/Library/Preferences/`
(per-user) do not. Per the user's CLAUDE.md and global rules, automation should not sudo-prompt — chezmoi
scripts that require sudo will hang waiting for input from a non-interactive shell. The clean rule:
**only per-user `defaults write` calls go in the chezmoi runner**. System-wide settings
(`/Library/Preferences/com.apple.X` writes, pmset, systemsetup, launchctl load of system daemons) belong
in `dot_local/bin/macos-system-setup.sh` (a separately-invoked script the user runs interactively from a
terminal, prompting for sudo once and applying everything in one batch).

The user already has precedent for this split: `dot_local/bin/executable_ssh-hardening.sh` is exactly
this pattern — manually invoked, requires sudo, modifies system state, runs once after Remote Login is
enabled. A `macos-system-setup.sh` script in the same shape would handle pmset and systemsetup calls.

### SIP-protected paths

System Integrity Protection prevents even root from modifying certain critical files. For `defaults`
purposes this is usually invisible (defaults targets `/Library/Preferences/`, not SIP-protected paths),
but a few system-level preference domains in `/System/Library/` are off-limits. The user is unlikely to
want to touch these for personalization purposes; the starter list avoids them.

### TCC-gated paths (Tahoe-tightened)

TCC (Transparency, Consent, and Control) gates certain `defaults` and `systemsetup` operations behind
Full Disk Access grants for the calling process. On Tahoe specifically, more operations are now TCC-gated
than on prior macOS versions [11]. The user's terminal (Ghostty) would need Full Disk Access for
TCC-gated automation to work, which itself requires manual System Settings clicks to grant — a
chicken-and-egg situation that's not worth automating around.

The same rule applies: keep TCC-gated settings out of the chezmoi runner. Document them in
`MACOS_MANUAL_SETUP.md` instead. Examples: Bluetooth visibility toggles, Remote Login state, screen
sharing state, accessibility permissions for tools like Karabiner-Elements (which the user has installed
per their `.chezmoidata/system_packages_autoinstall.yaml`).

### What this leaves for automation

After excluding sudo-required, SIP-protected, and TCC-gated settings, the remaining surface — per-user
`defaults write` to `~/Library/Preferences/` — is large and useful. Dock layout and behavior, Finder
display preferences, keyboard repeat rates, trackpad gestures, screenshot location and format,
screensaver password timing, text-editor defaults, mouse acceleration, menu bar clock formatting, hot
corners — all of these live here and all are safe to automate via the chezmoi runner pattern. The starter
list in Finding 7 stays inside this perimeter.

### Finding 7: Concrete recommendation — files, schema, starter list

The recommended structure adds three files to the chezmoi repo and a single justfile target. Nothing else
changes.

### File 1: `.chezmoidata/macos_defaults.yaml`

Pure declarative data, mirroring the user's existing `system_packages_autoinstall.yaml` shape. Two
top-level keys under a `macos` namespace: `defaults` (list of setting records) and `killall` (list of
processes to restart after applying). Keeping both under `macos` rather than at the top level reserves
namespace and matches the user's "scoped data" convention.

```yaml
# .chezmoidata/macos_defaults.yaml
# Per-user macOS defaults applied by run_onchange_after_30-macos-defaults.sh.tmpl.
# Schema: each entry is {domain, key, type, value}.
#   - type ∈ {bool, int, float, string}
#   - For arrays/dicts, see the docs/MACOS_MANUAL_SETUP.md (rare; not auto-managed).
#   - Adding a setting: append a record. Removing: delete the line and run `just a`.
#     (chezmoi will re-run the script because the YAML hash changes; existing
#     settings on the live system that you removed from YAML are NOT deleted —
#     manage those manually if you want them reverted.)
macos:
  defaults:
    # Dock
    - { domain: "com.apple.dock", key: "autohide",          type: bool,   value: true }
    - { domain: "com.apple.dock", key: "tilesize",          type: int,    value: 48 }
    - { domain: "com.apple.dock", key: "orientation",       type: string, value: "bottom" }
    - { domain: "com.apple.dock", key: "show-recents",      type: bool,   value: false }
    - { domain: "com.apple.dock", key: "mineffect",         type: string, value: "scale" }
    # ... etc
  killall:
    - "Dock"
    - "Finder"
    - "SystemUIServer"
    - "cfprefsd"
```

Why YAML: lint pipeline already covers it via `yq` (per CLAUDE.md). Why scalar-only `value` field:
handles ~95% of useful settings; arrays/dicts can be added later if a specific setting demands them, or
handled out-of-band via `defaults import` from a checked-in plist file. Why explicit `type`: avoids
YAML's inference foot-guns (e.g., the string `"yes"` becoming a boolean) and matches the
`defaults write -bool|-int|-string` flag explicitly.

### File 2: `.chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl`

The runner. Uses `30-` to sit after the Brewfile script (`10-`) and any other early-stage setup scripts;
`after_` because settings should be applied after dotfiles are in place and after any chezmoi script that
might restart Dock/Finder for other reasons.

```bash
#!/usr/bin/env bash
{{- if eq .chezmoi.os "darwin" }}
# macOS defaults applier — driven by .chezmoidata/macos_defaults.yaml.
# This script is idempotent at the chezmoi-hash level: it only runs when the
# YAML data file changes (the rendered template body changes, hash differs).
# `defaults write` is overwrite-by-default and microsecond-cheap, so we don't
# bother with read-then-skip per-key.

set -euo pipefail

# Quit System Settings so it can't fight our writes (a known footgun: open
# System Settings caches plist values and writes them back on Quit, undoing
# defaults writes that happened while it was open).
osascript -e 'tell application "System Settings" to quit' 2>/dev/null || true

echo "Applying macOS defaults..."

{{- range .macos.defaults }}
defaults write {{ .domain | quote }} {{ .key | quote }} -{{ .type }} {{ .value | quote }}
{{- end }}

# Restart affected processes so changes take effect immediately. cfprefsd is
# critical: it caches preferences and many writes won't take effect until it
# is killed (a long-standing macOS quirk; nix-darwin issue #1572 documents
# the same trap).
{{- range .macos.killall }}
killall {{ . | quote }} 2>/dev/null || true
{{- end }}

echo "macOS defaults applied."
{{- end }}
```

The `osascript -e 'tell application "System Settings" to quit'` step is non-obvious but important: if
System Settings is open while you `defaults write`, it can write its cached values back over yours when
the user closes it. Multiple production scripts include this guard. Empty rendered output on Linux
(`{{- if eq .chezmoi.os "darwin" }}...{{- end }}`) is safe per chezmoi maintainer confirmation [13].

### File 3: `dot_local/bin/executable_macos-defaults-drift.sh`

The drift checker. Read-only by construction (no `defaults write`), exits non-zero on drift.

```bash
#!/usr/bin/env bash
# macos-defaults-drift.sh — report drift between .chezmoidata/macos_defaults.yaml
# and the live system's defaults state. Exits 0 if clean, 1 if drift detected.
# Wired into the justfile as `just D`.
set -euo pipefail

DATA="${HOME}/.local/share/chezmoi/.chezmoidata/macos_defaults.yaml"
[[ -f $DATA ]] || { echo "No data file at $DATA"; exit 2; }

drift=0
printf '%-44s %-32s %-12s %-12s\n' DOMAIN KEY EXPECTED ACTUAL
echo "---------------------------------------------------------------------------------------------------"

while IFS=$'\t' read -r domain key type expected; do
  actual=$(defaults read "$domain" "$key" 2>/dev/null) || actual="<unset>"
  # Normalize bool: defaults stores true/false as 1/0
  case "$type" in
    bool|boolean)
      [[ "$expected" == "true" || "$expected" == "yes" ]] && expected=1
      [[ "$expected" == "false" || "$expected" == "no" ]] && expected=0
      ;;
  esac
  if [[ "$actual" != "$expected" ]]; then
    printf '%-44s %-32s %-12s %-12s\n' "$domain" "$key" "$expected" "$actual"
    drift=1
  fi
done < <(yq -r '.macos.defaults[] | [.domain, .key, .type, .value] | @tsv' "$DATA")

if [[ $drift -eq 0 ]]; then
  echo "(no drift detected)"
fi
exit $drift
```

### File 4 (optional): `dot_local/bin/executable_macos-defaults-apply.sh`

A user-facing reapplier that bypasses chezmoi for cases where you want to force re-application without
changing the YAML (i.e., to revert drift). Same data file, same logic as the runner, but invocable
directly:

```bash
#!/usr/bin/env bash
set -euo pipefail
DATA="${HOME}/.local/share/chezmoi/.chezmoidata/macos_defaults.yaml"
osascript -e 'tell application "System Settings" to quit' 2>/dev/null || true
yq -r '.macos.defaults[] | "defaults write \"" + .domain + "\" \"" + .key + "\" -" + .type + " \"" + (.value | tostring) + "\""' "$DATA" | bash
yq -r '.macos.killall[]' "$DATA" | xargs -n1 -I{} sh -c 'killall "$1" 2>/dev/null || true' _ {}
echo "macOS defaults applied (forced reapply)."
```

This makes the apply path explicit and removes the "why didn't chezmoi apply re-run" footgun for drift
remediation.

### Justfile additions

```just
D:
  nix develop .#run --command ~/.local/bin/macos-defaults-drift.sh

defaults-apply:
  nix develop .#run --command ~/.local/bin/macos-defaults-apply.sh
```

The single-letter `D` matches the user's existing convention (`d` is `chezmoi diff`, `a` is
`chezmoi apply`, `D` for "defaults drift" extends the pattern).

### Starter list of ~30 high-value defaults

This is a starting point, not a deliverable — the user should curate. Each setting is verified to work
via per-user `defaults write` (no sudo, no TCC) and is stable across macOS 24-26. Settings deliberately
omitted: Dock persistent-apps (array; restore from a per-machine snapshot is more useful), Finder
favorites sidebar (array; same reason), anything Apple Intelligence / Siri / Keyboard
(declarative-profile territory now), anything Bluetooth / Remote Login / Screen Sharing (TCC-gated on
Tahoe).

```yaml
macos:
  defaults:
    # ===== Dock =====
    - { domain: "com.apple.dock", key: "autohide",                  type: bool,   value: true }
    - { domain: "com.apple.dock", key: "autohide-delay",            type: float,  value: 0 }
    - { domain: "com.apple.dock", key: "autohide-time-modifier",    type: float,  value: 0.4 }
    - { domain: "com.apple.dock", key: "tilesize",                  type: int,    value: 48 }
    - { domain: "com.apple.dock", key: "orientation",               type: string, value: "bottom" }
    - { domain: "com.apple.dock", key: "mineffect",                 type: string, value: "scale" }
    - { domain: "com.apple.dock", key: "show-recents",              type: bool,   value: false }
    - { domain: "com.apple.dock", key: "minimize-to-application",   type: bool,   value: true }
    - { domain: "com.apple.dock", key: "show-process-indicators",   type: bool,   value: true }

    # ===== Finder =====
    - { domain: "com.apple.finder",          key: "ShowPathbar",                 type: bool,   value: true }
    - { domain: "com.apple.finder",          key: "ShowStatusBar",               type: bool,   value: true }
    - { domain: "com.apple.finder",          key: "FXPreferredViewStyle",        type: string, value: "Nlsv" }   # list view
    - { domain: "com.apple.finder",          key: "_FXShowPosixPathInTitle",     type: bool,   value: true }
    - { domain: "com.apple.finder",          key: "FXEnableExtensionChangeWarning", type: bool, value: false }
    - { domain: "com.apple.finder",          key: "FXDefaultSearchScope",        type: string, value: "SCcf" }   # current folder
    - { domain: "com.apple.finder",          key: "NewWindowTarget",             type: string, value: "PfHm" }   # home dir
    - { domain: "NSGlobalDomain",            key: "AppleShowAllExtensions",      type: bool,   value: true }

    # ===== Keyboard =====
    - { domain: "NSGlobalDomain",            key: "KeyRepeat",                   type: int,    value: 2 }        # fast
    - { domain: "NSGlobalDomain",            key: "InitialKeyRepeat",            type: int,    value: 15 }
    - { domain: "NSGlobalDomain",            key: "ApplePressAndHoldEnabled",    type: bool,   value: false }    # holding key repeats

    # ===== Trackpad =====
    - { domain: "com.apple.AppleMultitouchTrackpad",   key: "Clicking",          type: bool,   value: true }     # tap to click
    - { domain: "com.apple.driver.AppleBluetoothMultitouch.trackpad", key: "Clicking", type: bool, value: true }

    # ===== Screenshots =====
    - { domain: "com.apple.screencapture",   key: "type",                        type: string, value: "png" }
    - { domain: "com.apple.screencapture",   key: "disable-shadow",              type: bool,   value: true }
    - { domain: "com.apple.screencapture",   key: "show-thumbnail",              type: bool,   value: false }    # no floating preview
    # location: ~/Pictures/Screenshots — set via the runner because it needs $HOME
    - { domain: "com.apple.screencapture",   key: "location",                    type: string, value: "~/Pictures/Screenshots" }

    # ===== Screensaver =====
    - { domain: "com.apple.screensaver",     key: "askForPassword",              type: int,    value: 1 }
    - { domain: "com.apple.screensaver",     key: "askForPasswordDelay",         type: int,    value: 0 }

    # ===== Mission Control / Spaces =====
    - { domain: "com.apple.dock",            key: "mru-spaces",                  type: bool,   value: false }    # don't reorder

    # ===== TextEdit =====
    - { domain: "com.apple.TextEdit",        key: "RichText",                    type: int,    value: 0 }        # plain text default

    # ===== Global =====
    - { domain: "NSGlobalDomain",            key: "NSWindowResizeTime",          type: float,  value: 0.001 }
    - { domain: "NSGlobalDomain",            key: "ApplePersistenceIgnoreState", type: bool,   value: true }     # don't restore windows on app launch

  killall:
    - "Dock"
    - "Finder"
    - "SystemUIServer"
    - "cfprefsd"
```

That's 31 settings. The user should:

1. Review each — some (like `tilesize: 48`, `orientation: bottom`) are taste preferences that should
   match what the user already has on their main machine.
1. Run `defaults read <domain> <key>` for each on a machine they like, and snapshot the values into the
   YAML.
1. Add domain-specific killall entries if needed (e.g., `Mail` for Mail.app preferences).
1. Document how to discover new settings: `defaults read | grep -i <pattern>`,
   `defaults read-type <domain> <key>` (returns `Type is integer`, etc.), and the macos-defaults.com
   catalog as a starting reference [43].

### Bootstrap on a fresh Mac

Walk-through of what happens on first run:

1. User clones chezmoi repo via `chezmoi init <github-user>` (existing process).
1. `chezmoi apply` runs: `run_onchange_before_10-system-packages.sh.tmpl` installs Homebrew packages
   (existing); other run_once / run_onchange scripts execute;
   `run_onchange_after_30-macos-defaults.sh.tmpl` finally runs the defaults loop and killalls
   Dock/Finder/etc.
1. Result: ~30 settings applied in \<1s, Dock and Finder restarted, machine matches declared state.

No manual handholding beyond chezmoi init. The user's hard requirement #1 is met.

### Modification ergonomics

Add a setting:

1. Edit `.chezmoidata/macos_defaults.yaml`, append a record.
1. `just l` (lint) — yq validates structure, shellcheck stays happy.
1. `just a` (chezmoi apply) — runner re-runs because YAML hash changed.

Remove a setting:

1. Delete the line from YAML.
1. `just a` — runner runs but no longer writes that key. **Caveat:** the live system still has the value
   you deleted. If you want it reverted to macOS default, run `defaults delete <domain> <key>` manually.
   (This asymmetry — chezmoi-managed adds vs unmanaged removes — is a fundamental property of
   `defaults write`, not specific to the recommendation.)

The user's hard requirement #4 (one-file edit) is met for adds; removes require a separate one-time
manual cleanup, which should be documented in the file's leading comment.

## Synthesis & Insights

### The hidden cost of "declarative" tooling for a 30-line problem

The competitive landscape for macOS defaults management spans from "declarative-pure" (nix-darwin's typed
Nix module, full type checking and dependency graphs) to "imperative-pure" (mathiasbynens-style 200-line
bash script with inlined `defaults write` calls). Most third-party tools (dsully, koenrh, RATIU5) sit in
the middle: a YAML/TOML data file consumed by a binary runner. The marketing pitch for the middle tier is
consistent — "one file declares your settings, the binary applies them, and you get drift detection /
type safety / kill-list management for free." For users running this on shared infrastructure or
maintaining settings for a fleet, that pitch is correct: the per-tool overhead amortizes across many
users and many settings.

For a single-developer personal Mac with ~30 settings, the math inverts. The "binary runner" tier
requires you to track upstream releases (nix-darwin issues new releases monthly, dsully releases yearly,
koenrh has no releases), debug binary-specific failure modes (nix-darwin's activateSettings gap, dsully's
--dry-run bug), and accept a new dependency category in your toolchain. Meanwhile, the "imperative bash"
approach has been doing the same work for fifteen years with negligible failure rate — `defaults write`
is one of the most stable interfaces Apple ships, and a
`for setting in list; do defaults write $setting; done` loop is two lines.

The recommendation in this report — `.chezmoidata/macos_defaults.yaml` + thin runner — captures the
data/code separation that makes the "middle tier" attractive without paying the binary runner's cost. The
data lives as data; the runner is a stable 30-line consumer that rarely changes; chezmoi's hash gate
handles idempotency externally rather than baking it into the runner. This is the "trust the wrapper,
keep the script dumb" pattern, and it's the one that fits a developer's existing investment in chezmoi
best.

### Why the "trust the wrapper" pattern generalizes

The user's existing repository already shows this pattern at work. The Brewfile script
(`run_onchange_before_10-system-packages.sh.tmpl`) doesn't bother checking whether a Homebrew formula is
already installed before running `brew bundle` — it trusts that `brew bundle` is fast on a no-op and that
chezmoi won't run the script unless the YAML data has changed. The same logic applies to defaults:
`defaults write` is fast on a no-op, and chezmoi won't run the script unless the data has changed. By
treating chezmoi as the orchestration layer, individual scripts can stay small and focused on their
specific job rather than re-implementing change detection.

This pattern has an additional benefit: it composes cleanly with the user's existing `nix develop .#run`
lint pipeline. Because the runner is a chezmoi script template, it's already linted by shellcheck (via
`CI=1 chezmoi execute-template < script | shellcheck -`) and shfmt; because the data file is YAML, it's
already linted by yq. No new tooling is needed in the lint step. A new tool like dsully would not be
linted at all (the binary's behavior is opaque to the user's lint pipeline), so any errors in the YAML
schema would only surface at apply-time instead of pre-commit-time.

### The drift-detection insight

The user's requirement for drift detection is the single point where dsully looks most attractive — its
`--exit-code N` plumbing (when `--dry-run` works, post-fix-release) gives you "tell me if the system has
drifted" as a one-liner. But the same capability in 50 lines of bash is plausibly *better* for this user,
because: (a) the bash version reads the same data file the runner uses, so there's one source of truth
instead of two; (b) the bash version is modifiable on the fly without waiting for an upstream release;
(c) the bash version composes with the user's existing justfile and lint pipeline; (d) the bash version
doesn't depend on any binary the user doesn't already have.

A subtler point: a custom drift checker can encode user-specific opinions. For instance, the user may
want to ignore drift on certain settings (Dock persistent-apps, which they reorder by hand on each
machine) while strictly enforcing others (screenshots location, which they want consistent everywhere). A
50-line bash script can grow a `skip_drift` flag in the YAML record and act on it; a third-party binary
cannot, without an upstream feature request.

### A second-order observation about Tahoe

The TCC/SIP tightening on macOS 26 has an under-appreciated consequence for dotfile management: the
surface area of "things you can automate from the command line" is shrinking, and Apple is unlikely to
reverse course. Each new macOS version will likely move *more* settings out of `defaults write`
jurisdiction and into either System Settings UI clicks or signed configuration profiles. This means any
investment in an elaborate tool for managing the shrinking command-line surface is a depreciating asset;
investment in a thin, easily-modifiable wrapper that the user owns and can adapt is more durable.

The honest framing for a 2026 dotfiles author: `defaults write` is a stable but shrinking interface. The
chezmoi runner pattern handles what's left of it gracefully. The remainder — TCC grants,
declarative-profile settings, anything Apple Intelligence — should be documented as manual setup steps
and re-evaluated as Apple's tooling evolves (or as the user moves to a fleet-management context where MDM
becomes appropriate).

### The "configuration profile" escape hatch

For users who outgrow `defaults` for the security/privacy tier of settings, signed `.mobileconfig`
configuration profiles are the next step up. They can be authored in plain XML, signed with a self-signed
cert (Mac will warn at install but accept), and applied via `profiles install -path foo.mobileconfig`.
This is out of scope for this report but worth flagging: if and when the user wants to automate Bluetooth
visibility or Remote Login state, the path is `.mobileconfig`, not `defaults`. The chezmoi script for
that would be a sibling to the defaults runner: a `run_once_after_99-install-mobileconfigs.sh.tmpl` that
checks for an installed profile by name and installs from a checked-in `.mobileconfig` file if absent.
Defer until needed.

## Limitations & Caveats

### Single-source claims

Two findings rest on single sources or single observations:

1. **The dsully/macos-defaults `--dry-run` fix landing 2026-04-26 with no release**: confirmed by issue
   #10 + the HEAD commit on the same day. If the maintainer cuts a release within days, this caveat
   softens. Re-check before adopting dsully.
1. **Zero public chezmoi+nix-darwin defaults-only precedent**: a negative finding from a single GitHub
   code search. It's possible such a setup exists in private repos or has been discussed informally; the
   absence of a public example doesn't mean nobody has done it. The point stands directionally — there's
   no documented playbook to copy from.

### Starter list is opinionated

The 31-setting starter list reflects a generalist developer's preferences (fast keyboard repeat, autohide
Dock, list-view Finder, plain-text TextEdit). The user should treat it as a discussion document, not a
final answer. Some settings (like `Clicking: true` for tap-to-click) are matters of muscle memory; others
(like `_FXShowPosixPathInTitle: true`) are workflow-specific. Settings the user doesn't care about should
be removed from the YAML — every entry is a small ongoing maintenance burden (Apple may rename or
deprecate keys across versions).

### macOS 26 may further tighten

Apple's enterprise documentation indicates ongoing migration of restrictions from `defaults`-style
preferences to declarative configuration profiles. Settings that work in macOS 26.2 (the user's current
version) may not work in 26.5 or 27.0. The recommended pattern is robust to this — settings that stop
working will fail visibly when `defaults write` returns nonzero, and the drift checker will flag them as
`<unset>` even though declared. But the starter list will require maintenance over time.

### Scalar-only schema

The recommended YAML schema handles scalar values (bool, int, float, string) cleanly. Arrays and
dictionaries (e.g., Dock persistent-apps) are not handled. This is a deliberate trade-off — the v1
implementation stays small and the data file stays readable. If the user wants to manage array/dict
settings, the cleanest path is a separate `dot_config/macos/dock-layout.plist` checked into the repo and
applied via `defaults import com.apple.dock dock-layout.plist` from the runner. Treat this as a v2
extension if the need materializes.

### Drift checker doesn't restore

The drift checker reports drift, exits non-zero, but does not write. Restore is a separate explicit step
(`just defaults-apply` calls the reapplier script described in Finding 7). This is a deliberate safety
choice — auto-restore on detection would be surprising and could fight against an intentional manual
change the user made temporarily. Manual reapply is a small two-step (drift-check → reapply) but keeps
the tool's effects predictable.

### Two-machine difference is hard to reason about

If the user keeps two machines (e.g., dresden and uriel) on different YAML revisions briefly (machine A
hasn't pulled the latest commit yet), running the drift checker on machine A will report drift against
the un-pulled new settings. This is technically correct but potentially confusing. A future enhancement:
a `git status`-style "your YAML is N commits behind origin/main" check at the top of the drift script.
Not load-bearing for v1.

## Recommendations

### Immediate actions (this week)

1. **Create `.chezmoidata/macos_defaults.yaml`** with the starter list from Finding 7, customized to the
   user's preferences. Spend 15 minutes on each machine running `defaults read <domain> <key>` for any
   settings the user already has dialed in to capture them as the baseline.
1. **Create `.chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl`** with the runner from Finding
   7\. Verify it lints (`just l`) and renders cleanly
   (`CI=1 chezmoi execute-template --no-tty < .chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl`).
1. **Create `dot_local/bin/executable_macos-defaults-drift.sh`** with the drift checker. Test it on the
   current machine to verify it correctly identifies "no drift" against the YAML you just created.
1. **Create `dot_local/bin/executable_macos-defaults-apply.sh`** with the forced reapplier.
1. **Add `D` and `defaults-apply` targets to the justfile.**
1. **Add a section to CLAUDE.md** documenting the new pattern, mirroring the existing "Claude Code
   Settings" / "Homebrew install workflow" sections.
1. **Run `just a`** to apply on the current machine. Verify settings stick (open Dock preferences, check
   tilesize matches; open Finder, verify list view).
1. **Commit as one logical unit** with conventional-commit message:
   `feat(macos-defaults): declarative defaults management via chezmoi data + runner`.

### Near-term (next month)

9. **Document the manual setup checklist** at `docs/MACOS_MANUAL_SETUP.md`: TCC grants for Ghostty /
   Karabiner / Hammerspoon / etc., Bluetooth toggles, Remote Login enable, screen recording /
   accessibility permissions for any tools that need them. Reference this from CLAUDE.md and the README.
   This documents the "30% that defaults can no longer touch" honestly so future-you doesn't try to
   automate it.
1. **Run the drift checker on a 30-day cadence** (or hook it into the `osquery-report.sh` daily report).
   Drift will be rare on a single user's machine but worth catching when it happens.

### Quarterly review

11. **Re-evaluate dsully/macos-defaults annually.** Check (a) whether the `--dry-run` fix has been
    released; (b) whether the project has gained a second active contributor (mitigates bus factor); (c)
    whether YAML schema features have grown in ways that would matter for the user's use case. If the
    answer is yes-yes-yes, swap the runner for `macos-defaults apply ~/.config/macos-defaults/` and keep
    the drift checker (which still has value as a justfile target even if dsully provides its own).
01. **Re-evaluate nix-darwin if and when broader Nix-for-system adoption is on the table.** If the user
    ever wants to declaratively manage system services, LaunchAgents, or system-wide packages via Nix,
    the marginal cost of adding `system.defaults` to the existing nix-darwin config drops dramatically.

### Things to NOT do

- Do **not** install nix-darwin solely to gain `system.defaults`. The bootstrap cost (Nix install, sudo,
  primaryUser ceremony, four open Tahoe issues) is not justified for defaults-only use.
- Do **not** install dsully/macos-defaults *until* a release ships with the `--dry-run` fix. The Homebrew
  bottle currently in the tap is 0.3.0 with the bug.
- Do **not** try to automate TCC-gated settings (Bluetooth, Remote Login, Screen Sharing) via `defaults`
  — it will silently fail on Tahoe. Document them as manual steps.
- Do **not** add `defaults read` / skip-if-already-set logic to the runner. Trust the chezmoi hash gate.
- Do **not** put system-wide settings (`/Library/Preferences/`, `pmset`, `systemsetup`) into the chezmoi
  runner — they require sudo and will hang in non-interactive contexts. Use a separate manually-invoked
  `macos-system-setup.sh` script in the same shape as the existing `ssh-hardening.sh`.

## Bibliography

[1] chezmoi (2026). "Use scripts to perform actions." chezmoi documentation.
https://www.chezmoi.io/user-guide/use-scripts-to-perform-actions/ (Retrieved: 2026-04-26)

[2] dsully (2026). "Issue #10: --dry-run flag silently ignored: apply writes changes regardless."
dsully/macos-defaults GitHub. https://github.com/dsully/macos-defaults/issues/10 (Retrieved: 2026-04-26)

[3] dsully (2026). "macos-defaults README: synopsis." dsully/macos-defaults GitHub.
https://github.com/dsully/macos-defaults/blob/main/README.md (Retrieved: 2026-04-26)

[4] nix-darwin contributors (2026). "modules/system/defaults source." nix-darwin GitHub.
https://github.com/nix-darwin/nix-darwin/tree/master/modules/system/defaults (Retrieved: 2026-04-26)

[5] nix-darwin contributors (2026). "defaults-write.nix lines 12-16 (userDefaultsToList sudo asuser)."
nix-darwin GitHub. https://github.com/nix-darwin/nix-darwin/blob/master/modules/system/defaults-write.nix
(Retrieved: 2026-04-26)

[6] nix-darwin contributors (2025). "Issue #1621: Tahoe modified /etc/zshrc and /etc/zprofile (closed)."
nix-darwin GitHub. https://github.com/nix-darwin/nix-darwin/issues/1621 (Retrieved: 2026-04-26)

[7] nix-darwin contributors (2025). "Issue #1544: nix-darwin on macOS Tahoe — sudo: darwin-rebuild:
command not found (open)." nix-darwin GitHub. https://github.com/nix-darwin/nix-darwin/issues/1544
(Retrieved: 2026-04-26)

[8] nix-darwin contributors (2025). "Issue #1577: $TMPDIR issues on macOS 26 (open)." nix-darwin GitHub.
https://github.com/nix-darwin/nix-darwin/issues/1577 (Retrieved: 2026-04-26)

[9] nix-darwin contributors (2025). "Issue #1513: macOS 26: failed to stitch firmlinks (open)."
nix-darwin GitHub. https://github.com/nix-darwin/nix-darwin/issues/1513 (Retrieved: 2026-04-26)

[10] GitHub Code Search (2026). "Query: 'darwin-rebuild' chezmoi language:Shell — total: 0 results."
GitHub. https://github.com/search?q=%22darwin-rebuild%22+chezmoi+language%3AShell&type=code (Retrieved:
2026-04-26)

[11] Artic6 Blog (2026). "Automating Mac Mini Content Caching Server Setup on macOS Tahoe."
https://www.a6n.co.uk/2026/02/automating-mac-mini-content-caching.html (Retrieved: 2026-04-26)

[12] chezmoi (2026). "Developer guide: architecture / persistent state." chezmoi documentation.
https://www.chezmoi.io/developer-guide/architecture/ (Retrieved: 2026-04-26)

[13] twpayne (2024). "Discussion #4555: run_onchange empty body behavior." chezmoi GitHub.
https://github.com/twpayne/chezmoi/discussions/4555 (Retrieved: 2026-04-26)

[14] twpayne (2024). "Discussion #4208: What is the difference between run_onchange\_ and run_once\_?"
chezmoi GitHub. https://github.com/twpayne/chezmoi/discussions/4208 (Retrieved: 2026-04-26)

[15] cweagans (2026). "run_once_after_install_defaults.sh.tmpl." cweagans/dotfiles.
https://github.com/cweagans/dotfiles/blob/main/.chezmoiscripts/darwin/run_once_after_install_defaults.sh.tmpl
(Retrieved: 2026-04-26)

[16] posquit0 (2026). "run_onchange_after_02_configure_macos_defaults.sh." posquit0/dotfiles.
https://github.com/posquit0/dotfiles/blob/main/.chezmoiscripts/darwin/run_onchange_after_02_configure_macos_defaults.sh
(Retrieved: 2026-04-26)

[17] timriley (2026). "run_onchange_after_configure-macos.sh.tmpl." timriley/dotfiles.
https://github.com/timriley/dotfiles/blob/main/.chezmoiscripts/run_onchange_after_configure-macos.sh.tmpl
(Retrieved: 2026-04-26)

[18] smasato (2026). "run_onchange_after_10-defaults.sh.tmpl." smasato/dotfiles.
https://github.com/smasato/dotfiles/blob/main/.chezmoiscripts/run_onchange_after_10-defaults.sh.tmpl
(Retrieved: 2026-04-26)

[19] liby (2026). "run_onchange_after_320-setup-macos-defaults.sh." liby/dotfiles.
https://github.com/liby/dotfiles/blob/main/.chezmoiscripts/run_onchange_after_320-setup-macos-defaults.sh
(Retrieved: 2026-04-26)

[20] felixjung (2026). "run_onchange_after_configure_macos_settings.sh.tmpl." felixjung/dotfiles.
https://github.com/felixjung/dotfiles/blob/main/.chezmoiscripts/run_onchange_after_configure_macos_settings.sh.tmpl
(Retrieved: 2026-04-26)

[21] kevinold (2024). "run_once_04-macos-defaults.sh." kevinold/dotfiles.
https://github.com/kevinold/dotfiles/blob/45e753011bd5856ee60f9a7fb3dbe1a5cf8100b8/.chezmoiscripts/run_once_04-macos-defaults.sh
(Retrieved: 2026-04-26)

[22] dsully (2026). "macos-defaults README: schema, !-overwrite, ...-splice." dsully/macos-defaults
GitHub. https://github.com/dsully/macos-defaults/blob/main/README.md (Retrieved: 2026-04-26)

[23] dsully (2026). "macos-defaults README: 'On YAML' section." dsully/macos-defaults GitHub.
https://github.com/dsully/macos-defaults/blob/main/README.md#on-yaml (Retrieved: 2026-04-26)

[24] dsully (2026). "src/cmd/apply.rs: process_yaml_document + kill_process_by_name."
dsully/macos-defaults GitHub. https://github.com/dsully/macos-defaults/blob/main/src/cmd/apply.rs
(Retrieved: 2026-04-26)

[25] dsully (2026). "src/defaults.rs: plist_filename + extend_with_prefs_folders." dsully/macos-defaults
GitHub. https://github.com/dsully/macos-defaults/blob/main/src/defaults.rs (Retrieved: 2026-04-26)

[26] dsully (2026). "src/main.rs: Apply subcommand --exit-code." dsully/macos-defaults GitHub.
https://github.com/dsully/macos-defaults/blob/main/src/main.rs (Retrieved: 2026-04-26)

[27] dsully (2026). "Repository metadata + contributors API." dsully/macos-defaults GitHub.
https://api.github.com/repos/dsully/macos-defaults (Retrieved: 2026-04-26)

[28] zero-sh (2023). "apply-user-defaults: A small utility to set macOS user defaults declaratively from
a YAML file." zero-sh/apply-user-defaults GitHub. https://github.com/zero-sh/apply-user-defaults
(Retrieved: 2026-04-26)

[29] koenrh (2026). "deft: Declarative defaults manager." koenrh/deft GitHub.
https://github.com/koenrh/deft (Retrieved: 2026-04-26)

[30] RATIU5 (2025). "fjrd: Declarative macOS settings tool (TOML)." RATIU5/fjrd GitHub.
https://github.com/RATIU5/fjrd (Retrieved: 2026-04-26)

[31] g0t4 (2024). "mcp-server-macos-defaults: macOS defaults Model Context Protocol server."
g0t4/mcp-server-macos-defaults GitHub. https://github.com/g0t4/mcp-server-macos-defaults (Retrieved:
2026-04-26)

[32] nix-darwin contributors (2026). "README." nix-darwin GitHub.
https://github.com/nix-darwin/nix-darwin/blob/master/README.md (Retrieved: 2026-04-26)

[33] nix-darwin contributors (2026). "modules/system/primary-user.nix: system.requiresPrimaryUser
assertion." nix-darwin GitHub.
https://github.com/nix-darwin/nix-darwin/blob/master/modules/system/primary-user.nix (Retrieved:
2026-04-26)

[34] nix-darwin contributors (2026). "modules/system/defaults-write.nix lines 8-9, 128-152." nix-darwin
GitHub. https://github.com/nix-darwin/nix-darwin/blob/master/modules/system/defaults-write.nix
(Retrieved: 2026-04-26)

[35] nix-darwin contributors (2023-2026). "Issue #658: system.defaults settings not activated on switch."
nix-darwin GitHub. https://github.com/nix-darwin/nix-darwin/issues/658 (Retrieved: 2026-04-26)

[36] nix-darwin contributors (2025). "Issue #1572: trackpad settings require manual cfprefsd restart."
nix-darwin GitHub. https://github.com/nix-darwin/nix-darwin/issues/1572 (Retrieved: 2026-04-26)

[37] nix-darwin contributors (2025). "Issue #1475: How to activate user defaults now (sudo migration)?"
nix-darwin GitHub. https://github.com/nix-darwin/nix-darwin/issues/1475 (Retrieved: 2026-04-26)

[38] nix-darwin contributors (2026). "Issue #1721: Feature Request: ByHost / currentHost domain."
nix-darwin GitHub. https://github.com/nix-darwin/nix-darwin/issues/1721 (Retrieved: 2026-04-26)

[39] Fortunato, A. (2026). "Cross-Platform Dotfiles with Chezmoi, Nix, Brew, and Devpod."
alfonsofortunato.com. https://alfonsofortunato.com/blog/dotfile/ (Retrieved: 2026-04-26)

[40] HowToiSolve (2026). "How to Give Full Disk Access & Full Permissions on Mac (Tahoe 26)."
https://www.howtoisolve.com/full-disk-access-full-permissions-on-mac/ (Retrieved: 2026-04-26)

[41] Apple Inc. (2026). "What's new for enterprise in macOS Tahoe 26." Apple Support.
https://support.apple.com/en-us/124963 (Retrieved: 2026-04-26)

[42] ss64.com (2026). "SYSTEMSETUP Command: Configure System Preferences in macOS."
https://ss64.com/mac/systemsetup.html (Retrieved: 2026-04-26)

[43] Bertrand, Y. (2026). "macos-defaults.com — A list of macOS defaults commands with demos."
https://macos-defaults.com/ (Retrieved: 2026-04-26)

[44] chezmoi (2026). "Reference: .chezmoidata special directory." chezmoi documentation.
https://www.chezmoi.io/reference/special-directories/chezmoidata/ (Retrieved: 2026-04-26)

[45] chezmoi (2026). "Reference: source state attributes." chezmoi documentation.
https://www.chezmoi.io/reference/source-state-attributes/ (Retrieved: 2026-04-26)

[46] chezmoi (2026). "User guide: macOS." chezmoi documentation.
https://www.chezmoi.io/user-guide/machines/macos/ (Retrieved: 2026-04-26)

[47] Apple Inc. (2026). "macOS Tahoe 26 Release Notes." Apple Developer Documentation.
https://developer.apple.com/documentation/macos-release-notes/macos-26-release-notes (Retrieved:
2026-04-26)

[48] Wikipedia (2026). "macOS Tahoe." https://en.wikipedia.org/wiki/MacOS_Tahoe (Retrieved: 2026-04-26)

[49] Ansible Community (2026). "community.general.osx_defaults module — Manage macOS user defaults."
Ansible documentation.
https://docs.ansible.com/projects/ansible/latest/collections/community/general/osx_defaults_module.html
(Retrieved: 2026-04-26)

[50] Apple Inc. (2026). "About the security content of macOS Tahoe 26.2." Apple Support.
https://support.apple.com/en-us/125886 (Retrieved: 2026-04-26)

## Methodology Appendix

### Research approach

The report was produced via the deep-research deep-mode pipeline (8 phases: SCOPE, PLAN, RETRIEVE,
TRIANGULATE, OUTLINE REFINEMENT, SYNTHESIZE, CRITIQUE, PACKAGE). Phase durations: scope/plan ~3 minutes;
parallel retrieval ~5 minutes; synthesis and packaging ~25 minutes; total wall clock ~35 minutes.

### Source acquisition

Phase 3 (RETRIEVE) launched eight parallel WebSearch queries and three parallel `general-purpose`
sub-agents in a single message. The WebSearches covered: mathiasbynens canonical script + alternatives,
dsully/macos-defaults specifics, nix-darwin system.defaults, Ansible osx_defaults, macOS 26 Tahoe
breaking changes, chezmoi run_onchange patterns, drift detection script patterns, and SIP/sudo/TCC
boundaries. The sub-agents produced structured `{claim, evidence_quote, source_url, confidence}` blocks
for: (a) dsully tool internals + competitor landscape; (b) nix-darwin defaults module catalog + macOS 26
issues; (c) chezmoi run_onchange semantics + 5 production-repo case studies. Two follow-up Phase 4.5
gap-fills addressed: a 2026 starter list of Tahoe-tested settings, and TCC/Full-Disk-Access implications
for `defaults write`.

### Triangulation

Major claims required corroboration from 3+ independent sources where available. The `defaults` command
flag semantics (`-bool`, `-int`, etc.) and the chezmoi script-types semantics were verified against
canonical Apple/chezmoi documentation directly. The dsully `--dry-run` bug was verified against both the
open issue (#10) and the HEAD source code. nix-darwin's macOS 26 friction was triangulated across at
least four separate issue tickets. The "no chezmoi+nix-darwin defaults-only precedent" claim is a
single-source negative finding (zero results from a targeted GitHub code search) and is flagged as such
in Limitations.

### Verification of local environment

Local Bash calls verified: today's date (2026-04-26), the user's macOS version (26.2 build 25C56), the
`defaults` command's full flag list (confirming `-bool/-int/-float/-string/-array/-dict` types and
`-currentHost`/`-host`/`-globalDomain` scoping options). chezmoi's macOS-specific guide [46] was
consulted for environment expectations on Apple Silicon. This grounded the recommendation in the user's
actual environment rather than a hypothetical one.

### Sources of bias and uncertainty

The deep-research approach favors documented evidence over expert opinion, which can underweight insider
knowledge that hasn't made it into docs. For this report specifically: (a) the dsully tool's bus-factor
critique reflects publicly-visible commit and contributor data only — the maintainer may have private
maintenance plans that change the calculus; (b) Apple's enterprise documentation is the source of truth
for Tahoe policy changes, but specific pre-release behavior in 26.5+ is not yet documented; (c) the
starter list of ~30 settings reflects a "generalist developer" persona and may not match the user's
specific tastes.

### What was deliberately not researched

- **MDM (Jamf, Kandji, Mosyle) approaches** — out of scope for a single-developer personal Mac.
- **Legacy macOS versions** — the user's machine is on 26.2; no Catalina/Big Sur compatibility analysis.
- **Multi-user / role-based settings** — single-user personal machine assumed.
- **Backup/restore of existing system settings** before applying — the cweagans-style `.prev` plist
  backup is one approach, but adds complexity not justified at v1; rely on Time Machine + the repo's git
  history instead.
