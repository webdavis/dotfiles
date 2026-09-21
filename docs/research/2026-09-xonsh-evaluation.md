# xonsh evaluation, 2026-09-14

SP5 of the ledger asks one question: should the interactive shell on dresden move from Bash to
[xonsh](https://xon.sh), and what does the measured evidence say. The ledger bullet names the
integrations that have to be scored (cold and warm startup against Bash, atuin, the shell hooks,
starship, direnv, zoxide, carapace, Herdr attachment and the existing key chords), states that scripts
stay Bash regardless, and reserves the migration decision itself for the operator.

Nothing was installed through chezmoi, no managed manifest was edited, no code was written, and no
formula was added to Homebrew. xonsh was measured from a throwaway virtual environment in the session
scratchpad. Traces left on the machine are listed at the end.

## Verdict

**NO-GO on current evidence. Keep Bash as the interactive shell.**

Update 2026-09-21: the operator approved this no-go. Bash stays the interactive shell and SP5 is closed.
Xonsh is installed from Homebrew anyway, as an on-demand subshell the operator starts by typing `xonsh`,
never as the login shell.

The migration is technically possible, and it clears the two bars that sank Nushell in
`docs/research/2026-07-09-sp4-nushell-evaluation.md`: the key chord surface ports (prompt_toolkit
dispatches multi-key sequences, verified in a live session), and atuin has first-class xonsh support that
reuses the same `atuin history start/end` calls, so the daemon plus `filter_mode = "host"` configuration
carries over untouched. Three measured costs decide it anyway.

1. **Interactive startup roughly doubles.** Two interleaved measurement passes put the fully configured
   xonsh shell at 1.89x and 1.95x the median time to first prompt of the deployed `~/.bashrc`, and the
   gap is the Python interpreter floor, which no amount of configuration tuning removes. Every new Herdr
   pane pays it.
1. **A permanent per-command tax of roughly 20 to 35 ms.** atuin's Bash hook backgrounds
   `atuin history end`; its xonsh hook cannot, because of xonsh issue 5224, which upstream **closed as
   not planned**. The generated xonsh snippet carries the disabled background call and the issue link as
   a comment. That call therefore blocks the prompt after every single command, forever, unless upstream
   reverses that decision.
1. **The runtime is fragile in exactly the way this machine is managed.** xonsh's own installation
   guidance says setting it as the default login shell "is not recommended" and that a core-shell xonsh
   needs a Python environment kept "stable, predictable, and independent of system changes", explicitly
   adding that `venv`, `pipx` and `rye` "do not fully address this requirement" and pointing at Miniconda
   or Micromamba. The Homebrew formula (`xonsh 0.24.2`, dependency `python@3.14`) is the install method
   that guidance warns against, and this repository upgrades Homebrew **unattended** every week through
   uu's brew lane. The alternative is adding an isolated Python installer to a toolchain the global rules
   list as locked in.

Against those three sits the value proposition, Python expressions in the shell. This machine's tooling
is Rust (pns, uu, posture, lights, two Herdr plugins) and Bash (every script under `libexec`, every
`.chezmoiscripts` runner), with Python present only as uv-installed formatters. The rewrite bill is 1,926
lines of binding configuration and 77 helper functions, and it buys a capability the workload does not
currently ask for.

The evaluation also produced a Bash finding worth more than the migration: **bash-completion@2 is roughly
220 ms of the deployed shell's roughly 400 ms startup**, measured, while carapace already covers about
700 commands with case-insensitive matching. That is the largest interactive-startup win available on
this machine and it is shell-independent. It belongs to SP4.

## Decisions made in the operator's place

Each is an assumption this document makes so the work could finish overnight, with the alternative
stated. None is a commitment.

- **No migration specification was written.** A no-go makes a migration plan moot, and the Nushell
  evaluation set the precedent of shipping criteria plus a cost inventory instead. The ledger's
  `done_means` asks for "a reviewed spec and a go/no-go verdict", so if the operator reads that as
  requiring the plan regardless of verdict, the plan is still owed. The alternative is a follow-up task
  that writes the migration specification against this document's cost inventory.
- **This document has not been through the adversarial review step.** It is one agent's research. The
  "reviewed" half of `done_means` is an operator-scheduled pipeline step, not something this task can
  self-certify.
- **xonsh was measured from an ephemeral uv virtual environment**, xonsh 0.24.2 on CPython 3.13.5, rather
  than from `brew install xonsh`. Homebrew ships the same 0.24.2 on `python@3.14`, so the numbers should
  be close, but they are not the deployed artifact. The alternative is installing the formula and
  re-measuring, which is a machine change this task was not authorized to make.
- **The port of the key binding surface was modelled as a mechanical one-to-one mapping**, 353 generated
  prompt_toolkit bindings with vi-mode filters, to measure registration and parse cost honestly. A
  redesigned, smaller surface (which is what SP4's binding table would produce) would measure lower. The
  alternative is to re-measure after SP4 lands its table.
- **"Cold" is read as the first shell start after the configuration changes** (the xonsh compiled-code
  cache misses) and "warm" as every start after that. The alternative reading, first start after a reboot
  with cold page cache, could not be measured without rebooting the machine.
- **The modelled migration keeps Bash as the login shell and keeps `SHELL` pointing at Bash**, with xonsh
  launched as an interactive child. This is both upstream's recommendation and the containment pattern
  the Nushell evaluation identified. The alternative, `chsh` to xonsh, sends fzf previews and every other
  `$SHELL -c` door through a non-POSIX shell (see the fzf finding below).
- **Two integrations the ledger bullet did not name were scored anyway**, fnm and bash-completion@2,
  because `dot_bashrc.tmpl` loads both and both behave differently under xonsh. The alternative is to
  score only the nine named items, which would have understated the bill.

## What was checked and how

Machine: dresden, Apple M1, macOS 26.2 (build 25C56), Homebrew prefix `/opt/homebrew`.

Installed versions read from the binaries on this machine: GNU Bash 5.3.15 (Homebrew; the system
`/bin/bash` is 3.2.57), atuin 18.22.0, starship 1.26.0, zoxide 0.10.0, carapace-bin 1.7.3, direnv 2.37.1,
fzf 0.74.4, fnm 1.39.0, hyperfine 1.20.0, Herdr (the `herdr` binary at `~/.local/bin/herdr`).

Candidate: xonsh 0.24.2 with prompt_toolkit 3.0.53, installed into `<scratchpad>/xonsh-venv` with
`uv venv --python 3.13` plus `uv pip install 'xonsh[full]'`, and `xonsh-direnv 1.6.5` added later for the
direnv test. PyPI (Python Package Index) reports 0.24.2 as the current release, requiring Python 3.11 or
newer; Homebrew's formula is also 0.24.2 and depends on `python@3.14`.

Measurement methods:

- **Time to first prompt** was measured with a pseudoterminal harness (Python `pty.fork`, read until a
  marker string appears in the terminal output, then send `exit`). Both shells were instrumented to print
  the same marker at prompt time: Bash through an extra `PROMPT_COMMAND` entry appended after sourcing
  the real `~/.bashrc` (so starship, direnv, zoxide and atuin all still run and still write
  `PROMPT_COMMAND` in their documented order), xonsh through a `$PROMPT` wrapper appended after the
  integrations set it. Runs were **interleaved round-robin** across configurations, because this
  machine's load average moved between 18 and 102 during the session (overnight agents) and sequential
  blocks drifted with it.
- **xonsh's own startup breakdown** came from `xonsh --timings`, which prints an event table ending at
  `on_pre_prompt`. The `on_pre_rc` to `on_post_rc` delta isolates configuration cost from interpreter
  floor.
- **Component costs** came from `hyperfine --shell=none` with 15 to 20 runs and 3 warmup runs.
- **Key binding behavior** was tested two ways: prompt_toolkit's own `KeyProcessor` inside an asyncio
  event loop (dispatch of a given key sequence to a given handler), and a live interactive xonsh under a
  pseudoterminal with control bytes written directly to the terminal file descriptor.
- **Upstream claims** were read from the installed binaries' `--help` output first, then from xon.sh, the
  xonsh GitHub issue tracker and PyPI. The fzf `$SHELL` behavior came from the installed manual page, not
  the website.

**Load caveat, which applies to every absolute number below.** The machine carried a load average between
18 and 102 for the whole session. Absolute milliseconds are therefore inflated, in some cases by a factor
of two. What survives that is the **ratio between configurations measured in the same interleaved pass**,
and the minima, which approach an idle machine. Every conclusion here rests on ratios or on
order-of-magnitude gaps, never on a single absolute figure. The reproduction recipe at the end re-runs
the comparison in about three minutes on an idle machine.

## The ledger's criteria, scored

| #   | Criterion                                       | Score                                  | Evidence                                                                                                                                                                                                                                                                                                             |
| --- | ----------------------------------------------- | -------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Cold startup                                    | **fail**                               | First start after an rc edit: 2.5 to 3.6 s for a 1,064-line binding file, because xonsh parses and compiles it before the prompt. Bash's equivalent is 18 ms.                                                                                                                                                        |
| 2   | Warm startup                                    | **fail**                               | 1.89x and 1.95x Bash's median time to first prompt across two interleaved passes. The gap is the interpreter floor, not the configuration.                                                                                                                                                                           |
| 3   | atuin                                           | **pass with a permanent tax**          | `atuin init xonsh` is first-class and uses the same `history start/end` calls, so daemon mode and `filter_mode = "host"` carry over. But `history end` runs synchronously (xonsh issue 5224, closed as not planned), costing 19 to 57 ms after every command.                                                        |
| 4   | Shell hooks (the long-running command notifier) | **pass, as a rewrite**                 | `events.on_precommand` and `events.on_postcommand` are native and hand the hook the command, the return code and a start/end timestamp pair, so bash-preexec disappears and the `__cmd_notify_*` pair becomes a short Python function that calls the `pns` binary.                                                   |
| 5   | starship                                        | **pass**                               | `starship init xonsh --print-full-init` is first-class. Per-prompt cost is identical to Bash's, because both spawn `starship prompt`.                                                                                                                                                                                |
| 6   | direnv                                          | **weak pass, unmaintained dependency** | direnv has no xonsh hook (`direnv hook xonsh` is rejected by the installed 2.37.1). The only path is the community `xonsh-direnv` xontrib, last released in 2024 and documented against xonsh 0.18.3. It does work on 0.24.2 (verified interactively). A 12-line hand-rolled hook also works.                        |
| 7   | zoxide                                          | **pass**                               | `zoxide init xonsh` is first-class and listed in the installed binary's own shell list.                                                                                                                                                                                                                              |
| 8   | carapace                                        | **pass**                               | `carapace _carapace xonsh` is first-class and xonsh is in the binary's documented shell list. `CARAPACE_MATCH=1` is an environment variable, so case-insensitive matching for the roughly 700 carapace-covered commands survives the move.                                                                           |
| 9   | Herdr attachment                                | **pass, untouched**                    | `dot_config/herdr/config.toml` sets `default_shell = "bash"`, and Herdr runs plugin actions as argv and command keybindings through `/bin/sh -lc`, neither of which reads the interactive shell. A migration edits one config line and the `~/.bashrc` auto-attach block; nothing in Herdr assumes Bash beyond that. |
| 10  | Existing key chords                             | **pass for 328 of 353, fail for 25**   | Multi-key Control chords port and dispatch correctly. Escape-prefixed (Meta or Alt) bindings do not work in vi mode, which is the repo's default editing mode.                                                                                                                                                       |

Two items the bullet did not name:

| #   | Criterion         | Score                                              | Evidence                                                                                                                                                                                                                                                      |
| --- | ----------------- | -------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 11  | fnm cd hook       | **hand-roll required**                             | `fnm env --shell` accepts bash, zsh, fish and powershell only. `fnm env --json` exists, so an `on_chdir` hook is about 15 lines.                                                                                                                              |
| 12  | bash-completion@2 | **partial, and probably should be dropped anyway** | xonsh reads bash completion scripts through `$BASH_COMPLETIONS` (populated by default, including the Homebrew paths), but by spawning Bash. Measured on the Bash side, sourcing bash-completion@2 costs roughly 220 ms of the shell's roughly 400 ms startup. |

## Startup measurements

### Time to first prompt, interleaved

Median of 12 interleaved rounds per configuration, milliseconds. Pass A and Pass B are separate
interleaved runs at different machine loads, both with xonsh's compiled-code cache primed.

| Configuration                              | A min | A median | B min | B median |
| ------------------------------------------ | ----- | -------- | ----- | -------- |
| Bash, deployed `~/.bashrc`                 | 344   | 411      | 484   | 586      |
| xonsh, no configuration at all             | 330   | 408      | 484   | 577      |
| xonsh + atuin, starship, zoxide, carapace  | 707   | 808      | 1022  | 1148     |
| xonsh + those + 353 generated key bindings | 680   | 803      | 1020  | 1106     |

Read three things off that table.

- **An empty xonsh costs about what the entire deployed Bash configuration costs.** The Python
  interpreter plus xonsh plus prompt_toolkit import is the whole budget Bash spends on bash-completion,
  five init generators, 353 bindings and 77 functions combined.
- **The four integrations roughly double xonsh again**, and the key bindings then add nothing measurable,
  because their compiled form is cached.
- **The fully configured ratio is 1.89x to 1.95x Bash**, consistent across two passes at different loads.

`xonsh --timings` decomposes the floor the same way: with no rc, `on_ptk_create` (the prompt_toolkit
import) took 95 to 96 ms and `on_pre_cmdloop` a further 74 to 103 ms, reaching `on_pre_prompt` at 280 to
301 ms.

### Cold start, the edit-test loop

The first xonsh start after the configuration file changes pays a full parse and compile. Measured on the
`on_pre_rc` to `on_post_rc` delta, with a fresh filename to force a cache miss:

| Configuration file         | Lines | First start                                    | Later starts     |
| -------------------------- | ----- | ---------------------------------------------- | ---------------- |
| 353 generated key bindings | 1,064 | 2.50 to 3.42 s                                 | 0.118 to 0.144 s |
| Four integration snippets  | 4     | about 0.40 s                                   | about 0.38 s     |
| Both together              | 1,068 | 3.57 to 3.65 s (time to prompt 3.81 to 3.88 s) | 0.37 to 0.41 s   |
| A 304-line subset          | 304   | 0.53 s                                         | 0.51 to 0.58 s   |

The mechanism is `xonsh/codecache.py`: `run_script_with_cache` marshals the compiled code object into
`$XONSH_DATA_DIR/xonsh_script_cache`, and reuses it while the cache file's modification time is at or
after the source's. So the parse is paid once per edit, and **every chezmoi apply that rewrites the
configuration makes the next new pane cost about three seconds**. A syntax error is worse: it is never
cached, so a broken rc pays the full parse on every start (measured on a deliberately truncated fixture,
2.5 to 2.9 s per start, with `xonsh check` reporting the exact line). The integration snippets are not
cached at all, because `execx($(...))` compiles a string rather than a file, which is why their roughly
0.4 s recurs on every start.

Note against a claim not made here: an earlier run of this measurement appeared to show the cache never
working, which turned out to be a redirected `XONSH_DATA_DIR` in the harness plus one syntactically
invalid fixture. The table above is from the default data directory with the anomaly explained.

### Where Bash's own 400 ms goes

hyperfine, `--shell=none`, 15 runs, load average about 31. Each row is a full interactive Bash start with
only the named piece loaded.

| What is loaded                                                            | Mean     | Range          |
| ------------------------------------------------------------------------- | -------- | -------------- |
| `bash --norc --noprofile -i -c exit` (floor)                              | 30.4 ms  | 13.7 to 56.3   |
| floor + bash-completion@2 only                                            | 253.1 ms | 172.8 to 317.4 |
| floor + `.bash_bindings` and `.fzf_bindings` (353 bindings, 55 functions) | 47.8 ms  | 32.4 to 89.5   |

And the five init generators, each a subprocess spawn whose output is evaluated (load average about 35):

| Generator | Bash form | xonsh form      |
| --------- | --------- | --------------- |
| atuin     | 24.7 ms   | 18.5 ms         |
| starship  | 18.4 ms   | 17.2 ms         |
| zoxide    | 15.7 ms   | 21.0 ms         |
| direnv    | 37.3 ms   | no xonsh target |
| carapace  | 26.1 ms   | 31.6 ms         |

The spawns cost the same either way, which is the point: the shells differ in what they do with the
output, not in fetching it. Bash `eval`s shell text; xonsh parses and compiles Python-flavoured source
with its own parser, uncached.

The actionable part is the middle row of the first table. **bash-completion@2 is about 220 ms of Bash's
roughly 400 ms**, while the 353 bindings and their helper functions cost about 18 ms. carapace already
registers a completer for roughly 700 commands with `CARAPACE_MATCH=1`, and `dot_bashrc.tmpl` already
notes that git and gh completions come from carapace and Homebrew's git completion. Lazy-loading or
dropping bash-completion@2 is therefore the largest single interactive-startup win on this machine, it is
independent of the shell question, and it is SP4 work.

### Per-command and per-prompt costs

hyperfine, 15 to 20 runs, load average about 22 to 35.

| Call                                                        | Cost                           | Who pays it                                    |
| ----------------------------------------------------------- | ------------------------------ | ---------------------------------------------- |
| `starship prompt`                                           | 119.6 ms mean (96.3 to 148.7)  | both shells, every prompt                      |
| `direnv export` inside a directory with an allowed `.envrc` | 143.3 ms mean (100.4 to 193.0) | both shells                                    |
| `direnv export` elsewhere                                   | 10.8 ms mean (5.2 to 32.5)     | both shells                                    |
| `atuin history start --hook`                                | 27.0 ms mean (19.6 to 33.6)    | both shells, synchronous in both               |
| `atuin history end --hook`                                  | 32.9 ms mean (19.2 to 56.8)    | **backgrounded in Bash, synchronous in xonsh** |
| `atuin --version` (the binary's own spawn floor)            | 31.1 ms mean (12.5 to 59.0)    | bounds how much the daemon can save            |

The atuin calls were measured against a throwaway database in the scratchpad with the daemon disabled, so
the operator's own history was not written to. With the daemon enabled the sqlite write moves off
process, but the binary spawn floor (the last row) stays, so the synchronous `history end` in xonsh still
costs on the order of 15 to 30 ms after every command. That is the tax item 3 of the verdict names.

## The key chord surface

Inventory, counted from `dot_bash_bindings` (1,089 lines) and `dot_fzf_bindings` (837 lines):

| Property                                             | Count                                                              |
| ---------------------------------------------------- | ------------------------------------------------------------------ |
| binding statements                                   | 360 (353 with a quoted key sequence, 7 in `Control-a:` form)       |
| multi-key sequences                                  | 308, prefixed `\C-g` (144), `\C-x` (104), `\C-_` (28), `\C-t` (27) |
| macro bindings that type a command and press Enter   | 170                                                                |
| `bind -x` bindings to shell functions                | 32                                                                 |
| dual registration                                    | 142 `vi-insert` plus 134 `vi-command`                              |
| helper functions in the two binding files            | 55, plus 22 in `dot_bash_functions`                                |
| lines that touch `READLINE_LINE` or `READLINE_POINT` | 4, in 2 functions                                                  |

What was verified about porting that to xonsh, which drives prompt_toolkit:

- **Multi-key sequences dispatch.** `('c-g','d','r')`, `('c-x','d','c')`, `('c-t','c-t')` and
  `('escape','l')` all reached their handlers through prompt_toolkit's own `KeyProcessor`. Every prefix
  key this repository uses (`c-g`, `c-x`, `c-t`, `c-_`, `c-s`, `c-w`, `escape`, `s-tab`) is a valid
  prompt_toolkit key name. This is the single biggest difference from Nushell, whose line editor has no
  sequential keymap at all.
- **The macro idiom ports, and was run for real.** In a live interactive xonsh with `$VI_MODE = True`,
  pressing `Ctrl-g d r` ran a handler that cleared the buffer, inserted `echo XCHORDXOK` and submitted
  it, and the command's output came back. That is the exact shape of all 170 `"\C-x0<cmd>\r"` macros, and
  it does not need readline's prefix-composition trick, so the `\C-x0` / `\C-x1` / `\C-x2` helper
  bindings disappear rather than needing equivalents.
- **Registration is cheap.** 353 three-key bindings with a `ViInsertMode()` filter registered in 3.0 ms.
- **Escape-prefixed bindings do not work in vi mode.** 25 of the 353 bindings use `\M-` or a bare escape
  prefix (14 `vi-insert`, 10 `vi-command`, 1 emacs). In a live session with `$VI_MODE = True`, Meta-l did
  not reach its handler; the terminal switched to the vi navigation cursor instead, because
  prompt_toolkit's vi bindings claim bare escape (`prompt_toolkit/key_binding/bindings/vi.py`, the
  `@handle("escape")` handler). Adding `eager=True` did not change it. The same binding fires correctly
  with `$VI_MODE = False`. `dot_inputrc` sets `editing-mode vi`, so vi is the primary mode here and those
  25 bindings need new chords or a custom conflict resolution.
- **The helper function bodies need not be ported.** `source-bash` imports a Bash file's exported
  variables, aliases and functions into xonsh (functions arrive as callable aliases). Sourcing the real
  `~/.bash_functions` produced 68 aliases in about 120 ms. Only the two functions that manipulate
  `READLINE_LINE` and `READLINE_POINT` genuinely have to become prompt_toolkit handlers. Note the side
  effect: `source-bash` starts a login Bash, which on this machine re-runs the whole interactive
  configuration (the `stty` warnings from `dot_bash_bindings` appear), so it is not free and not
  side-effect free.
- **`.inputrc` mostly has equivalents.** xonsh 0.24.2 registers `COMPLETIONS_DISPLAY`,
  `COMPLETIONS_MENU_ROWS`, `COMPLETIONS_CONFIRM`, `COMPLETION_QUERY_LIMIT`,
  `UPDATE_COMPLETIONS_ON_KEYPRESS`, `VI_MODE`, `MOUSE_SUPPORT` and others. `CASE_SENSITIVE_COMPLETIONS`
  appears only in xonsh's legacy readline shell, not in the prompt_toolkit shell's environment, so
  `completion-ignore-case` and `completion-map-case` have no prompt_toolkit-shell equivalent found. In
  practice `CARAPACE_MATCH=1` covers the roughly 700 carapace commands, which is most of where that
  setting is felt.

So the chord surface is a **rewrite of roughly 1,926 lines into Python**, with 25 chords needing
redesign, but it is not the impossible port that stopped Nushell. It is also the part that SP4 is already
planning to restructure.

## Integration findings worth carrying forward

**atuin is the surprise, in xonsh's favour.** `atuin init xonsh` registers `on_precommand` and
`on_postcommand` hooks that call the same `atuin history start/end` subcommands the Bash hook calls, so
`[daemon] enabled`, `filter_mode = "host"`, `enter_accept = true` and the LaunchAgent-managed daemon all
keep working with no new mechanism. Ctrl-R is bound through `on_ptk_create` and honours the same
`--disable-up-arrow` flag `dot_bashrc.tmpl` already passes. The search result protocol
(`__atuin_accept__:` on stderr, then `buffer.validate_and_handle()`) mirrors the Bash design. The one
defect is the synchronous `history end` described above, and it is upstream-blocked.

**direnv is the weakest link.** The installed direnv 2.37.1 accepts bash, zsh, fish, tcsh, elvish, murex
and pwsh, and rejects `xonsh` outright. The community `xonsh-direnv` 1.6.5 does work on 0.24.2 (verified
in a live session: an allowed `.envrc` exported its variable), but its README documents xonsh 0.18.3, its
changelog explicitly mentions xonsh "has a history of breaking its built-ins", and its last activity is
2024\. Its implementation is 30 lines and calls `direnv export json` on post-init, on chdir, and
unconditionally after **every command**, so a hand-rolled 12-line replacement (verified working) is the
better dependency posture if this ever proceeds. Cost is the same either way, and Bash pays the same
per-prompt direnv cost through `direnv hook bash`.

**fzf constrains `SHELL`, not the shell.** The installed manual page states that fzf runs preview,
reload, execute and become commands with `$SHELL -c` when `SHELL` is set. `dot_bashrc.tmpl` exports
`SHELL` as Homebrew Bash, and the repository has 64 fzf bindings with 29 helper functions whose preview
and action commands are Bash. If a migration ever changed `SHELL`, every one of those would run under a
non-POSIX shell. Two mitigations exist (leave `SHELL` as Bash, which is also upstream's advice, or pass
`--with-shell 'bash -c'`), and fzf ships shell integration for bash, zsh, fish and nushell but not xonsh.

**Herdr is genuinely unaffected.** `default_shell = "bash"` is one line of
`dot_config/herdr/config.toml`, plugin actions are exec'd as argv, and command keybindings go through
`/bin/sh -lc`. The auto-attach block at the end of `dot_bashrc.tmpl` would need to move or stay in Bash,
which it can, since the containment pattern keeps Bash as the login shell.

**Scripts and every non-interactive door stay Bash**, exactly as the ledger requires: the non-interactive
half of `dot_bashrc.tmpl` (the PATH assembly and the brew shellenv cache), `dot_profile`,
`dot_bash_profile`, `~/.bash_functions`, everything under `~/.local/libexec`, every `.chezmoiscripts`
runner, `bash -lc` and `ssh host cmd`. This is the containment win the Nushell evaluation identified, and
it holds here for the same reason.

## Costs this repository would pay, beyond the shell itself

- **Two new treefmt formatters.** shellcheck and shfmt both exclude `*.tmpl` and neither reads Python.
  xonsh ships `xonsh format`, `xonsh check` and `xonsh lint` (all three work: `xonsh check` caught a
  truncated fixture at the exact line, `xonsh lint` reported an unused import), so the gate is buildable,
  but it is new `treefmt.toml` entries plus a render-then-check script for a `dot_xonshrc.tmpl`,
  mirroring `scripts/treefmt/shellcheck-rendered-template.sh`.
- **A live trap in the existing classifier.** `scripts/treefmt/shellcheck-rendered-template.sh` is handed
  every root `dot_*.tmpl`, and its `is_shell_template` test accepts any shebang line matching the
  substring `sh`. `#!/usr/bin/env xonsh` matches. Reproduced: the file would be rendered and
  shellchecked, and shellcheck fails it with SC1008 and SC1073. The fix is one line (tighten the match or
  omit the shebang), but it has to be known before the first `.xsh.tmpl` lands, or the drift gate goes
  red on a file nobody can lint.
- **Toolchain hand-syncs.** xonsh would need adding to `Brewfile.dev`, to the lint workflow's toolchain
  step and to `.chezmoidata/system_packages_autoinstall.yaml`, three places the repository documents as
  hand-synced with nothing enforcing agreement.
- **A second runtime in the boot path of the operator's daily tool.** Homebrew's xonsh depends on
  `python@3.14`. uu's brew lane upgrades Homebrew unattended every week. Upstream's own guidance is that
  a core-shell xonsh needs a Python isolated from exactly that kind of change.

## Ecosystem size, for calibration

Homebrew install analytics, read from `brew info`, 30-day install-on-request counts: xonsh 169, nushell
4,336, fish 6,580, zsh 16,767, bash 23,070. xonsh has roughly one twenty-sixth of Nushell's Homebrew
adoption, and Nushell was already judged a thin ecosystem in the 2026-07-09 evaluation. Release cadence
is healthy though: 12 releases between 2026-04-20 (0.23.0) and 2026-08-23 (0.24.2), roughly one every ten
days, which cuts both ways (active maintenance, and a re-stabilization tax on a solo machine).

## What would change the verdict

Any of the first three would remove a specific blocker; the fourth and fifth would change the value side;
the last is the cheap lever this repository controls.

1. **xonsh's interpreter floor drops below roughly 150 ms**, so that a fully configured xonsh reaches the
   prompt no slower than the deployed Bash. Today an empty xonsh already costs what all of Bash's
   configuration costs.
1. **xonsh issue 5224 is reopened and fixed**, letting atuin's xonsh hook background `atuin history end`
   the way its Bash hook does, removing the per-command tax.
1. **direnv ships `direnv hook xonsh` and fnm ships `--shell xonsh`**, removing the two hand-rolled or
   unmaintained integrations.
1. **Upstream drops the "not recommended as a login shell" guidance**, or the operator accepts an
   isolated Python installer (Miniconda or Micromamba) into the locked-in toolchain and accepts that the
   interactive shell now depends on it.
1. **The daily workload shifts toward Python-shaped shell work**, where inline Python expressions and
   structured pipelines pay for the move. Today the tooling is Rust and Bash.
1. **SP4 builds its binding table as a shell-agnostic data table with a renderer.** That is the lever: if
   the 1,926-line binding layer becomes data plus one readline renderer, then re-evaluating any shell
   later costs one more renderer instead of a rewrite, and this verdict becomes cheap to revisit rather
   than expensive. Recommend building it that way regardless of the shell decision.

## How to re-measure on an idle machine

Every number here was taken under load. This reproduces the headline comparison in about three minutes
with no installation, no managed-manifest change and nothing left outside a scratch directory. Run it
with no agents working.

```bash
set -euo pipefail
work="$(mktemp -d)"
uv venv --python 3.13 "$work/venv"
VIRTUAL_ENV="$work/venv" uv pip install 'xonsh[full]'

# xonsh's own startup breakdown, ending at on_pre_prompt
printf 'exit()\n' | script -q /dev/null "$work/venv/bin/xonsh" --timings -i --no-rc \
  | tr -d '\r' | sed 's/\x1b\[[0-9;?]*[a-zA-Z]//g' | grep '^|'

# the four integrations, warm (run twice; the second run is the warm one)
cat > "$work/rc.xsh" <<'RC'
execx($(starship init xonsh --print-full-init))
execx($(zoxide init xonsh), 'exec', __xonsh__.ctx, filename='zoxide')
exec($(carapace _carapace xonsh))
execx($(atuin init xonsh --disable-up-arrow))
RC
for _ in 1 2; do
  printf 'exit()\n' | script -q /dev/null "$work/venv/bin/xonsh" --timings -i --rc "$work/rc.xsh" \
    | tr -d '\r' | sed 's/\x1b\[[0-9;?]*[a-zA-Z]//g' | grep -E 'on_post_rc|on_pre_prompt '
done

# Bash's own breakdown for comparison
hyperfine --warmup 3 --runs 15 --shell=none 'bash --norc --noprofile -i -c exit' 'bash -i -c exit'
```

The pseudoterminal marker harness that produced the interleaved time-to-first-prompt table is not
reproduced here because it is 40 lines of Python; if the operator wants the comparison rerun properly,
that harness should be rebuilt from the method description above (append a marker to Bash's
`PROMPT_COMMAND` after sourcing the real `~/.bashrc`, wrap xonsh's `$PROMPT`, read the pseudoterminal
until the marker appears, interleave the configurations).

## Traces left on this machine

Nothing was installed into Homebrew, `~/.local/bin`, any chezmoi source file or any managed manifest. Two
directories were created by the measurement runs and left in place rather than deleted, because deletion
is gated on the operator:

- `~/.local/share/xonsh/` (a `history_json` directory with one file per measured session, and
  `xonsh_script_cache` with 13 compiled-code entries) and `~/.cache/xonsh/`. These were created by xonsh
  itself on its first runs, before the harness was changed to redirect its data directory into the
  scratchpad. They are inert without xonsh installed.
- The session scratchpad holds the throwaway virtual environment, the generated fixtures and the
  harnesses. It is disposable.

Removing the first pair is `trash ~/.local/share/xonsh ~/.cache/xonsh`, for the operator to run.

## Open questions for the operator

1. **Ratify the no-go?** If yes, this document files beside
   `docs/research/2026-07-09-sp4-nushell-evaluation.md` and the ledger records both, so neither question
   is re-litigated.
1. **Is the migration specification still owed?** The ledger's `done_means` says "a reviewed spec and a
   go/no-go verdict"; this document is the evaluation, not a migration plan. Say whether a no-go
   discharges the spec half or whether a follow-up writes the plan anyway.
1. **Does SP4 build its binding table as shell-agnostic data with a renderer?** That is the single change
   that would make any future shell evaluation cheap, and it is worth doing on its own merits.
1. **Should SP4 take the bash-completion@2 startup win?** Roughly 220 ms of about 400 ms, measured. The
   open risk is which completions carapace does not cover; that needs an inventory before anything is
   dropped, and lazy-loading is the conservative middle path.
1. **Do you want the comparison re-measured on an idle machine before ratifying?** The ratios held across
   two passes at different loads, so the verdict does not hang on it, but the absolute numbers in this
   document are inflated.
1. **Is there a workload reason to want Python in the shell that this evaluation missed?** The value side
   was judged from the repository's contents (Rust tooling, Bash scripts, Python only as formatters).
   That is the weakest evidence here, and it is the operator's call, not a measurement.
1. **If a migration ever proceeds, is `SHELL` staying as Bash agreed?** Upstream advises it, fzf's
   `$SHELL -c` previews require it, and it keeps every non-interactive door untouched.
