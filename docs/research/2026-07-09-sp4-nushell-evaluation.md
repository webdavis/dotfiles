# SP4 — bash → nushell GO/NO-GO Evaluation

**Date:** 2026-07-09 · **Scope:** research only, read-only on repo + machine · **Effort:** max
**Machine:** dresden · **Live source of truth:** `integration/modernization` checkout at
`.worktrees/slice-herdr/`

______________________________________________________________________

## 1. VERDICT

# NO-GO

Keep bash as the interactive shell. Close GH #5 as *evaluated / declined* (not the old "wontfix" framing
— decision 3 required an honest look; this is it), and leave a one-paragraph pointer to this report so
the question is not re-litigated.

**Deciding rationale.** Two of this repo's largest, most load-bearing bash investments have no migration
path that clears the cost bar. First, the interactive keybinding surface — **365 `bind` statements across
`dot_bash_bindings` + `dot_fzf_bindings`, backed by 53 helper shell functions**, and essentially *all of
them* are multi-key readline chords (`\C-x0`, `\C-g d r`, `\C-x n l t`, macros that end in `\r`).
Nushell's line editor, reedline, **does not support multi-key chord sequences at all** — it binds single
`modifier+keycode` events — and its vi mode is documented-incomplete (Shift-C/Shift-S dead, `B` motion
wrong, `$` overshoots). That surface cannot be *ported*; it must be *re-conceived* against a weaker
editor, forfeiting years of muscle memory. Second, **atuin** — the history system the whole shell leans
on, tuned here to daemon mode + `filter_mode = "host"` — is the nushell ecosystem's weakest integration
(recording via hooks or the new experimental pty-proxy "hex", daemon-coexistence with nushell
undocumented, a prior nu-0.106 breakage), so re-stabilizing it is genuine, open-ended risk against a hot
path that today measures ~15 ms and simply works. Against that cost sits nushell's actual value
proposition — structured-data pipelines — which barely touches an interactive-shell + agent-notification
workflow. Nushell also ships a breaking release every 4 weeks with config-schema breaks every 2–5
releases, i.e. a permanent re-stabilization tax on a solo, low-maintenance-bias machine. The one thing
the roadmap flagged as the hard part — non-interactive login doors (`bash -lc`, `ssh host cmd`) and the
brew-shellenv cache — is actually *not* a blocker (the standard pattern keeps bash as login shell and
launches nu interactively only, leaving every non-interactive door untouched). The blockers are the
bindings and atuin, and they are decisive.

**Reconsider only if** all three become true at once: (a) reedline gains real multi-key-sequence
keybindings *and* complete vi mode (track nushell/reedline; today: no); (b) atuin's nushell + daemon path
is first-class and documented to match this repo's host-filtered daemon config; (c) the operator's daily
work shifts toward structured-data/dataframe shell use where nushell's model pays for itself. Until then
the honest answer is that bash is the right tool here.

______________________________________________________________________

## 2. Roadmap SP4 criteria — scored with evidence

The criteria are the SP4 deferred-index bullet in the roadmap
(`2026-07-02-repo-modernization-roadmap-design.md`, lines 219–224).

| #   | Criterion                                                                             | Score                           | Evidence                                                                                                                                                                                                                                                                                                                                                                               |
| --- | ------------------------------------------------------------------------------------- | ------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| C1  | **reedline keybinding parity** with the current bash bindings                         | ❌ **FAIL (decisive)**          | reedline binds single `modifier+keycode`; **no multi-key chord sequences** (nushell book, line_editor.html). The repo has **365 `bind` lines + 53 helper functions**, ~all multi-key chords / macros ending in `\r`. vi mode incomplete (nushell#5226). Not portable — a from-scratch re-conception against a weaker editor.                                                           |
| C2  | **atuin** nushell integration (must cover this repo's **daemon** mode)                | ⚠️ **WEAK / high-risk**         | atuin supports nushell, but as its weakest shell: recording via nu hooks or the new experimental pty-proxy "hex" (blog v18.13); daemon+nushell coexistence undocumented; prior breakage (atuin#2838, nu 0.106). Repo runs daemon + `filter_mode=host` + LaunchAgent — exactly the config with the least nushell coverage. Live-installed atuin: **18.16.1**.                           |
| C3  | **starship / zoxide / carapace** integration                                          | ✅ **PASS** (carapace ≥ bash)   | `starship init nu`, `zoxide init nushell` are first-class. **carapace is arguably nushell's best-supported shell** (external-completer, 600+ commands). Live: starship 1.26.0, zoxide 0.9.9, carapace-bin 1.7.3.                                                                                                                                                                       |
| C4  | **direnv** integration                                                                | 🟡 **PASS w/ friction**         | Works via nu `pre_prompt`/`env_change` hooks; needs nu ≥ 0.104 (current 0.114). Community recipe, not an official one-liner like `direnv hook bash`. Live: direnv 2.37.1.                                                                                                                                                                                                              |
| C5  | **macOS login-shell semantics + brew-shellenv cache analog**                          | ✅ **PASS — not the blocker**   | Recommended pattern: **keep bash as login shell, launch nu interactively** (nushell book default_shell.html; nu is not POSIX and is discouraged as login shell). This leaves `dot_profile`, `dot_bash_profile`, the brew-shellenv cache, `bash -lc`, `ssh host cmd`, and the herdr `/bin/sh -lc` gate **entirely on bash, untouched**. Contains blast radius rather than expanding it. |
| C6  | **native `pre_prompt`/`pre_execution` hooks** replacing bash-preexec for the notifier | ✅ **PASS (rebuild, not port)** | nushell has native `pre_execution`/`pre_prompt` hooks (nushell book hooks.html) — cleaner than bash-preexec, no atuin-clobber concern. But it's a **rewrite** of the `__cmd_notify_*` block, not a port. Positive, but doesn't offset C1/C2.                                                                                                                                           |
| C7  | **herdr pane/spawn compatibility**                                                    | ✅ **PASS**                     | Nothing in herdr assumes bash. Keybindings exec via `/bin/sh -lc` or argv (`plugin_action`); smart-nav shells the `herdr` CLI; auto-attach runs bare `herdr`. Panes inherit `SHELL` (brew bash) → nu launches as interactive child. Live: herdr 0.7.0-preview.                                                                                                                         |
| C8  | **incremental migration path** (opt-in pane first, cutover last)                      | 🟡 **Possible but low-payoff**  | An opt-in nu pane is feasible (bash stays login shell; launch `nu` by hand in one pane). But the two blockers (C1 bindings, C2 atuin) hit on day one of any real use, so the "safe ramp" still front-loads the worst cost.                                                                                                                                                             |

**Net:** one decisive fail (C1), one high-risk weak link (C2) on the most-depended-on subsystem, against
five passes that are either "not actually the problem" (C5/C7), "fine but a rewrite" (C6), or "as good or
better than bash" (C3). The passes don't buy down the fails.

______________________________________________________________________

## 3. Migration-cost inventory — what a GO would have to rebuild

Ordered hardest-first. "Hard parts" flagged 🔴.

1. 🔴 **Keybindings (the whole reason this is a NO-GO).** `dot_bash_bindings` (299 `bind` + 24 fns) and
   `dot_fzf_bindings` (66 `bind` + 29 fns). Every binding is a readline construct reedline can't express:

   - **Multi-key chord trees** — `\C-g` `d` `r`, `\C-x` `n` `l` `t`, `\C-g` `c` `m`, etc. Reedline has no
     sequential-keypress keymap. Each would collapse to a single chord (namespace explosion) or move out
     of the line editor entirely.
   - **Macro bindings that inject text + `\r`** — the pervasive `"\C-x0<cmd>\r"` pattern (clear line →
     type command → Enter). Reedline's nearest equivalents are `edit: insertstring` and
     `ExecuteHostCommand`, one binding at a time, no `\C-x0`-style prefix composition.
   - **vi-insert + vi-command dual registration** — every logical binding is defined twice; reedline's
     `vi_insert`/`vi_normal` split is similar in spirit but the incomplete vi mode (nushell#5226) means
     the `i…\r` "enter insert, run, return" idiom used throughout vi-command bindings is not reliable.
   - **`bind -x` shell-function bindings** — 53 helper functions (git helpers, the network/DNS/LAN/WAN
     diagnostic suite, `__fzf_*` widgets, `__bash_bindings_*` project-root/commit-copy/list-bindings).
     Bodies are bash (arrays, `getopts`, `printf`, `tput`, `mapfile`, `READLINE_LINE`/`READLINE_POINT`
     manipulation). Rewriting in nu is a large, bug-prone port with no test harness today.
   - **`.inputrc` vi mode + readline settings** — no reedline equivalent file; folds into `config.nu`.
   - **Realistic estimate:** this is not a weekend. It is the single largest line-count + muscle-memory
     asset in the shell config, and the target editor is strictly less capable for this style.

1. 🔴 **atuin recording.** Re-init under nu (hooks or pty-proxy "hex"), then **prove** the daemon +
   `filter_mode=host` + LaunchAgent + `--force` self-heal + upgrade-bounce all still hold. This is the
   fragile one: the repo's existing atuin diagnostic ladder (CLAUDE.md) exists *because* atuin recording
   has broken 3× on bash already; nushell is a less-traveled path. Highest re-stabilization risk.

1. **Long-running-command notifier.** Rewrite the `__cmd_notify_preexec`/`__cmd_notify_precmd` block
   (`dot_bashrc.tmpl:282–323`) as nu `pre_execution`/`pre_prompt` hooks. Clean in nu (native hooks, no
   bash-preexec dependency), but a full rewrite incl. the TUI-skip list, `$SECONDS` timing, exit-code
   capture, and the `≥60s local / ≥300s full+hue` thresholds. **Note:** SP3 moves those thresholds into
   the Rust binary anyway, so this hook shrinks to "measure duration, exec the binary."

1. **Init ordering + PROMPT_COMMAND semantics.** bash's ordered `PROMPT_COMMAND` writers (direnv →
   starship → zoxide → atuin, atuin last) become nu hook-list ordering. Different model
   (`pre_prompt`/`env_change` hook arrays), needs re-derivation and testing; the "atuin last, after
   zoxide, both after starship" invariant has to be re-established in nu's terms.

1. **Aliases + interactive niceties.** `cd=z`, eza/bat/dust/procs aliases, `rm=trash-put`, navigation
   aliases, the mosh/ssh `STARSHIP_CONFIG` branch, `stty -ixon`, flow-control. Mostly mechanical in
   `config.nu` (chezmoi `.tmpl` templating is fine — C5), but each is a line to re-verify.

1. **carapace / starship / zoxide / direnv re-init.** Mechanical (C3/C4). Lowest risk.

**Not migrated (stays bash — this is the containment win):** `dot_bashrc.tmpl` non-interactive PATH/env
half, `dot_profile`, `dot_bash_profile`, brew-shellenv cache (`run_after_44` + self-heal + test),
`~/.bash_functions`, all non-interactive login doors, herdr `/bin/sh -lc` gate. `SHELL` stays brew bash;
nu is an interactive child only.

______________________________________________________________________

## 4. Integration maturity — today (mid-2026), with citations

| Tool                         | Live ver             | nushell status                                                                                      | Grade            | Source                                                                                                                                                                                          |
| ---------------------------- | -------------------- | --------------------------------------------------------------------------------------------------- | ---------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **nushell**                  | 0.114.0 (2026-07-04) | pre-1.0; 4-wk cadence; config-schema breaks every 2–5 releases                                      | —                | [releases](https://github.com/nushell/nushell/releases), [road-to-1.0](https://www.nushell.sh/blog/2023-06-27-road-to-1_0.html), [#14102](https://github.com/nushell/nushell/discussions/14102) |
| **reedline** (keybindings)   | bundled              | **no multi-key chords; vi mode incomplete**                                                         | 🔴 blocker       | [line_editor](https://www.nushell.sh/book/line_editor.html), [nushell#5226](https://github.com/nushell/nushell/issues/5226)                                                                     |
| **atuin**                    | 18.16.1              | supported but weakest shell; hooks or pty-proxy "hex"; daemon+nu undocumented; prior nu-0.106 break | 🟠 high-risk     | [v18.13 blog](https://blog.atuin.sh/atuin-v18-13/), [atuin#2838](https://github.com/atuinsh/atuin/issues/2838), [shell-integration](https://docs.atuin.sh/cli/guide/shell-integration/)         |
| **carapace**                 | 1.7.3                | first-class external completer, 600+ cmds; ≥ bash                                                   | 🟢 strong        | [carapace](https://github.com/carapace-sh/carapace), [external_completers](https://www.nushell.sh/cookbook/external_completers.html)                                                            |
| **starship**                 | 1.26.0               | `starship init nu`, first-class                                                                     | 🟢 strong        | [external_completers cookbook / starship docs](https://www.nushell.sh/cookbook/external_completers.html)                                                                                        |
| **zoxide**                   | 0.9.9                | `zoxide init nushell`, first-class                                                                  | 🟢 strong        | [zoxide#69](https://github.com/ajeetdsouza/zoxide/issues/69)                                                                                                                                    |
| **direnv**                   | 2.37.1               | `pre_prompt`/`env_change` hooks, needs nu ≥ 0.104; community recipe                                 | 🟡 workable      | [direnv cookbook](https://www.nushell.sh/cookbook/direnv.html), [direnv#1175](https://github.com/direnv/direnv/pull/1175)                                                                       |
| **login-shell / brew cache** | —                    | keep bash login shell, nu interactive-only (nu not POSIX)                                           | 🟢 not-a-blocker | [default_shell](https://www.nushell.sh/book/default_shell.html), [nushell#10316](https://github.com/nushell/nushell/issues/10316)                                                               |
| **native hooks (notifier)**  | —                    | `pre_execution`/`pre_prompt` exist; cleaner than bash-preexec                                       | 🟢 (rebuild)     | [hooks](https://www.nushell.sh/book/hooks.html)                                                                                                                                                 |
| **chezmoi templating**       | —                    | `config.nu.tmpl` is an ordinary Go template; no special issue                                       | 🟢               | (repo convention)                                                                                                                                                                               |
| **herdr**                    | 0.7.0-preview        | no bash assumption; panes inherit `SHELL`                                                           | 🟢               | (repo CLAUDE.md, live)                                                                                                                                                                          |

______________________________________________________________________

## 5. What this verdict means for SP3's shell-hook seam

This evaluation ran early *specifically* to unblock SP3's seam design. The answer is clean:

- **The shell stays bash.** SP3 designs its shell seam against **bash-preexec exactly as today** — the
  long-running-command notifier remains a bash-preexec block in `dot_bashrc.tmpl` that execs the new Rust
  binary once per command. No nushell-agnostic hook abstraction is needed. **Do not build a
  shell-portability layer** for the notifier — that would be YAGNI against a decision that just landed.

- **Zero nushell design debt, by construction.** SP3's own contract already makes the binary a
  **stateless, per-event CLI** invoked over a shell-agnostic stdin/flag contract (roadmap SP3:
  "Invocation model: stateless per-event CLI"). That contract is *already* shell-neutral. So in the
  unlikely future where an opt-in nu pane appears, nushell's native `pre_execution` hook can call the
  **same binary with the same arguments** — no SP3 rework. SP3 therefore assumes bash now at no cost and
  no lock-in. This is the ideal seam: bash-specific glue (the preexec wiring) stays thin and in the shell
  config; the Rust service knows nothing about the shell.

- **Concrete SP3 guidance:** keep the preexec/precmd shim in bash; keep it dumb (measure duration, gather
  `PWD`/exit-code/`HERDR_PANE_ID`, exec the binary); put all routing/threshold logic in Rust. The
  thresholds (`≥60s`, `≥300s`) move into the service config per the SP3 contract. The shim is the only
  bash the notifier needs, and it is trivially re-expressible as a nu hook later if ever required.

______________________________________________________________________

## 6. Sources

- Nushell releases & cadence: https://github.com/nushell/nushell/releases · road-to-1.0
  https://www.nushell.sh/blog/2023-06-27-road-to-1_0.html · v1.0 stability discussion
  https://github.com/nushell/nushell/discussions/14102
- reedline / keybindings / vi mode: https://www.nushell.sh/book/line_editor.html ·
  https://github.com/nushell/reedline · vi-mode gaps https://github.com/nushell/nushell/issues/5226
- Nushell hooks: https://www.nushell.sh/book/hooks.html
- Login-shell semantics: https://www.nushell.sh/book/default_shell.html ·
  https://github.com/nushell/nushell/issues/10316
- atuin nushell + daemon + pty-proxy: https://blog.atuin.sh/atuin-v18-13/ ·
  https://docs.atuin.sh/cli/guide/shell-integration/ · https://docs.atuin.sh/cli/reference/daemon/ ·
  nu-0.106 deprecation bug https://github.com/atuinsh/atuin/issues/2838
- carapace: https://github.com/carapace-sh/carapace ·
  https://www.nushell.sh/cookbook/external_completers.html
- zoxide nushell: https://github.com/ajeetdsouza/zoxide/issues/69
- direnv nushell: https://www.nushell.sh/cookbook/direnv.html ·
  https://github.com/direnv/direnv/pull/1175
- Live machine (dresden, 2026-07-09): nu 0.114.0, atuin 18.16.1, starship 1.26.0, carapace-bin 1.7.3,
  zoxide 0.9.9, direnv 2.37.1, herdr 0.7.0-preview. Binding surface: `dot_bash_bindings` 299 `bind` + 24
  fns; `dot_fzf_bindings` 66 `bind` + 29 fns. No existing `~/.config/nushell`.
