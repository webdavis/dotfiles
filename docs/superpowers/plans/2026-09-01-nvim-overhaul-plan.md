# Neovim Config Overhaul Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended)
> or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`)
> syntax for tracking.

**Goal:** Move the live Neovim config into chezmoi unchanged, then land every fix, drop, add, custom
plugin and the lazy-load pass as 62 small reviewed pull requests, ending at a verified clean editor.

**Architecture:** One task per pull request in the spec's section 11 order and lanes. The source is
`dot_config/nvim/`; pure Lua under `lua/custom_api/` has headless tests in `tests/` run by
`nvim --headless --clean -l tests/run.lua`; bash pieces are bats tested. Every PR ships a dump diff.

**Tech Stack:** Neovim 0.12.5, lazy.nvim, LuaJIT, bash 3.2, bats-core, chezmoi, treefmt (stylua,
luacheck), herdr 0.8.2, herdr-nvim, pns, gh-axi.

**Spec:** `docs/superpowers/specs/2026-09-01-nvim-overhaul-design-v4.md` (v4.3); every task names its
spec sections. Inventory: `~/.claude/pipeline/nvim-overhaul/inventory-2026-09-01.md`.

## Global Constraints

- Decision H: the import carries zero change; every fix is a later PR (spec 3.7).
- One PR per behavior, small (operator rule 2026-08-10); at most two lanes open at once.
- Every test passes within a second; bash tests are bats, Lua tests use the headless runner (6.3).
- We test the behavior of tools we wrote, nothing else: no plugin, chezmoi or launchd assertions.
- No em-dashes anywhere. Markdown wraps at 105 columns. Conventional Commits, no AI trailer.
- `trash`, never `rm`, for anything including agent scratch. No removal mechanisms in the repo.
- The operator runs every `chezmoi apply`. Agents never apply.
- No new `<C-…>` or `<M-…>` chords; new keymaps carry `desc` in `Tool: action` form (8.2).
- No nvim-dap. No conform or nvim-lint. Nothing touches the shell, herdr's config, pns or moshi.
  Nothing in PR 3 to PR 29b edits `lazy.lua`'s `defaults` or adds a trigger; only PR 30a to 30d move
  the startup number.
- Focus is never a signal (7.2): pane id, tab id and the pinned socket identify a Neovim, and the
  agent lookup is `herdr-nvim`'s own.

### Verified facts, 2026-09-01

- `xcode-select -p` is `/Applications/Xcode.app/Contents/Developer` (Xcode 26.6); `sourcekit-lsp` is
  `/usr/bin/sourcekit-lsp`; the four Homebrew Swift tools are on PATH. PR 3 installs nothing.
- `stylua`, `luacheck` and `go` are NOT installed on dresden (PR 1 and PR 27 install them).
- All seven spec pins re-read with `git ls-remote <repo> HEAD`: xcodebuild.nvim `633eb71`, neotest
  `27bf921`, nvim-mcp `0b5ace3`, claudecode.nvim `2390c6e`, vim-slime `305b4d8`, neotest-swift
  `7487799`, neotest-rust (`rouge8/neotest-rust`) `2c9941d`. Re-read at each PR and recorded.
- `private_dot_codex/private_config.toml.tmpl` is UNTRACKED in this worktree (another slice is landing
  it) and has no pull request yet; the registering PR (10a or 10b) waits for the PR that lands it on
  `main` and extends it (spec 2, 7.3), and PR 9's record writes that PR's number down the day it
  merges. Never a `modify_config.toml`.
- Headless `+qa` never reaches lazy.nvim's `VeryLazy` firing (spec 9.1); verified 2026-09-01 on the
  live config: which-key is unloaded after `nvim --headless +qa` and loaded after the same command
  with `-c 'doautocmd User VeryLazy'`. Every measured run fires it by hand.

### Plan decisions v4.3 did not overrule, one line each

- Branch harness: every measurement after PR 2 runs the branch through `XDG_CONFIG_HOME` (below).
- PR 2 runs a pre-merge preview of the 3.7 proof through the harness; the post-apply run is binding.
- `custom_api/herdr.lua` is a thin wrapper over `herdr-nvim`'s lookup; PR 11 creates it, PR 13 extends
  it (a spec edge since v4.3), PR 16 reuses it. The lookup itself is never rewritten.
- `task_events.detail` renders `35s` under a minute and `5m12s` from a minute (10.9 asserts `(35s)`).
- The auto-reload watch (5.4) and the preview host (bug #11) are `custom_api` modules with specs.
- neotest adapters (12.1 default): `webdavis/neotest-swift` and `rouge8/neotest-rust`.
- PR 10a ships `nvim-mcp` (install, registrations, the 7.5 rule, plus the resolver on that row) or is
  the crate design spec, by the row PR 9's record names; PR 10b ships and registers the crate.
- The acceptance record (task 63) is the last commit on PR 31's branch, so PR 31 carries it (spec 11).

### How every task ships

- **Pipeline.** Read `slice-pipeline.md` and `pipeline-model-allocation.md` in the memory directory at
  dispatch; the 2026-09-01 row is current (Sonnet implements, Fable by inheritance for steps 2, 4b and
  6v, sol at ultra for 4a, the orchestrator briefs, reads, adjudicates and merges). Checklist:
  `~/.claude/pipeline/slice-checklist.sh new nvim-<slug> F`; `findings-register.sh new`; merge
  through `pipeline-merge.sh`.
- **Strategy F on every task.** No deferrals: the operator's goal leaves no work for later, so every
  finding is fixed in-round and 6v closes the fix. No task runs Strategy A.
- **Brief.** Written from the task at dispatch to `~/.claude/pipeline/slices/brief-nvim-<slug>.md`,
  logged, then step 3 dispatched separately. Every brief carries: trash never rm, no push, no apply,
  gates in the foreground with output pasted, the mutation table with an unmutated control, the
  self-check ("does anything I added admit the state I was fixing, or assert something I did not
  measure?"), and "do not end your turn on a background wait".
- **Branch harness.** Every measurement runs against the BRANCH: with `S` a kept scratch directory,
  `mkdir -p "$S/xdg" && ln -s "<worktree>/dot_config/nvim" "$S/xdg/nvim"`, then
  `export XDG_CONFIG_HOME="$S/xdg"`. The data directory stays shared (`~/.local/share/nvim`); a branch
  that adds a plugin installs it there at first start, and the lock pins it either way. Every `nvim`
  run starts from the one fixed, empty benchmark directory (9.1),
  `BENCH=~/.local/state/nvim-overhaul/bench`, entered with `mkdir -p "$BENCH" && cd "$BENCH"`, never
  from `$S`, so cwd-relative health lines and root-sensitive plugins see the same nothing on every
  run.
- **Gates, named once.** G1 `just test-unit`; G2 `just lint-check`; G3
  `nvim --headless -c 'doautocmd User VeryLazy' +qa` under the harness from `$BENCH`, five runs, every
  stderr file empty; G4 the dump diff (3.7 check 5) matching the PR's stated intent; G5 the 9.1 gate:
  the warm median (runs 2 to 5 after `sudo purge`, no agent running, `User VeryLazy` fired by hand,
  every number labelled "synthetic") is `after <= before + 10` ms against the previously merged PR's
  number, and for PR 30d and PR 31 ALSO `after < baseline - 10` against the import-day baseline; G6
  `just ship` before the push. Every PR runs G1 to G6. `just test-rust` applies only where a Rust crate
  changes (PR 10b). The TUI run (`nvim --startuptime` in a herdr pane) is recorded beside G5 where a
  task says so and is never the gate.
- **Merge main, then re-gate.** Before each review round the branch merges `main` and re-runs G1 to G5;
  a verdict on an un-re-gated branch is not a verdict (spec 11). Shared files serialize: the later PR
  waits for the earlier one to merge, and that edge is in its Depends line.
- **Red first.** Test steps precede implementation: write the spec file, run it, see the named
  failure, implement, run green, commit. Lua invocation:
  `nvim --headless --clean -l dot_config/nvim/tests/run.lua <name>_spec`; the bats file spawns exactly
  that, one `@test` per spec file, and every PR that adds a spec file adds its `@test` line. Steps
  marked OPERATOR are the operator's alone.

### Lanes (spec 11)

| Lane                 | Order                                                       |
| -------------------- | ----------------------------------------------------------- |
| first, second        | PR 1, then PR 2, then PR 3 (strictly serial)                |
| custom_api and agent | PR 6, 7a, 7b, 7c, 7d, 7e, 7f, 8, 9, 10a, 10b, 12, 11, 13, 23, 16, 14, 15 |
| LSP and tools        | PR 5a, 5b, 17a, 27, 29a                                     |
| drops and git        | PR 17b, 17c, 17d, 17e, 18, 22b, 24, 25                      |
| standalone           | PR 4a, 4b, 4c, 4d, 19a, 19b, 20, 21, 22a, 26a, 26b, 26c, 28, 29b |
| last                 | PR 30a, 30b1, 30b2, 30c1 to 30c9, 30d, 31, then task 63     |

A lane is the suggested order; the Depends line is what holds a PR back, and it names every
shared-file predecessor (spec 11 lists the files per edge). `lazy-lock.json` is left out: one key per
edit, keep both sides, re-gate. The 30c rows depend on PR 1 to 29b (the lane rule, spec 11) plus the
shared-file predecessor their Depends line names.

### Task 1: PR 1, lint infrastructure (spec 3.8)

Lane: first. Depends on: none. Brief: `brief-nvim-lint-infra.md`. Closes 71 (lint half).

**Files:** Modify `treefmt.toml`, `Brewfile.dev`, `.github/workflows/lint.yml` (toolchain step),
`.chezmoidata/system_packages_autoinstall.yaml` (formulae, alphabetical; the weekly cleanup removes
undeclared formulae).

**Interfaces:** Two treefmt formatters, `stylua` (rewrites, `includes = ["dot_config/nvim/**/*.lua"]`)
and `luacheck` (check only, `options = ["--config", "dot_config/nvim/dot_luacheckrc"]`, same
includes). Both match nothing until PR 2.

- [ ] **Step 1:** `brew install stylua luacheck`; record `brew list --versions stylua luacheck`
  (expected 2.5.2 and 1.2.0).
- [ ] **Step 2:** Add both to `Brewfile.dev`, the YAML formulae, and the `brew install` line of
  `.github/workflows/lint.yml`, by hand. Add the two `[formatter.*]` tables to `treefmt.toml`.
- [ ] **Step 3, proof (no test: config only):** create `dot_config/nvim/x.lua` holding `local a = 1`
  (unused), run `treefmt --no-cache --formatters luacheck`, expect a non-zero exit naming the file;
  `trash dot_config/nvim/x.lua`; `just lint-check` green. Paste both in the PR body.
- [ ] **Step 4:** Gates G1, G2, G6 plus `just lint-actions` (the workflow changed). Commit:
  `build(lint): add stylua and luacheck formatters scoped to dot_config/nvim`.

### Task 2: PR 2, import unchanged (spec 3.1 to 3.8)

Lane: first. Depends on: PR 1. Brief: `brief-nvim-import.md`. Closes 45 (part), 46 (track), 51, 52,
65, 74. `B` is the backup directory, `S` one kept scratch directory, `BENCH` the fixed empty benchmark
directory (`~/.local/state/nvim-overhaul/bench`, 9.1), `DOTFILES` the worktree.

**Files:** Create `dot_config/nvim/**` (rsync of the standalone tree, `.luacheckrc` as
`dot_luacheckrc`, `.prettierignore` as `dot_prettierignore`, `lua/overseer/template/user/run_script.lua`
as `literal_run_script.lua` so chezmoi copies it instead of executing it, see step 7),
`dot_config/nvim/tests/dump_state.lua`. Modify `.chezmoiignore` (the three `.config/nvim/...` lines of
3.4), `.gitignore` (one rule scoped to `dot_config/nvim/`, see step 7), `treefmt.toml` (mdformat and
taplo `excludes` both gain `dot_config/nvim/**`; stylua per step 3).

**Interfaces, `tests/dump_state.lua <out.json>` (3.7 check 5):** runs WITHOUT `--clean`, invoked as
`nvim --headless -u <config>/init.lua -l tests/dump_state.lua <out.json>` from `cd "$BENCH"`
(the `-u` flag is REQUIRED: `-l` alone skips all source-state initialization per `:help startup` item 9,
so init.lua and every plugin never load and the dump silently captures nothing), writes to `argv[1]`.
Fires `doautocmd User VeryLazy` FIRST, inside the dump process itself, and asserts a known
VeryLazy-triggered plugin (`which-key.nvim`) shows loaded afterward, erroring otherwise: the dump runs
in its own process, so without this every VeryLazy-triggered plugin (which-key, noice, textobjects,
unimpaired, claudecode after PR 30c9) is silently absent from both sides of every diff. Global pass:
`nvim_get_keymap(mode)` for `n`, `v`, `x`, `s`, `o`, `i`, `c`, `t`. Buffer-local pass: `:edit
$DOTFILES/justfile`, then
`vim.wait(5000, function() return vim.fn.maparg("]g", "n", false, true).buffer == 1 end)` (`]g` is the
first map gitsigns `on_attach` sets; `vim.b.gitsigns_head` is NOT the signal, it is set before
`on_attach`); exit 1 on timeout; then `nvim_buf_get_keymap(0, mode)` for the same modes. `DOTFILES`
must be `export`ed by the caller before this runs, or the child nvim process cannot read it and the
edit target is empty. Octo's `<localleader>` groups are EXCLUDED (which-key metadata, PR 24 checks
them by hand); this buffer-local pass proves only the gitsigns generic-buffer surface, not
filetype-local maps (markdown, octo, etc.). Which-key pass: `dofile("<config>/lua/plugins/which-key.lua")`
returns the spec table; its `opts.spec` is a list of blocks that each nest their actual group rows ONE
LEVEL DEEPER as their own array part (a naive top-level read finds zero groups on both sides and
compares equal), so this walks every nesting depth and refuses to write a dump with zero groups
captured. Plugin state pass: for every `require("lazy").plugins()` entry, emit `name`, `lazy` (the
`.lazy` field) and `loaded` (whether `._.loaded` is non-nil), not just a name list, because a plugin
flipping eager/lazy leaves its name unchanged. Projection: each keymap row keeps only `mode`, `lhs`,
`buffer`, `desc`, `noremap`, `silent`, `expr`, `nowait`, `rhs`; a Lua `callback` is fingerprinted as
`<callback:source:line>` via `debug.getinfo` (this is a keymap-metadata dump, not a behavior proof: two
callbacks can share a fingerprint only if defined on the same line of the same file, which does not
happen in practice, and a callback's runtime behavior is never exercised); a classic Vimscript
`<SNR>NNN_name` reference has its number stripped to `<SNR>_name` (confirmed to vary between separate
nvim invocations of the identical, unchanged config); and the config-root prefix of any path is
replaced with `<config>` (a pre-merge preview runs against a scratch deployment at a different absolute
path than the live config, and the raw path would otherwise differ by root alone). Rows are written
with an explicit, hand-ordered key sequence, NOT `vim.json.encode` on a whole Lua table: a Lua table's
hash-part iteration order is not guaranteed stable across separate process runs (measured: the same
plugin row encoded with two different key orders on two runs of the identical config), which would make
every line compare unequal for a reason that has nothing to do with the config. Rows are sorted by
`mode` then `lhs` then `buffer`, one JSON object per line.

The phase block (3.7), run once per phase with `P` set as the comment says, `NVIM_CONFIG` resolved from
`XDG_CONFIG_HOME` (left unset for the live config, exported by the caller for a scratch one). The
startup runs fire `User VeryLazy` by hand (9.1: headless `+qa` never reaches lazy.nvim's own firing), so
the numbers are synthetic and labelled so; `--startuptime` APPENDS to an existing file rather than
overwriting it (`nvim --help`), so each `st-N.log` is truncated before its run or a re-run of this block
against the same `P` silently doubles the sample count. The `sed` replaces the volatile roots (the
per-instance runtime directory, `$TMPDIR`, the state directory, the config root, log size, timestamps)
and keeps every other path, then blanks markview's own `:checkhealth` section: markview demo-samples a
few random glyphs from its symbol tables on every run and reorders its parser list (confirmed by diffing
two runs of the identical, unchanged config), so it can never compare equal regardless of any config
change, and it carries no pass/fail signal nvim-treesitter's own section does not already carry:

```bash
P="$S/before"; mkdir -p "$P" "$BENCH"; cd "$BENCH"   # second phase: P="$S/after"
NVIM_CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}/nvim"
export DOTFILES
for i in 1 2 3 4 5; do
  : >"$P/st-$i.log"
  nvim --headless --startuptime "$P/st-$i.log" -c 'doautocmd User VeryLazy' +qa 2>"$P/err-$i.log"
done
nvim --headless "+checkhealth" "+w! $P/health.txt" +qa
RUN="${XDG_RUNTIME_DIR:-${TMPDIR}nvim.${USER}}"
sed -E "s#${RUN}/[^/ ]+#<run>#g; s#${TMPDIR}#<tmpdir>/#g; s#${HOME}/.local/state/nvim/#<state>/#g; \
  s#${NVIM_CONFIG}#<config>#g; s/Log size: [0-9]+ KB/Log size: <n> KB/g; \
  s/[0-9]{4}-[0-9]{2}-[0-9]{2}[^ ]*//g" "$P/health.txt" \
  | awk '
      /^markview:/ { print; print "<markview: symbol/parser samples omitted, nondeterministic between runs>"; skip=1; next }
      skip && /^==============================================================================$/ { skip=0 }
      skip { next }
      { print }
    ' >"$P/health.norm"
nvim --headless -u "$NVIM_CONFIG/init.lua" -l "$DOTFILES/dot_config/nvim/tests/dump_state.lua" \
  "$P/state.json" 2>"$P/dump.err"
```

Even with every normalization above, `checkhealth` output carries RESIDUAL, config-independent
non-determinism from third-party plugins this program does not own (a same-config control run, twice in
a row, was measured to differ): a system-info section reorders a couple of lines, an overseer health
line reports a real elapsed-ms timing, and a filetype-mismatch warning lists its set in hash-iteration
order. The health diff (3.7 check 4) is therefore ADVISORY, reviewed by eye against this known noise
profile, never a hard gate; a hard gate on it would fail every future PR, including a perfect one.

- [ ] **Step 1, backups (3.2):** `TS=$(date +%Y-%m-%dT%H-%M-%S)`; `cp -R ~/.config/nvim
  ~/workspaces/backups/$TS.neovim-config.backup`; `git -C "$DOTFILES" bundle create
  ~/workspaces/backups/$TS.dotfiles.backup.bundle --all`; a `$TS.dotfiles-worktree.backup/` copy only
  if `git status` is dirty. The four 3.2 verification lines (`diff -r` of the copy, the stash count,
  the porcelain count, `git bundle verify`) print empty, `1`, `4`, "is okay"; pasted. (`cp -R` cannot
  copy a live `.git/fsmonitor--daemon.ipc` socket; that one line in the diff is expected and not a gap.)
- [ ] **Step 2, drain (3.1, 3.6), in `~/.config/nvim`:** FOUR commits in this order: three drain
  commits, `fix(autocmds): close the aerial sidebar with the others on quit`
  (`lua/config/autocmds.lua`); `feat(herdr-nvim): annotate lines back to herdr agents`
  (`lua/plugins/herdr-nvim.lua` plus the `herdr-nvim` line of `lazy-lock.json`, one unit); `docs: add
  the CLAUDE.md conventions file`; then `git stash drop stash@{0}`; then the fourth commit, the README
  commit prepending "moved to `webdavis/dotfiles` under `dot_config/nvim/`" (an incomplete drain leaves
  work stranded in a repository about to be archived); then `git push origin main`; `git fetch origin`;
  `rev-list origin/main..HEAD` empty.
- [ ] **Step 3, stylua check, BEFORE the flatten:** `stylua --check "$B"` with Homebrew's 2.5.2. Exit
  0: no exclusion; else add `dot_config/nvim/**` to the stylua `excludes` too, said in the body.
- [ ] **Step 4:** write `tests/dump_state.lua` on the branch per the interface above (it must exist
  before the before-phase runs). It is the only new file.
- [ ] **Step 5, before phase, the binding baseline (9.1, 3.7):** `sudo purge`; the phase block with
  `P="$S/before"`. This program runs agents continuously, so the "no agent running" precondition 9.1
  states cannot hold here and the spec's own section 2 already records agent load moving the warm
  median by more than twice the 10ms gate tolerance; the timing number is therefore captured and
  labelled ADVISORY, never gated, with a quiescent re-run left to the operator. Run 1 is cold (recorded,
  also advisory); the median of runs 2 to 5 is written into the PR body labelled "synthetic (VeryLazy
  fired by hand), advisory: agents may be running". A TUI `nvim --startuptime` in a herdr pane is
  recorded beside it, also not gated. Also, now, the byte diff WITHOUT `--exclude=README.md`: its only
  output names `README.md`, pasted (3.7 check 1).
- [ ] **Step 6, OPERATOR gate, archive (3.6 step 4):** after the operator confirms in the PR thread,
  the agent renames `webdavis/neovim-config` to `webdavis/neovim-config-archive` and archives it
  through `gh-axi`, pasting the `archived: true` line.
- [ ] **Step 7, flatten (3.3):** `rsync -a --exclude=.git --exclude=.claude --exclude=.DS_Store
  --exclude=.github --exclude=.gitignore ~/.config/nvim/ "$DOTFILES/dot_config/nvim/"`; rename
  `.luacheckrc` and `.prettierignore` to their `dot_` names; rename
  `lua/overseer/template/user/run_script.lua` to `literal_run_script.lua` (chezmoi reads a bare `run_`
  prefix as an executable script rather than a file to copy, per `chezmoi`'s own source-state-attributes
  reference; `literal_` is the documented escape and stops attribute parsing for that path component);
  sweep the WHOLE flattened tree for every other chezmoi keyword prefix
  (`run_ exact_ modify_ symlink_ private_ executable_ create_ remove_ encrypted_ external_ empty_
  readonly_ once_ onchange_ before_ after_`) and the `.tmpl` suffix, listing what was checked (today:
  `run_script.lua` is the only hit, no `.tmpl` files exist); assert `! test -e dot_config/nvim/.git`
  and `git status --porcelain | grep -c '/\.git/'` prints 0. Add the ignores (3.4), the ONE new
  `.gitignore` rule (3.3: the live repo's own `.gitignore` carried `.DS_Store`, `private/` and `*.sw*`;
  the first and third are already global rules in the dotfiles `.gitignore`, so only
  `dot_config/nvim/private/` is new), the mdformat AND taplo exclusions. Verify the deployed name is
  unchanged and the rename did not leak by rendering to a SCRATCH deployment (never the live one, which
  could already hold an unmanaged file at the target and hide the bug) and diffing the backup against
  it: only the dropped repo-metadata files (`.github`, `.gitignore`, `docs`) may appear.
- [ ] **Step 8, pre-merge preview:** render the branch to a scratch home,
  `HOME="$S/home" chezmoi --source "$DOTFILES" --destination "$S/home" apply "$S/home/.config/nvim"`
  (create `$S/home/.config` first; chezmoi's own `mkdir` for the leaf is not recursive), point the
  harness at `$S/home/.config`, run the phase block with `P="$S/preview"`, and diff every `$S/before`
  artifact against `$S/preview`: the byte, lock and state diffs empty, the health diff advisory (see
  above), the err logs empty, the timing advisory. Gates G1 to G6.
- [ ] **Step 9:** Commits: `chore(nvim): import the live Neovim config unchanged`, `chore(chezmoi):
  ignore the nvim repo metadata and tests`, `chore(lint): keep mdformat and taplo off dot_config/nvim
  until the hygiene PR`, `test(nvim): add the keymap and plugin state dump`. Merge.
- [ ] **Step 10, OPERATOR:** full `chezmoi apply`.
- [ ] **Step 11, after phase (3.7):** the phase block with `P="$S/after"` against the live config, then
  the comparison below (run under `set -euo pipefail`; the byte, lock, state and err-log checks are
  hard gates, the health diff is advisory per above, and the timing line is printed but not gated,
  posted as a PR comment):

  ```bash
  set -euo pipefail
  for required in "$S"/{before,after}/{health.norm,state.json}; do
    [[ -f "$required" ]] || { echo "FATAL: comparison input is missing: $required" >&2; exit 1; }
  done
  diff -r --exclude=.git --exclude=.claude --exclude=.DS_Store --exclude=README.md "$B" ~/.config/nvim
  diff "$B/lazy-lock.json" ~/.config/nvim/lazy-lock.json
  diff "$S/before/health.norm" "$S/after/health.norm" || true   # advisory, see step 5
  RACING='"lhs":"\[s"|"lhs":"\]s"|"lhs":"as"'
  SORT_NVIM_WINS='{"kind":"keymap","mode":"n","lhs":"[s","buffer":0,"desc":"Previous delimiter","noremap":1,"silent":1,"expr":1,"nowait":0,"rhs":"<callback:@/Users/stephen/.local/share/nvim/lazy/sort.nvim/lua/sort/motions.lua:80>"}
  {"kind":"keymap","mode":"n","lhs":"]s","buffer":0,"desc":"Next delimiter","noremap":1,"silent":1,"expr":1,"nowait":0,"rhs":"<callback:@/Users/stephen/.local/share/nvim/lazy/sort.nvim/lua/sort/motions.lua:68>"}
  {"kind":"keymap","mode":"o","lhs":"[s","buffer":0,"desc":"Previous delimiter","noremap":1,"silent":1,"expr":1,"nowait":0,"rhs":"<callback:@/Users/stephen/.local/share/nvim/lazy/sort.nvim/lua/sort/motions.lua:80>"}
  {"kind":"keymap","mode":"o","lhs":"]s","buffer":0,"desc":"Next delimiter","noremap":1,"silent":1,"expr":1,"nowait":0,"rhs":"<callback:@/Users/stephen/.local/share/nvim/lazy/sort.nvim/lua/sort/motions.lua:68>"}
  {"kind":"keymap","mode":"o","lhs":"as","buffer":0,"desc":"Around sortable region","noremap":1,"silent":1,"expr":0,"nowait":0,"rhs":"<callback:@/Users/stephen/.local/share/nvim/lazy/sort.nvim/lua/sort/textobjects.lua:104>"}
  {"kind":"keymap","mode":"x","lhs":"[s","buffer":0,"desc":"Previous delimiter","noremap":1,"silent":1,"expr":1,"nowait":0,"rhs":"<callback:@/Users/stephen/.local/share/nvim/lazy/sort.nvim/lua/sort/motions.lua:80>"}
  {"kind":"keymap","mode":"x","lhs":"[s","buffer":0,"desc":"Previous delimiter","noremap":1,"silent":1,"expr":1,"nowait":0,"rhs":"<callback:@/Users/stephen/.local/share/nvim/lazy/sort.nvim/lua/sort/motions.lua:80>"}
  {"kind":"keymap","mode":"x","lhs":"]s","buffer":0,"desc":"Next delimiter","noremap":1,"silent":1,"expr":1,"nowait":0,"rhs":"<callback:@/Users/stephen/.local/share/nvim/lazy/sort.nvim/lua/sort/motions.lua:68>"}
  {"kind":"keymap","mode":"x","lhs":"]s","buffer":0,"desc":"Next delimiter","noremap":1,"silent":1,"expr":1,"nowait":0,"rhs":"<callback:@/Users/stephen/.local/share/nvim/lazy/sort.nvim/lua/sort/motions.lua:68>"}
  {"kind":"keymap","mode":"x","lhs":"as","buffer":0,"desc":"Around sortable region","noremap":1,"silent":1,"expr":0,"nowait":0,"rhs":"<callback:@/Users/stephen/.local/share/nvim/lazy/sort.nvim/lua/sort/textobjects.lua:104>"}
  {"kind":"keymap","mode":"x","lhs":"as","buffer":0,"desc":"Around sortable region","noremap":1,"silent":1,"expr":0,"nowait":0,"rhs":"<callback:@/Users/stephen/.local/share/nvim/lazy/sort.nvim/lua/sort/textobjects.lua:104>"}'
  TEXTOBJECTS_WINS='{"kind":"keymap","mode":"n","lhs":"[s","buffer":0,"desc":"previous local scope","noremap":1,"silent":1,"expr":0,"nowait":0,"rhs":"<callback:@<config>/lua/custom_api/util.lua:141:fn:@<config>/lua/plugins/treesitter.lua:123>"}
  {"kind":"keymap","mode":"n","lhs":"]s","buffer":0,"desc":"next local scope","noremap":1,"silent":1,"expr":0,"nowait":0,"rhs":"<callback:@<config>/lua/custom_api/util.lua:141:fn:@<config>/lua/plugins/treesitter.lua:122>"}
  {"kind":"keymap","mode":"o","lhs":"[s","buffer":0,"desc":"previous local scope","noremap":1,"silent":1,"expr":0,"nowait":0,"rhs":"<callback:@<config>/lua/custom_api/util.lua:141:fn:@<config>/lua/plugins/treesitter.lua:123>"}
  {"kind":"keymap","mode":"o","lhs":"]s","buffer":0,"desc":"next local scope","noremap":1,"silent":1,"expr":0,"nowait":0,"rhs":"<callback:@<config>/lua/custom_api/util.lua:141:fn:@<config>/lua/plugins/treesitter.lua:122>"}
  {"kind":"keymap","mode":"o","lhs":"as","buffer":0,"desc":"local scope","noremap":1,"silent":1,"expr":0,"nowait":0,"rhs":"<callback:@<config>/lua/custom_api/util.lua:141:fn:@<config>/lua/plugins/treesitter.lua:97>"}
  {"kind":"keymap","mode":"x","lhs":"[s","buffer":0,"desc":"previous local scope","noremap":1,"silent":1,"expr":0,"nowait":0,"rhs":"<callback:@<config>/lua/custom_api/util.lua:141:fn:@<config>/lua/plugins/treesitter.lua:123>"}
  {"kind":"keymap","mode":"x","lhs":"[s","buffer":0,"desc":"previous local scope","noremap":1,"silent":1,"expr":0,"nowait":0,"rhs":"<callback:@<config>/lua/custom_api/util.lua:141:fn:@<config>/lua/plugins/treesitter.lua:123>"}
  {"kind":"keymap","mode":"x","lhs":"]s","buffer":0,"desc":"next local scope","noremap":1,"silent":1,"expr":0,"nowait":0,"rhs":"<callback:@<config>/lua/custom_api/util.lua:141:fn:@<config>/lua/plugins/treesitter.lua:122>"}
  {"kind":"keymap","mode":"x","lhs":"]s","buffer":0,"desc":"next local scope","noremap":1,"silent":1,"expr":0,"nowait":0,"rhs":"<callback:@<config>/lua/custom_api/util.lua:141:fn:@<config>/lua/plugins/treesitter.lua:122>"}
  {"kind":"keymap","mode":"x","lhs":"as","buffer":0,"desc":"local scope","noremap":1,"silent":1,"expr":0,"nowait":0,"rhs":"<callback:@<config>/lua/custom_api/util.lua:141:fn:@<config>/lua/plugins/treesitter.lua:97>"}
  {"kind":"keymap","mode":"x","lhs":"as","buffer":0,"desc":"local scope","noremap":1,"silent":1,"expr":0,"nowait":0,"rhs":"<callback:@<config>/lua/custom_api/util.lua:141:fn:@<config>/lua/plugins/treesitter.lua:97>"}'
  check_racing_variant() {
    local label="$1" file="$2" subset
    subset="$(grep -E "$RACING" "$file" | sort)"
    if [[ "$subset" != "$SORT_NVIM_WINS" && "$subset" != "$TEXTOBJECTS_WINS" ]]; then
      printf 'FATAL: %s racing rows match neither known race variant:\n%s\n' "$label" "$subset" >&2
      exit 1
    fi
  }
  diff <(grep -Ev "$RACING" "$S/before/state.json") <(grep -Ev "$RACING" "$S/after/state.json")
  diff <(grep -E "$RACING" "$S/before/state.json" | sort) <(grep -E "$RACING" "$S/after/state.json" | sort) || true
  check_racing_variant before "$S/before/state.json"
  check_racing_variant after "$S/after/state.json"
  err_report="$(wc -c "$S"/*/err-*.log | awk '$1 != 0 && $2 != "total"')"
  [[ -z "$err_report" ]] || { echo "$err_report"; echo "FATAL: a startup run wrote to stderr" >&2; exit 1; }
  median() {
    local n
    n="$(grep -h "NVIM STARTED" "$1"/st-{2,3,4,5}.log | wc -l | tr -d ' ')" || true
    [[ "$n" == "4" ]] || { echo "FATAL: expected exactly 4 warm samples in $1, found $n" >&2; exit 1; }
    grep -h "NVIM STARTED" "$1"/st-{2,3,4,5}.log | sort -n | awk '{a[NR]=$1} END {print (a[2]+a[3])/2}'
  }
  before_median="$(median "$S/before")"; after_median="$(median "$S/after")"
  printf 'before %s after %s (synthetic, VeryLazy fired by hand), advisory: agents may be running\n' \
    "$before_median" "$after_median"
  ```

  A proof that cannot fail is not a proof: the median function used to return 0 from zero samples and
  half a real value from two, and neither a missing log nor a nonempty diff forced a failure. This
  version requires exactly four warm samples per phase and treats a nonempty stderr log as fatal. The
  `[s`, `]s` and `as` rows are checked against the two known outcomes rather than dropped from the gate:
  `sort.nvim` and `nvim-treesitter-textobjects` both map them and plugin load order picks the winner,
  so 2 of 12 dumps of the identical unchanged config named the other one (measured 2026-09-02, spec
  3.7); exempting the rows wholesale made a genuine new regression on any of the three invisible, so
  each side's racing subset must equal one of the two byte-for-byte, and anything else fails the same
  way every other row does. A diff of two MISSING files is empty and a grep over a missing file inside
  a process substitution reports nothing, so every comparison input is checked to exist before any of
  them is read. The dump's own stderr goes to `$P/dump.err`, printed by the comparison but not gated: 6
  of 10 dumps of the identical unchanged config wrote the same `aerial.nvim` treesitter stack trace
  from a scheduled callback (measured 2026-09-02). Uncaptured, a scheduled error could change
  what the capture sees while nothing recorded that it happened.
- [ ] **Step 12, OPERATOR:** when the three 3.6 guards (`status --porcelain`, `rev-list
  origin/main..HEAD`, `stash list`, all in `~/.config/nvim`) print nothing, `trash ~/.config/nvim/.git`.
  Task 3 waits for steps 10 to 12.

### Task 3: PR 3, Swift stack (spec 5.3, 8.3, 10.7)

Lane: second. Depends on: PR 2. Brief: `brief-nvim-swift.md`. Closes 43.

**Files:** Create `lua/plugins/xcodebuild.lua`. Modify `lua/plugins/lsp.lua` (sourcekit),
`lua/plugins/which-key.lua` (`{ "<leader>X", group = "xcode" }`), `lazy-lock.json`. No package edits.

- [ ] **Step 1:** `vim.lsp.config("sourcekit", { cmd = { "sourcekit-lsp" }, root_markers = {
  "buildServer.json", ".bsp", "*.xcodeproj", "*.xcworkspace", "compile_commands.json",
  "Package.swift", ".git" } })` and `vim.lsp.enable("sourcekit")` inside `if vim.fn.has("mac") == 1`.
- [ ] **Step 2:** the `xcodebuild.nvim` spec: `commit = "633eb71"`, deps `MunifTanjim/nui.nvim`,
  `folke/snacks.nvim`, `stevearc/oil.nvim`, `ft = "swift"`, `cmd`, `cond = vim.fn.has("mac") == 1`,
  `keys` for `<leader>Xb` build, `Xr` run, `Xt` test, `XT` test current, `Xs` scheme, `Xd` device,
  `Xl` logs, `Xp` project manager, each `desc = "Xcode: …"`. The group row is unconditional (8.4).
- [ ] **Step 3, live (10.7), pasted:** hover on a UIKit symbol in an Xcode project with
  `buildServer.json`; `:XcodebuildBuild` through `xcbeautify`; save runs swiftformat and swiftlint;
  the Vapor smoke check exactly as 10.7 states, `swift test` from a shell, exit 0.
- [ ] **Step 4:** Gates G1 to G6. G4 shows exactly the eight `<leader>X` maps and the group row.
  Commits: `feat(nvim): configure sourcekit-lsp on macOS`, `feat(nvim): add xcodebuild.nvim`.

### Task 4: PR 4a, the lazy checker off (spec 3.5)

Lane: standalone. Depends on: PR 2. Brief: `brief-nvim-checker-off.md`. Closes 46 (checker).

- [ ] **Step 1:** `lua/config/lazy.lua:48` `checker.enabled = false`. Commit: `chore(nvim): turn the
  lazy update checker off`.
- [ ] **Step 2:** Gates G1 to G6; G4 unchanged. No test: config only.

### Task 5: PR 4b, remove the LazyVim scaffolding (spec 3.5, 8.3)

Lane: standalone. Depends on: PR 3 (`which-key.lua`). Brief: `brief-nvim-lazyvim-scaffolding.md`.
Closes 47, 53.

**Files:** Delete `lazyvim.json`. Modify `lua/config/autocmds.lua` (`lazyvim_` to `nvim_config_`,
`lazyvim_last_loc` to `nvim_config_last_loc`), `lua/plugins/which-key.lua` (`<leader>L` group
"lazy"), the `<leader>L` descs ("LazyVim: …" to "Lazy: …").

- [ ] **Step 1:** two commits: `refactor(nvim): rename the lazyvim augroups and drop lazyvim.json`,
  `refactor(nvim): rename the <leader>L group to lazy`.
- [ ] **Step 2:** Gates G1 to G6; G4 shows one group rename and the desc changes, no `lhs` change.

### Task 6: PR 4c, the `<leader>A` herdr group row (spec 8.2 rule 2)

Lane: standalone. Depends on: PR 4b (`which-key.lua`). Brief: `brief-nvim-herdr-group.md`.

- [ ] **Step 1:** `{ "<leader>A", group = "herdr" }` in prefix order. Commit: `feat(nvim): name the
  <leader>A herdr group`.
- [ ] **Step 2:** Gates G1 to G6; G4 shows exactly one new group row. No test.

### Task 7: PR 4d, lift the formatter exclusions (spec 3.8, 12.7)

Lane: standalone. Depends on: PR 2. Brief: `brief-nvim-lift-exclusions.md`.

- [ ] **Step 1:** remove the mdformat (and, if PR 2 added it, stylua) exclusion from `treefmt.toml`;
  `just l`; commit the rewrap of `CLAUDE.md` and `docs/todo.md` alone: `style(nvim): apply mdformat
  (and stylua) to the imported config`, called out in the body.
- [ ] **Step 2:** Gates G1 to G6; G4 unchanged. No test.

### Task 8: PR 5a, LSP config, bug #12 (spec 4)

Lane: LSP and tools. Depends on: PR 3 (`lsp.lua`). Brief: `brief-nvim-lsp-config.md`. Closes 13.

**Files:** Modify `lua/plugins/lsp.lua:50-148` (per-server tables to `vim.lsp.config("<name>",
{…})`).

- [ ] **Step 1, red:** `nvim --headless -c 'lua assert(vim.tbl_contains(vim.lsp.config.clangd.cmd,
  "--clang-tidy"))' +qa` under the harness fails today (the `servers` block is never read).
- [ ] **Step 2:** move every server table into `vim.lsp.config`; mason-lspconfig keeps
  `ensure_installed` and `automatic_enable`; the same assertion for `--header-insertion=iwyu`. Green:
  both pass; a `.c` file's `:LspInfo` shows clangd with both flags. Paste.
- [ ] **Step 3:** Gates G1 to G6; G4 unchanged. Commit: `fix(nvim): move server settings to
  vim.lsp.config`.

### Task 9: PR 5b, mason-tool-installer off at start (spec 3.9 prep)

Lane: LSP and tools. Depends on: PR 5a (`lsp.lua`). Brief: `brief-nvim-mason-run-on-start.md`. Closes
50 (prep).

**Files:** Modify `lua/plugins/lsp.lua:212` (`run_on_start = false`).

- [ ] **Step 1:** `run_on_start = false` (the bootstrap's `MasonToolsInstallSync` needs it, 3.9, so
  the sync run and the autostart run cannot race). Commit: `chore(nvim): stop mason-tool-installer
  running at start`.
- [ ] **Step 2:** Gates G1 to G6; G4 unchanged; `:MasonToolsInstallSync` still installs by hand. No
  test: config only.

### Task 10: PR 6, custom_api errors and the test harness (spec 6.1, 6.3)

Lane: custom_api. Depends on: PR 2. Brief: `brief-nvim-api-errors.md`. Closes 19, 20, 54, 58, 59, 66
(harness).

**Files:** Create `dot_config/nvim/tests/run.lua`, `tests/util_spec.lua`, `tests/try_spec.lua`,
`tests/git_spec.lua` (latest_commit only, extended in PR 7a to 7c), `lua/custom_api/try.lua`,
`test/unit/nvim-custom-api.bats`. Modify `lua/custom_api/{init,git,github,util}.lua`, `justfile`
(`test-nvim`). Delete `lua/custom_api/helpers.lua` once `pack` has no caller.

**Interfaces:** `run.lua [--config <dir>] [<name>_spec]`, exit 1 on any failure, prepends
`<config>/lua/?.lua;<config>/lua/?/init.lua` to `package.path`. `custom_api.try(fn, { label = "x" })`
runs `xpcall(fn, debug.traceback)` and notifies `[x] <message>` plus traceback. `git.latest_commit()`
returns `({ hash, summary, body }, err)`. Operational failures are `nil, message`; bugs `error()`.

- [ ] **Step 1, red:** write `run.lua` and `util_spec.lua` (`trim`, `sanitize_input`, `normalize`);
  the util cases pass, proving the runner; add `try_spec.lua` asserting the label "git.default_branch"
  appears verbatim in the notified text and a traceback line is present: FAIL, `try` not found.
- [ ] **Step 2:** implement `try.lua`; `latest_commit` to `(table, err)`; fix the two error texts;
  delete `helpers.wrap` and every `debug.getinfo` reflection; nothing runs on `require`.
- [ ] **Step 3, green:** all three specs pass; `bats test/unit/nvim-custom-api.bats` green, each
  `@test` under 200 ms (`--clean`, about 30 ms).
- [ ] **Mutants (control first):** return "anonymous" instead of the label (try_spec red); drop the
  traceback (red); make `latest_commit` return a string again (git_spec red).
- [ ] **Step 4:** Gates G1 to G6; G4 unchanged. Commits: `test(nvim): add the headless Lua runner and
  bats wiring`, `refactor(nvim): route custom_api errors through try and (value, err)`,
  `refactor(nvim): delete helpers.wrap`.

### Task 11: PR 7a, the pure-helper tests and bug #8 (spec 6.3, bug #8)

Lane: custom_api. Depends on: PR 6 (`custom_api/git.lua`, `tests/git_spec.lua`). Brief:
`brief-nvim-pure-helper-tests.md`. Closes 9, 67.

**Files:** Modify `lua/custom_api/git.lua:101-112` (`extract_upstream`), `tests/git_spec.lua`.

**Interfaces:** `git.extract_upstream` returns `i`, so the first commit word survives; the pure
helpers `convert_remote_protocol`, `normalize_branch`, `is_current_branch`, `parse_branch_line` keep
their signatures.

- [ ] **Step 1, red:** `git_spec` cases for `convert_remote_protocol`, `normalize_branch`,
  `is_current_branch`, `parse_branch_line`, and `extract_upstream` (the first commit word survives).
  FAIL on `extract_upstream` (returns `i + 1`).
- [ ] **Step 2:** return `i`. Green. **Mutants:** restore `i + 1` (red).
- [ ] **Step 3:** Gates G1 to G6; G4 unchanged; live: a branch with an upstream lists its full commit
  message (10.6), pasted. Commit: `fix(nvim): keep the first commit word in extract_upstream`.

### Task 12: PR 7b, inject the shell runner (spec 6.2)

Lane: custom_api. Depends on: PR 7a (`custom_api/git.lua`, `tests/git_spec.lua`). Brief:
`brief-nvim-inject-runner.md`. Closes 55.

**Files:** Create `tests/github_spec.lua`. Modify `lua/custom_api/git.lua`, `lua/custom_api/github.lua`,
`tests/git_spec.lua`.

**Interfaces:** `git.runner` and `github.runner` default to `util.run_shell_command`; a fake returns
`(exit_code, output)` by command string and every shell call in both modules goes through it.

- [ ] **Step 1, red:** `git_spec`: `latest_commit` with a fake runner; `github_spec`: `repo()` with a
  fake `gh repo view` reply. FAIL, no `runner` field.
- [ ] **Step 2:** inject the runner in both modules. Green. **Mutants:** one call shells out past the
  runner (red).
- [ ] **Step 3:** Gates G1 to G6; G4 unchanged. Commit: `refactor(nvim): inject the shell runner into
  git and github`.

### Task 13: PR 7c, the GitHub default-branch fallback, bugs #4 and #4b (spec 6.2)

Lane: custom_api. Depends on: PR 7b (`custom_api/git.lua`, `custom_api/github.lua`, both specs). Brief:
`brief-nvim-default-branch.md`. Closes 4, 5, 56.

**Files:** Modify `lua/custom_api/git.lua:231-247`, `lua/custom_api/github.lua`, `tests/git_spec.lua`,
`tests/github_spec.lua`, `lua/plugins/git.lua:997` (the caller).

**Interfaces:** `git.default_branch()` takes no repo, checks `refs/remotes/origin/main` then `master`
and returns `(name, err)`, `nil, "no default branch"` otherwise; `github.default_branch({ owner,
name })` returns `(name, err)` and supplies both `%s`. The caller calls `git.default_branch()` and falls
through to `github.default_branch` with `github.repo()`'s owner and name.

- [ ] **Step 1, red:** `git_spec`: `default_branch` with a fake runner answering
  `refs/remotes/origin/main`, then `master`, then neither; `github_spec`: the fallback with a fake
  `gh api` reply. FAIL on the string-versus-table read and the `%s` arity.
- [ ] **Step 2:** move the fallback; rewire `git.lua:997`. Green; live: a no-upstream branch returns
  the GitHub default (10.6), pasted.
- [ ] **Mutants:** `default_branch` ignores the runner and shells out (red); the `%s` arity bug back
  (github_spec red).
- [ ] **Step 3:** Gates G1 to G6; G4 unchanged. Commit: `fix(nvim): move the GitHub default-branch
  fallback into github`.

### Task 14: PR 7d, bug #1, `github.account().username` (spec 4, 6.2)

Lane: custom_api. Depends on: PR 7c (`custom_api/github.lua`, `tests/github_spec.lua`, `git.lua`).
Brief: `brief-nvim-github-account.md`. Closes 1.

**Files:** Modify `lua/custom_api/github.lua`, `tests/github_spec.lua`, `lua/plugins/git.lua:267`.

**Interfaces:** `github.account()` returns `({ username }, err)`; `github.username()` is deleted; the
`<C-g>i` keymap reads `account().username` through `custom_api.try`.

- [ ] **Step 1, red:** `github_spec`: `account().username` with a fake `gh api user` reply; an
  operational failure returns `nil, message`. FAIL, `account` undefined.
- [ ] **Step 2:** implement; `git.lua:267` reads it. Green; live: `<C-g>i` reaches the prompt (10.6),
  pasted. **Mutants:** return the raw string (red); swallow the error (red).
- [ ] **Step 3:** Gates G1 to G6; G4 unchanged (`<C-g>i` keeps its `lhs`). Commit: `fix(nvim): read
  the GitHub username from account()`.

### Task 15: PR 7e, split `map` and `overseer_runner` out of util (spec 6.2)

Lane: custom_api. Depends on: PR 7d (`custom_api/util.lua`, `custom_api/init.lua` after PR 6). Brief:
`brief-nvim-split-util.md`. Closes 57.

**Files:** Create `lua/custom_api/keymap.lua` (`map`), `lua/custom_api/overseer.lua`
(`overseer_runner`). Modify `lua/custom_api/util.lua` (keeps the string helpers and
`run_shell_command`), `lua/custom_api/init.lua`, every caller of `map` and `overseer_runner`.

**Interfaces:** `custom_api.keymap.map({ mode, lhs, rhs, desc })` passes a string or function `rhs` to
`vim.keymap.set` directly (the redundant closure at `util.lua:141-147` is gone).

- [ ] **Step 1, red:** `util_spec` asserts `util.map` and `util.overseer_runner` are nil and
  `require("custom_api.keymap").map` exists. FAIL.
- [ ] **Step 2:** move both; drop the closure; update the callers. Green. **Mutants:** keep the
  closure (a function `rhs` still works, so the dump is the check: G4 unchanged).
- [ ] **Step 3:** Gates G1 to G6; G4 unchanged (every `lhs`, `rhs` and `desc` identical). Commit:
  `refactor(nvim): split map and overseer_runner out of util`.

### Task 16: PR 7f, rename `copy_URL_to_clipboard` (spec 6.2, decision E)

Lane: custom_api. Depends on: PR 7e (`custom_api/util.lua`, `git.lua`). Brief:
`brief-nvim-rename-copy-url.md`. Closes 60.

**Files:** Modify `lua/custom_api/util.lua`, `lua/plugins/git.lua:33` (the one caller).

- [ ] **Step 1, red:** `util_spec` asserts `util.copy_url_to_clipboard` exists and the old name is
  nil. FAIL.
- [ ] **Step 2:** rename; the caller. Green. Live: the copy-URL keymap still copies.
- [ ] **Step 3:** Gates G1 to G6; G4 unchanged. Commit: `refactor(nvim): rename copy_URL_to_clipboard`.

### Task 17: PR 8, delete delegate.lua (spec 7.1, 8.3)

Lane: custom_api. Depends on: PR 7f, PR 4c (`which-key.lua`). Brief: `brief-nvim-delete-delegate.md`.
Closes 6, 10, 61.

**Files:** Delete `lua/custom_api/delegate.lua`. Modify `lua/custom_api/init.lua` (drop
`M.delegate`), `lua/config/keymaps.lua:6` (drop the `setup()`), `lua/plugins/which-key.lua`
(`<leader>d` group "do").

- [ ] **Step 1:** the three edits plus the delete in one commit: `refactor(nvim): delete delegate.lua
  and rename the <leader>d group to do`.
- [ ] **Step 2:** Gates G1 to G6. G4 shows exactly `<leader>dt`, `<leader>dp`, `<leader>ds` removed
  and the group renamed; `<leader>dx` and `<leader>da` stay. No test.

### Task 18: PR 9, MCP server evaluation only (spec 7.3)

Lane: agent. Depends on: PR 2. Brief: `brief-nvim-mcp-eval.md`. Closes 62 (evaluation). Budget: one
working day, extended once and only by the first row of the 7.3 table. Evaluation ONLY: nothing is
installed through chezmoi, nothing is registered, no CLAUDE.md is edited, so a failed candidate never
reaches `~/.claude.json` or the Codex template.

**Files:** Create `docs/research/2026-09-nvim-mcp-evaluation.md`. Nothing else.

- [ ] **Step 1:** `cargo install --git https://github.com/linw1995/nvim-mcp --rev 0b5ace3` by hand,
  outside chezmoi; the six criteria in table order (5 and 6, then 4, then 1 to 3) against the live
  setup (two workspaces, two Neovim panes in one), each pass, fail or undecided with its command and
  output, and the one row taken. Criterion 5's hand check is the 10.8 loop run once by hand; the
  recorded 10.8 check belongs to the registering PR. The record also names the PR that lands
  `private_dot_codex/private_config.toml.tmpl` on `main` once it has a number (7.3). Commit:
  `docs(nvim): record the nvim-mcp evaluation`. A "5 or 6 undecided" day ends with only this commit
  and the one-day extension.
- [ ] **Step 2:** the body states the row and therefore which of PR 10a and PR 10b ship and which are
  skipped. Gates G1, G2, G6 (a doc: G3 to G5 unchanged by construction, stated). No `chezmoi apply`.

### Task 19: PR 10a, ship by PR 9's row (spec 7.3, 7.5, 10.8)

Lane: agent. Depends on: PR 9, PR 4d (`dot_config/nvim/CLAUDE.md`), and the PR that lands
`private_dot_codex/private_config.toml.tmpl` on `main` (its number from PR 9's record; PR 10a does not
open before it). Brief: `brief-nvim-mcp-ship.md`. Closes 62 (ship), 63, 73 (MCP half) and custom #4
(resolver) on the `nvim-mcp` rows; on a crate row this PR is the design spec only and those close in
PR 10b. Two shapes, one taken:

**`nvim-mcp` rows (5, 6, 4 pass).** Create
`.chezmoiscripts/run_onchange_after_73-install-nvim-mcp.sh.tmpl` (`cargo install --git
https://github.com/linw1995/nvim-mcp --rev 0b5ace3`, guarded on `command -v cargo`, quiet on no-op),
`lua/plugins/nvim-mcp.lua`. Modify `modify_private_dot_claude.json` (a stable `nvim-mcp` entry beside
composio), `private_dot_codex/private_config.toml.tmpl` (one `[mcp_servers.nvim]` table beside the
ten), `.chezmoitemplates/global-agent-rules.md` and `dot_config/nvim/CLAUDE.md` (the 7.5 rule
verbatim), the lock. No project `.mcp.json`. On the as-is row (1, 2, 3 all pass) both registrations
run `nvim-mcp` directly. On the resolver row (any of 1 to 3 fails or undecided) also create
`dot_local/libexec/executable_nvim-mcp-connect.sh` (at most 80 lines, bash 3.2, `set -euo pipefail`)
and `test/unit/nvim-mcp-connect.bats`, and both registrations run the resolver. Resolver order: if
`NVIM_MCP_SOCKET` is set, `exec nvim-mcp --connect "$NVIM_MCP_SOCKET"`; else list
`${XDG_RUNTIME_DIR:-${TMPDIR}nvim.${USER}}/*/nvim.*.0` (the root `:help serverstart()` documents;
`$TMPDIR` alone misses Linux) and ask each with `nvim --server "$sock" --remote-expr` for the joined
`HERDR_WORKSPACE_ID`, `HERDR_TAB_ID`, `HERDR_PANE_ID` (the 7.3 expression) under a short deadline,
skipping refusals and timeouts. `select_socket <workspace> <tab> <lines "socket workspace tab pane">`
keeps the caller's workspace, prints the one candidate in the caller's tab, else the lone candidate in
the workspace, else exits 1 with the candidate pane ids on stderr and the instruction (launch from
`<leader>Cc` or export `NVIM_MCP_SOCKET`). Never focus.

- [ ] **Step 1, red (resolver row only):** bats sourcing the function: the tab match wins over a
  second candidate; a lone candidate in the workspace wins; two unpinned candidates exit 1 naming both
  pane ids; another workspace's candidate is ignored; empty ids are skipped; `NVIM_MCP_SOCKET` set
  short-circuits main; the listing glob resolves under `XDG_RUNTIME_DIR` when set and under
  `${TMPDIR}nvim.${USER}` otherwise. FAIL, function undefined. **Step 2:** implement; `shellcheck`
  clean. **Mutants:** drop the workspace filter (case 4 red); drop the tab match (case 1 red); print
  the first socket when two remain (case 3 red); list `$TMPDIR/nvim.$USER` only (the
  `XDG_RUNTIME_DIR` case red).
- [ ] **Step 3:** the install script; the plugin spec; both registrations; the 7.5 rule in both
  CLAUDE.md files and the shared partial. Commits: `feat(chezmoi): install nvim-mcp at apply time`,
  `feat(nvim): resolve the pane's Neovim socket for nvim-mcp` (resolver row), `feat(agents): register
  the Neovim MCP server and the open-buffer rule`.
- [ ] **Step 4, live (10.8):** an unsaved edit is read and written back through the MCP tool from a
  Claude session in the herdr pane, and once from Codex. Pasted, with the row taken and the statement
  that PR 10b is skipped. Gates G1 to G6 (G4 shows only what the plugin spec adds).
- [ ] **Step 5, OPERATOR:** `chezmoi apply` after merge lands the registrations.

**Crate rows (5 or 6 fails; or 4 fails or undecided).** The design spec only, no code, no
registration: `docs/superpowers/specs/2026-09-nvim-workspace-mcp-design.md` (name confirmed first;
rename rule): five tools (`current_buffer`, `list_buffers`, `read_buffer`, `edit_buffer`,
`diagnostics`), the same pane-id resolution over the same runtime root and socket pin, about 600
lines of Rust as the cap, a `run_onchange_after_59`-style build script. Reviewed as a doc; commit
`docs(nvim): design the nvim-workspace-mcp crate`. The CLAUDE.md and Codex-template edges are unused
on this shape and PR 10b carries them.

### Task 20: PR 10b, the custom crate build and its registration (spec 7.3, 7.5, 10.8)

Lane: agent. Depends on: PR 10a, PR 4d (`dot_config/nvim/CLAUDE.md`), the Codex-template PR. Brief:
`brief-nvim-mcp-crate.md`. Closes custom #4 (build), 63, 73 (MCP half) on the crate rows. Built only
on a crate row of PR 9's table, inside this program; skipped with a stated reason otherwise.

**Files:** Create the crate at `dot_local/share/nvim-workspace-mcp/`, its build script. Modify
`modify_private_dot_claude.json`, `private_dot_codex/private_config.toml.tmpl`,
`.chezmoitemplates/global-agent-rules.md`, `dot_config/nvim/CLAUDE.md` (the 7.5 rule verbatim).

- [ ] **Step 1, red:** cargo tests for the resolution (tab match, lone, ambiguous refusal, pin wins,
  the `XDG_RUNTIME_DIR` root). FAIL. **Step 2:** the crate per the 10a spec, the build script, both
  registrations pointed at it, the 7.5 rule. **Mutants:** ignore the tab (red); guess on ambiguity.
- [ ] **Step 3:** Gates G1 to G6 plus `just test-rust`; live 10.8 from Claude and once from Codex
  through the crate, pasted. Commits: `feat(nvim): build the nvim-workspace-mcp server`,
  `feat(agents): register the Neovim MCP server and the open-buffer rule`.
- [ ] **Step 4, OPERATOR:** `chezmoi apply`.

### Task 21: PR 12, claudecode.nvim with provider none (spec 5.3, 7.2, 8.3)

Lane: agent. Depends on: PR 8 (`which-key.lua`), PR 29a (`lsp.lua`: this PR is in the `lsp.lua` chain
after 29a and before 30a, spec 11). Brief: `brief-nvim-claudecode.md`. Closes 31, 42, 73 (send half),
77, 78.

**Files:** Create `lua/plugins/claudecode.lua` (`commit = "2390c6e"`, `opts = { terminal = { provider
= "none" } }`, `dependencies = { "folke/snacks.nvim" }`, the eight item-78 dispositions as the header
comment, keys `<leader>Cs` send (visual), `Ca` add file, `Cy` accept diff, `Cn` deny diff),
`lua/custom_api/autosave.lua`, `tests/autosave_spec.lua`. Modify `lua/plugins/which-key.lua`
(`{ "<leader>C", group = "claude" }`), `lua/plugins/autosave.lua` (the condition, the
`AutoSaveWritePre`/`AutoSaveWritePost` flag), `lua/plugins/lsp.lua` (lsp-format's `BufWritePre`
returns early on `vim.b.autosave_write`), the lock.

**Interfaces:** `autosave.should_save(name, buftype)` (pure): false when `name` matches `(proposed)`
or `(NEW FILE - proposed)` or `buftype == "acwrite"`, else true.

- [ ] **Step 1, red:** `autosave_spec`: the three exclusions false, an ordinary file true. FAIL.
  **Step 2:** implement; wire the condition; the write-flag pair; the plugin spec; the group row.
- [ ] **Step 3, live (10.8 send half):** `:ClaudeCodeSend` on a selection lands as an at-mention in
  the pane's session; a `(proposed)` buffer is not auto-saved; `:w` still formats. Pasted.
- [ ] **Mutants:** drop the `acwrite` branch (red); drop the `(proposed)` match (red).
- [ ] **Step 4:** Gates G1 to G6; G4 shows the four `<leader>C` maps and the group row. Commits:
  `feat(nvim): add claudecode.nvim with provider none`, `feat(nvim): keep auto-save off proposed
  buffers`.

### Task 22: PR 11, vim-slime with the herdr target (spec 7.4)

Lane: agent. Depends on: PR 12 (`claudecode.lua`, the group). Brief: `brief-nvim-slime.md`. Closes 39.

**Files:** Create `dot_config/nvim/autoload/slime/targets/herdr.vim`, `lua/custom_api/herdr.lua`,
`lua/plugins/slime.lua` (`commit = "305b4d8"`, `g:slime_no_mappings = 1`, `g:slime_target =
"herdr"`). Modify `lua/plugins/claudecode.lua` (keys `<leader>Cp` pipe, `<leader>CP` set target,
`desc = "Slime: …"`), `lazy-lock.json`.

**Interfaces:** `herdr.agent_pane()` wraps `herdr-nvim` (`lua/herdr-nvim/agents.lua`, module path
verified from the installed plugin at PR time): `agents.list()` filtered to `kind == "claude"`, then
`agents.resolve()`, else `ui.pick_agent`; returns the pane id or nil. No unit test: glue over a
third-party API, checked live. The target: `config` prompts with `agent_pane()` as the default, `send`
runs `herdr pane send-text` then `herdr pane send-keys <pane> <enter-name>`, `ValidEnv` checks
`$HERDR_ENV`.

- [ ] **Step 1, the recorded check, before binding anything (7.4), pasted in the body:**

  ```bash
  p=$(herdr pane split --current --direction down --cwd "$PWD" --no-focus | jq -r .result.pane.pane_id)
  herdr pane send-text "$p" $'printf "%s|" a b\nprintf "%s|" c d\n'   # two lines in one send
  herdr pane send-keys "$p" enter                                    # the spelling under test
  sleep 1; herdr pane read "$p" --lines 6                            # expect a|b| then c|d|
  herdr pane close "$p"
  ```

  If `enter` is rejected, try `return` then `cr`; the accepted spelling is what the target binds. If
  the two lines arrive merged or truncated, `send` splits on newlines, one `send-text` per line.
- [ ] **Step 2:** implement the wrapper, the target and the spec. Live: `<leader>Cp` on a paragraph
  lands in the agent pane and runs. Gates G1 to G6; G4 shows `<leader>Cp` and `<leader>CP`. Commits:
  `feat(nvim): wrap the herdr-nvim agent lookup`, `feat(nvim): add vim-slime with a herdr target`.

### Task 23: PR 13, the launch helper `<leader>Cc` (spec 7.2)

Lane: agent. Depends on: PR 12 (`claudecode.lua`), PR 11 (`custom_api/herdr.lua`, spec 11). Brief:
`brief-nvim-launch-helper.md`. Closes 77 (launch).

**Files:** Modify `lua/custom_api/herdr.lua` (`agent_name`, `plan_launch`, `launch_or_attach`),
`lua/plugins/claudecode.lua` (`<leader>Cc`, `desc = "Claude: launch or attach --ide"`). Create
`tests/herdr_spec.lua`.

**Interfaces:** `herdr.agent_name(pane_id)` (pure) returns `"claude-"` plus the id lowercased with
`:` as `-` (`wW:p3K` gives `claude-ww-p3k`). `herdr.plan_launch(pane_id, cwd, servername)` (pure)
returns `{ "prompt", pane_id }` when a pane was found, else `{ "split", cwd, servername }`.
`launch_or_attach()` runs the plan: `herdr agent prompt <pane> /ide` (it refuses while blocked, which
is right); or `herdr pane split --current --direction right --cwd <cwd> --focus --env
NVIM_MCP_SOCKET=<vim.v.servername>`, reads `.result.pane.pane_id`, then `herdr agent start <name>
--kind claude --pane <id> -- --ide`, retrying once with a `-2` suffix if the name is rejected.

- [ ] **Step 1, red:** `agent_name` on `wW:p3K`; `plan_launch` with and without a pane. FAIL.
  **Step 2:** implement; before binding, one `herdr pane split` with `--env` on a scratch pane:
  confirm `.result.pane.pane_id` and that the new shell sees `NVIM_MCP_SOCKET`; JSON in the body.
- [ ] **Mutants:** always split (red); drop the lowercasing (red).
- [ ] **Step 3:** Gates G1 to G6; G4 shows `<leader>Cc`. Commit: `feat(nvim): launch or attach claude
  --ide from the editor`.

### Task 24: PR 23, the git-blame remap then drop (spec 5.2, decision C)

Lane: custom_api. Depends on: PR 7f (`custom_api/git.lua`, `custom_api/github.lua`, both specs),
PR 18 (`git.lua`). Brief: `brief-nvim-blame-remap.md`. Closes 21, 28.

**Files:** Modify `lua/custom_api/git.lua` (`parse_blame_porcelain`, `blame_sha`),
`lua/custom_api/github.lua` (`commit_url`), `tests/git_spec.lua`, `tests/github_spec.lua`,
`lua/plugins/git.lua` (gitsigns `on_attach` gains `<C-g>By`, `<C-g>Bo`, `<C-g>BO`; `<C-g>Bt` to
`gitsigns.toggle_current_line_blame`; `current_line_blame = true`; delete `:1131-1140`), the lock.

**Interfaces:** `git.parse_blame_porcelain(text)` returns `(sha, err)`, err on an all-zero SHA ("not
committed yet"). `git.blame_sha({ file, line })` runs `git blame -L <n>,<n> --porcelain -- <file>`
through `git.runner`. `github.commit_url(sha)` builds the GitHub commit URL from `github.repo()`.

- [ ] **Step 1, red:** porcelain with a real SHA; all-zero SHA gives err; `blame_sha` with a fake
  runner; `commit_url` shape. FAIL. **Step 2:** implement; the three keymaps in `on_attach`; then the
  drop in a second commit. **Mutants:** accept the zero SHA (red); return the second token (red).
- [ ] **Step 3:** Gates G1 to G6; G4 shows the four `<C-g>B*` maps leaving the global pass and the
  three appearing in the buffer-local pass, group row unchanged. Commits: `feat(nvim): rebuild the
  blame keymaps on custom_api and gitsigns`, `chore(nvim): drop git-blame.nvim`.

### Task 25: PR 16, custom #3, the agent-context sender (spec 7.7, 8.3)

Lane: agent. Depends on: PR 12, PR 13 (`claudecode.lua`), PR 23 (`blame_sha`). Brief:
`brief-nvim-agent-context.md`. Closes custom #3.

**Files:** Create `lua/custom_api/agent_context.lua`, `tests/agent_context_spec.lua`. Modify
`lua/plugins/claudecode.lua` (`<leader>Cx`, `desc = "Claude: send context"`).

**Interfaces:** `compose_text(parts)` with `parts = { mention, diagnostic, func, blame }` (any but
`mention` may be nil) returns one string, one part per line. `may_send(status)` (pure) is true on
`idle` or `done`, false on `working`, `blocked`, `unknown`. `compose()` gathers `@<rel>:<line>`, the
first `vim.diagnostic.get` on the line, the enclosing treesitter node whose type ends in
`function_definition`, `function_declaration` or `method_definition`, `git.blame_sha` plus
`git.latest_commit`. `send(text)`: `herdr.agent_pane()`, `herdr agent get <pane>` for
`.result.agent.agent_status`; on `may_send` run `herdr pane send-text <pane> <text>` (no Enter);
otherwise hold it in a one-slot queue (a newer send replaces it, with a notice) and run `herdr agent
wait <pane> --until idle --until done --timeout 600000` detached through `vim.system`, sending on
exit 0 and dropping with a notice otherwise. BEST-EFFORT, and the header says so (7.7): two herdr
calls cannot be atomic, so three rules narrow the window. One waiter: `send` while a `wait` is
running replaces the queued text and starts no second `wait` (`M.waiter` is the one handle). Recheck:
immediately before EVERY `send-text`, on the direct path and in the waiter's `on_exit` alike, read
`agent get` again and send only if `may_send` still holds. Drop, never retry: a failed recheck drops
the text with a notice naming the state seen. Never `agent prompt`; the header says this is stricter
than `herdr-nvim`'s `dispatch.send`. The state machine is pure: `next_action(state, queued, waiting)`
returns `send`, `queue`, `replace` or `drop` so the rules are unit-tested without herdr.

- [ ] **Step 1, red:** `may_send` for all five states; `next_action`: idle with nothing queued is
  `send`; working with no waiter is `queue`; working with a waiter running is `replace` (no second
  waiter); a recheck that reads `working` after exit 0 is `drop`; all four parts; mention only; nil
  parts leave no blank line. FAIL. **Step 2:** implement, with a fake `agent get` sequence in the spec
  (`idle` then `working` between check and send gives `drop`). Live (10.9): `<leader>Cx` on a
  diagnostic line with the agent `idle` puts the three lines in the prompt unsent; with it `working`
  nothing is typed until it settles, then the held text arrives; a second `<leader>Cx` while it waits
  replaces the queued text and `ps` shows one `herdr agent wait`. Pasted.
- [ ] **Mutants:** `may_send` true on `working` (red); skip the recheck before the send (the
  `idle`-then-`working` case red); start a second waiter on `replace` (red); drop the diagnostic
  (red); join with spaces.
- [ ] **Step 3:** Gates G1 to G6; G4 shows `<leader>Cx`. Commit: `feat(nvim): send cursor context to
  the agent pane when it is idle`.

### Task 26: PR 14, custom #1, the editor-side pns producer (spec 7.7, 10.9)

Lane: agent. Depends on: PR 7f, PR 3 (the xcodebuild spec), PR 19b (`overseer.lua`). Brief:
`brief-nvim-task-events.md`. Closes custom #1.

**Files:** Create `lua/custom_api/task_events.lua`, `tests/task_events_spec.lua`. Modify
`lua/plugins/overseer.lua` (an `on_complete` component), `lua/plugins/xcodebuild.lua` (`User`
autocmds). The neotest edge is PR 28's.

**Interfaces:** `task_events.tier(seconds)` returns `"none"` under 30, `"notify"` from 30, `"long"`
from 300. `task_events.detail(tool, task, seconds)` returns `"<tool>: <task> (35s)"` under a minute,
`(5m12s)` from one. `report({ tool, task, state, seconds })` runs `~/.local/libexec/pns/pns --agent
nvim --state done|failed --project <cwd basename> --detail <detail> --pane $HERDR_PANE_ID` plus
`--long-running` at `"long"`, nothing at `"none"`. Edge, verified at the pin: xcodebuild.nvim fires
`User` patterns `XcodebuildBuild{Started,Finished}` and `XcodebuildTests{Started,Finished}`
(`lua/xcodebuild/broadcasting/events.lua`), duration between the pair.

- [ ] **Step 1, red:** `tier` at 29, 30, 299, 300; `detail` at 35 and 312; `report` under a fake
  `vim.system` records argv, none at 10 s, `--long-running` at 300 s. FAIL. **Step 2:** implement; the
  two edges; the module name confirmed with the operator first. Live (10.9): a 35 s overseer task
  gives one Discord card whose detail reads `overseer: <task> (35s)`; a 10 s one gives none. The
  banner is not asserted (the engine suppresses it for the watched pane).
- [ ] **Mutants:** threshold 30 to 31 (red at 30); drop `--long-running` (red); report at `"none"`.
- [ ] **Step 3:** Gates G1 to G6; G4 unchanged. Commit: `feat(nvim): report task completions to pns`.

### Task 27: PR 15, custom #2, the review-ledger quickfix (spec 7.7)

Lane: agent. Depends on: PR 7f. Brief: `brief-nvim-review-ledger.md`. Closes custom #2.

**Files:** Create `lua/custom_api/review_ledger.lua`, `tests/review_ledger_spec.lua`,
`tests/fixtures/findings-sample.md`. Modify `lua/config/keymaps.lua` (the `:ReviewLedger[!] [file]`
command; no keymap).

**Interfaces:** `review_ledger.parse(lines, path, include_fixed)` returns quickfix items
`{ filename, lnum, text }`; `filename`/`lnum` from a `path:line` token in the summary when present,
else `path` and the row's line; `text = "F<n> <severity> <disposition>: <summary>"`; `FIXED` rows
skipped unless `include_fixed`. Default file: newest `~/.claude/pipeline/slices/findings-*.md`.

- [ ] **Step 1, red:** a row with `path:line`; a row without; a FIXED row skipped then included with
  the bang; a non-`| F` line ignored. FAIL. **Step 2:** implement; the command. Live: `:ReviewLedger`
  on a real findings file, `:cnext` lands on the row. Pasted.
- [ ] **Mutants:** never skip FIXED (red); ignore the `path:line` token (red).
- [ ] **Step 3:** Gates G1 to G6; G4 unchanged. Commit: `feat(nvim): load a findings ledger into the
  quickfix list`.

### Task 28: PR 17a, drop cspell (spec 5.1)

Lane: LSP and tools. Depends on: PR 5b (`lsp.lua`). Brief: `brief-nvim-drop-cspell.md`. Closes 23.

- [ ] **Step 1:** remove the dep at `lsp.lua:247` and the lock line, one commit: `chore(nvim): drop
  cspell.nvim`.
- [ ] **Step 2:** Gates G1 to G6; G4 shows one plugin name gone; `:checkhealth` has no new line.

### Task 29: PR 17b, drop gitmoji (spec 5.1)

Lane: drops and git. Depends on: PR 2. Brief: `brief-nvim-drop-gitmoji.md`. Closes 24.

- [ ] **Step 1:** `blink-cmp.lua:114` dep, `:261-263` provider, `:279` `sources.default`, the lock
  line, one commit: `chore(nvim): drop gitmoji.nvim`.
- [ ] **Step 2:** Gates G1 to G6; G4 shows one plugin name gone; completion still works live.

### Task 30: PR 17c, drop nvim-notify (spec 5.1)

Lane: drops and git. Depends on: PR 2. Brief: `brief-nvim-drop-notify.md`. Closes 25.

- [ ] **Step 1:** remove the dep at `noice.lua:11` and the lock line: `chore(nvim): drop nvim-notify
  for snacks.notifier`.
- [ ] **Step 2:** Gates G1 to G6; G4 shows one plugin name gone; a `vim.notify` renders through snacks.

### Task 31: PR 17d, drop gv.vim (spec 5.1)

Lane: drops and git. Depends on: PR 7f (`git.lua`). Brief: `brief-nvim-drop-gv.md`. Closes 26.

- [ ] **Step 1:** remove it from the fugitive deps at `git.lua:255` and the lock line: `chore(nvim):
  drop gv.vim`.
- [ ] **Step 2:** Gates G1 to G6; G4 shows one plugin name gone; `<leader>gl` still opens the log.

### Task 32: PR 17e, drop git-messenger, re-point `<leader>gm` (spec 5.1, 8.3)

Lane: drops and git. Depends on: PR 17d (`git.lua`). Brief: `brief-nvim-drop-git-messenger.md`.
Closes 27.

- [ ] **Step 1:** remove the spec and re-point `<leader>gm` at `gitsigns.blame_line({ full = true })`,
  the lock line, one commit: `refactor(nvim): drop git-messenger, <leader>gm to gitsigns blame_line`.
- [ ] **Step 2:** Gates G1 to G6; G4 shows the name gone and `<leader>gm` with a new `rhs` only.

### Task 33: PR 18, the dead `<C-g>bc` (bug #2)

Lane: drops and git. Depends on: PR 17e (`git.lua`). Brief: `brief-nvim-dead-keymap.md`. Closes 2.

- [ ] **Step 1:** delete `git.lua:464-475`. Commit: `fix(nvim): remove the dead <C-g>bc definition`.
- [ ] **Step 2:** Gates G1 to G6; G4 unchanged (`:538-543` already won). No test.

### Task 34: PR 19a, overseer bug #3, the `<M-[>` binding

Lane: standalone. Depends on: PR 2. Brief: `brief-nvim-overseer-watch-run.md`. Closes 3.

- [ ] **Step 1:** `<M-[>` to `vim.cmd("OverseerWatchRun")` (`overseer.lua:412-414`). Commit:
  `fix(nvim): bind <M-[> to OverseerWatchRun`.
- [ ] **Step 2:** Gates G1 to G6; G4 shows `<M-[>` with a new `rhs` only; `<M-[>` runs live. No test.

### Task 35: PR 19b, overseer bug #14, the dead config

Lane: standalone. Depends on: PR 19a (`overseer.lua`). Brief: `brief-nvim-overseer-cleanup.md`. Closes
15.

- [ ] **Step 1:** `run_template` to `run_task`; delete the dead `bundles` and `log` tables
  (`overseer.lua:192-200`, the `log` block). Commit: `refactor(nvim): drop dead overseer config`.
- [ ] **Step 2:** Gates G1 to G6; G4 unchanged; `<leader>o` tasks still run live. No test.

### Task 36: PR 20, auto-reload with re-arm (spec 5.4, bug #6)

Lane: standalone. Depends on: PR 4b (`autocmds.lua`). Brief: `brief-nvim-auto-reload.md`. Closes 7,
40.

**Files:** Create `lua/custom_api/auto_reload.lua`, `tests/auto_reload_spec.lua`. Modify
`lua/config/autocmds.lua` (the `nvim_config_auto_reload` augroup), `lua/config/options.lua:116-118`
(delete the duplicate `checktime`).

**Interfaces:** `auto_reload.watch(bufnr)` starts a `vim.uv.new_fs_event()` on
`vim.uv.fs_realpath(name)` if none; the callback schedules `checktime <bufnr>` then re-arms (stop,
close, start on the current realpath). `auto_reload.unwatch(bufnr)` stops and closes.

- [ ] **Step 1, red:** in the headless runner (real temp file; `--clean` keeps `vim.uv` and buffers):
  open, `watch`, overwrite in place from `io.open`, `vim.wait(500, …)` until the buffer shows the new
  text; replace by `os.rename` twice and the buffer follows both times; `unwatch` then a write leaves
  the buffer stale. FAIL. **Step 2:** implement, about 30 lines; wire the augroup on `BufReadPost`,
  `BufWritePost`, `BufDelete`, `BufUnload`; delete the `options.lua` copy.
- [ ] **Mutants:** drop the re-arm (rename case red on the second rename); drop the `checktime`
  schedule (case 1 red); ignore `unwatch` (case 3 red).
- [ ] **Step 3:** Gates G1 to G6; G4 unchanged. Commits: `fix(nvim): remove the duplicate checktime
  autocmd`, `feat(nvim): reload buffers when an agent writes the file`.

### Task 37: PR 21, the markdown preview host (bug #11)

Lane: standalone. Depends on: PR 2. Brief: `brief-nvim-mkdp-host.md`. Closes 12.

**Files:** Create `lua/custom_api/preview_host.lua`, `tests/preview_host_spec.lua`. Modify
`lua/plugins/markdown.lua:314`.

**Interfaces:** `preview_host.resolve(hostname, suffix_env)` returns `hostname .. "." .. suffix`
when `suffix_env` is set and non-empty, else `"127.0.0.1"`. `mkdp_open_ip` reads
`resolve(vim.fn.hostname(), vim.env.NVIM_MKDP_HOST)`.

- [ ] **Step 1, red:** suffix set; nil; empty string. FAIL. **Step 2:** implement. **Mutants:** the
  hostname alone when set (red); loopback always (red).
- [ ] **Step 3:** Gates G1 to G6; live: the preview opens off dresden (10.6). Commit: `fix(nvim):
  derive the markdown preview host from the hostname`.

### Task 38: PR 22a, harpoon width at spec load (bug #10)

Lane: standalone. Depends on: PR 2. Brief: `brief-nvim-harpoon-width.md`. Closes 11.

- [ ] **Step 1:** `harpoon.lua:6` becomes `opts = function() … end`. Commit: `fix(nvim): defer the
  harpoon width read to opts()`.
- [ ] **Step 2:** Gates G1 to G6; G4 unchanged. No test.

### Task 39: PR 22b, noice inc_rename (bug #17)

Lane: drops and git. Depends on: PR 17c (`noice.lua`). Brief: `brief-nvim-noice-inc-rename.md`.
Closes 18.

- [ ] **Step 1:** `noice.lua:36` `inc_rename = false`. Commit: `fix(nvim): turn off noice inc_rename
  for the absent plugin`.
- [ ] **Step 2:** Gates G1 to G6; G4 unchanged. No test.

### Task 40: PR 24, drop telescope (decision B)

Lane: drops and git. Depends on: PR 23 (`git.lua`), PR 12 (`autosave.lua`). Brief:
`brief-nvim-drop-telescope.md`. Closes 29, 32.

- [ ] **Step 1:** one commit: remove `git.lua:1142-1155`, the octo dep `:1161`, the `telescope` key
  in `chezmoi.lua`, `TelescopePrompt` in `autosave.lua`, the lock line. `chore(nvim): drop telescope`.
- [ ] **Step 2:** Gates G1 to G6; G4 shows the telescope maps gone. The octo `<localleader>` groups are
  outside the dump (3.7): press `\` in an octo buffer and record the popup in the body. No test.

### Task 41: PR 25, dial augends then drop boole (spec 5.1, 8.3)

Lane: drops and git. Depends on: PR 2. Brief: `brief-nvim-dial-augends.md`. Closes 30.

- [ ] **Step 1:** write the dial spec with `augend.constant.new` for every `additions` and
  `allow_caps_additions` pair in `boole.lua`, keys `<C-a>`/`<C-x>` in normal and visual; delete
  `boole.lua` in the same commit. `refactor(nvim): move boole's toggles to dial and drop boole`.
- [ ] **Step 2:** Gates G1 to G6; G4 shows `<C-a>`/`<C-x>` with a new `rhs` and boole gone; live:
  `true` toggles to `false`. No test.

### Task 42: PR 26a, bump nvim-surround (spec 5.5)

Lane: standalone. Depends on: PR 2. Brief: `brief-nvim-bump-surround.md`. Closes 33, 34 (no-op).

- [ ] **Step 1:** `textobjects.lua:4` `version = "^4.0.0"`, the lock. Commit: `chore(nvim): bump
  nvim-surround to ^4`. Body: none-ls is not bumped (item 34, `0b45795` is the rollback anchor).
- [ ] **Step 2:** Gates G1 to G6; G4 unchanged; live `ys`/`cs`/`ds`. No test.

### Task 43: PR 26b, bump hlslens (bug #15)

Lane: standalone. Depends on: PR 2. Brief: `brief-nvim-bump-hlslens.md`. Closes 16.

- [ ] **Step 1:** the pin `4254054` to `be2d7b2`: `chore(nvim): bump hlslens past the vim.validate fix`.
- [ ] **Step 2:** Gates G1 to G6; G4 unchanged; health loses the one `vim.deprecated` warning.

### Task 44: PR 26c, bump catppuccin with the rename (bug #16)

Lane: standalone. Depends on: PR 2. Brief: `brief-nvim-bump-catppuccin.md`. Closes 17.

- [ ] **Step 1:** bump past `605b460`; `ui.lua:60` to `"catppuccin-nvim"`; `:7` stays; one commit:
  `chore(nvim): bump catppuccin and rename the colorscheme call`.
- [ ] **Step 2:** Gates G1 to G6; G4 unchanged; the colorscheme loads with no message. No test.

### Task 45: PR 27, gopls and go (decision D)

Lane: LSP and tools. Depends on: PR 17a (`lsp.lua`). Brief: `brief-nvim-gopls.md`. Closes 37.

- [ ] **Step 1:** `brew install go`; add `go` to the YAML formulae; `"gopls"` to mason-lspconfig
  `ensure_installed`. Two commits: `build(packages): add go for Mason's gopls`, `feat(nvim): ensure
  gopls is installed`.
- [ ] **Step 2:** Gates G1 to G6; `:checkhealth mason` shows Go available. No test.
- [ ] **Step 3, OPERATOR:** `chezmoi apply` reconciles the bundle.

### Task 46: PR 28, neotest (spec 5.3, 8.3, 12.1)

Lane: standalone. Depends on: PR 3, PR 12 (`which-key.lua`), PR 14 (`task_events`). Brief:
`brief-nvim-neotest.md`. Closes 44.

- [ ] **Step 1:** `lua/plugins/neotest.lua`: `nvim-neotest/neotest` `commit = "27bf921"`,
  `nvim-neotest/nvim-nio`, adapters `webdavis/neotest-swift` (`7487799`) and `rouge8/neotest-rust`
  (`2c9941d`), keys `<leader>tt` nearest, `tf` file, `ta` all, `ts` summary, `to` output, `tS` stop,
  group row `{ "<leader>t", group = "test" }`; the `client.listeners.results` edge
  (`lua/neotest/client/events/init.lua` at the pin) reporting through `task_events.report`.
- [ ] **Step 2:** Gates G1 to G6; G4 shows the six maps and the group; live: `<leader>tt` on a Rust
  test runs; a 35 s run gives the 10.9 card. Commit: `feat(nvim): add neotest with Swift and Rust
  adapters`. No unit test.

### Task 47: PR 29a, health floor, none-ls gating (spec 4, 10.2)

Lane: LSP and tools. Depends on: PR 27 (`lsp.lua`). Brief: `brief-nvim-none-ls-gating.md`. Closes
68 (none-ls half), 71 (health half).

- [ ] **Step 1:** wrap the `nixfmt`, `rubocop`, `eslint` sources in `.with({ condition =
  function(utils) return utils.executable("<bin>") end })`. Commit: `fix(nvim): gate none-ls sources
  on their binaries`.
- [ ] **Step 2:** Gates G1 to G6; the health file loses the three ERROR lines, nothing else changes.

### Task 48: PR 29b, health floor, the treesitter runtimepath line (spec 4, 5.5, 10.2)

Lane: standalone. Depends on: PR 2. Brief: `brief-nvim-treesitter-health.md`. Closes 36 (note), 68,
71 (health half).

- [ ] **Step 1:** reproduce the "is not in runtimepath" line, find the cause, fix it or record it as a
  health-check artifact with the evidence. Confirm the `nvim-treesitter-context` bar renders on
  0.12.5 and note the archival of nvim-treesitter in the body. Commit accordingly.
- [ ] **Step 2:** TUI `:checkhealth snacks`: the two `vim.ui.*` lines re-checked; fixed if they
  persist, own commit. Gates G1 to G6; every remaining ERROR line is a 10.2 exception, in the body.

### Task 49: PR 30a, startup triggers for the LSP group (spec 9)

Lane: last. Depends on: PR 29a, PR 12 (`lsp.lua`). Brief: `brief-nvim-triggers-lsp.md`. Closes 48
(part).

- [ ] **Step 1:** `event = { "BufReadPre", "BufNewFile" }` on `mason-lspconfig`, `nvim-lspconfig`,
  `none-ls`, `lsp-format`; Mason `cmd = "Mason"` plus the same event through its dependents. Commit:
  `perf(nvim): lazy-load the LSP group on buffer read`.
- [ ] **Step 2:** Gates G1 to G6; G4 unchanged; G5 records the drop (expected to move). A `.c` file
  still attaches clangd with both flags; a keymap or command that no longer fires is a wrong trigger,
  fixed in-round.

### Task 50: PR 30b1, startup triggers for fugitive (spec 9)

Lane: last. Depends on: PR 24 (`git.lua`). Brief: `brief-nvim-triggers-fugitive.md`. Closes 48 (part).

- [ ] **Step 1:** fugitive and its deps `cmd = { "Git", "G", … }` plus the `<C-g>` keys, in `git.lua`.
  Commit: `perf(nvim): lazy-load fugitive on its commands and keys`.
- [ ] **Step 2:** Gates G1 to G6; G4 unchanged; G5 records the drop; every `<C-g>` map fires live.

### Task 51: PR 30b2, startup triggers for octo (spec 9)

Lane: last. Depends on: PR 30b1 (`git.lua`). Brief: `brief-nvim-triggers-octo.md`. Closes 48 (part).

- [ ] **Step 1:** octo `cmd = "Octo"` plus its `<leader>gh` keys, in `git.lua`. Commit: `perf(nvim):
  lazy-load octo on its command and keys`.
- [ ] **Step 2:** Gates G1 to G6; G4 unchanged; G5 records the drop; the `<leader>gh` maps fire live
  and the octo `<localleader>` popup still appears.

The nine 30c tasks below share one shape (spec 9, spec 11): lane last; each depends on PR 1 to 29b
(the lane rule) plus the shared-file predecessor its Depends line names; each adds ONE trigger to ONE
spec file and changes nothing else there; each runs gates G1 to G6 with G4 unchanged (lazy `keys`
still dump) and G5 recording the drop; every keymap and command the trigger names fires live, and one
that no longer fires is a wrong trigger, fixed in-round; one commit, `perf(nvim): lazy-load <plugin>`.
The specs the section 9 table lists as already triggered (`kulala`, `ansible`, `docker`, the markdown
group, `grug-far`, `codesnap`, `xcodebuild.nvim` and the rest of that row) get NO PR: lazy.nvim already
marks them lazy, verified 2026-09-01 against the live spec files.

### Task 52: PR 30c1, startup trigger for treesj (spec 9)

Lane: last. Depends on: PR 1 to 29b. Brief: `brief-nvim-trigger-treesj.md`. Closes 48 (part).

- [ ] **Step 1:** `treesj.lua` gains `keys` for its five `<leader>j` maps. Commit: `perf(nvim):
  lazy-load treesj`.
- [ ] **Step 2:** the shared gates above; the five `<leader>j` maps fire live.

### Task 53: PR 30c2, startup trigger for auto-save (spec 9)

Lane: last. Depends on: PR 1 to 29b; PR 24 (`autosave.lua`). Brief: `brief-nvim-trigger-autosave.md`.
Closes 48 (part).

- [ ] **Step 1:** `autosave.lua` gains `event = { "InsertLeave", "TextChanged" }` and the `<leader>uv`
  toggle in `keys`. Commit: `perf(nvim): lazy-load auto-save`.
- [ ] **Step 2:** the shared gates above; an edit still auto-saves and `<leader>uv` still toggles.

### Task 54: PR 30c3, startup trigger for overseer (spec 9)

Lane: last. Depends on: PR 1 to 29b; PR 14 (`overseer.lua`). Brief: `brief-nvim-trigger-overseer.md`.
Closes 48 (part).

- [ ] **Step 1:** `overseer.lua` gains `cmd` plus its `<leader>o` and `<M-…>` keys. Commit:
  `perf(nvim): lazy-load overseer`.
- [ ] **Step 2:** the shared gates above; `<leader>o`, `<M-7>`, `<M-8>`, `<M-;>` and `<M-[>` fire
  live and a task completion still reaches pns (PR 14's edge).

### Task 55: PR 30c4, startup trigger for harpoon (spec 9)

Lane: last. Depends on: PR 1 to 29b; PR 22a (`harpoon.lua`). Brief: `brief-nvim-trigger-harpoon.md`.
Closes 48 (part).

- [ ] **Step 1:** `harpoon.lua` gains `keys` (`<C-p>`, `<C-n>` and its `<leader>` maps). Commit:
  `perf(nvim): lazy-load harpoon`.
- [ ] **Step 2:** the shared gates above; every harpoon map fires live.

### Task 56: PR 30c5, startup trigger for urlview (spec 9)

Lane: last. Depends on: PR 1 to 29b. Brief: `brief-nvim-trigger-urlview.md`. Closes 48 (part).

- [ ] **Step 1:** `urlview.lua` gains `cmd` and its `<leader>U` keys. Commit: `perf(nvim): lazy-load
  urlview`.
- [ ] **Step 2:** the shared gates above; `<leader>U` maps and `:UrlView` fire live.

### Task 57: PR 30c6, startup trigger for sort (spec 9)

Lane: last. Depends on: PR 1 to 29b. Brief: `brief-nvim-trigger-sort.md`. Closes 48 (part).

- [ ] **Step 1:** `sort.lua` gains `cmd = "Sort"` and its keys. Commit: `perf(nvim): lazy-load sort`.
- [ ] **Step 2:** the shared gates above; `:Sort` and its keys fire live.

### Task 58: PR 30c7, startup trigger for live-rename (spec 9)

Lane: last. Depends on: PR 1 to 29b. Brief: `brief-nvim-trigger-live-rename.md`. Closes 48 (part).

- [ ] **Step 1:** `live-rename.lua` gains `keys`. Commit: `perf(nvim): lazy-load live-rename`.
- [ ] **Step 2:** the shared gates above; the rename map fires live on an LSP buffer.

### Task 59: PR 30c8, startup trigger for aerial (spec 9)

Lane: last. Depends on: PR 1 to 29b. Brief: `brief-nvim-trigger-aerial.md`. Closes 48 (part).

- [ ] **Step 1:** `aerial.lua` gains `cmd` and its keys. Commit: `perf(nvim): lazy-load aerial`.
- [ ] **Step 2:** the shared gates above; the aerial toggle fires live and the PR 2 close-sidebars
  autocmd still closes it on quit.

### Task 60: PR 30c9, startup trigger for claudecode.nvim (spec 9, 7.2)

Lane: last. Depends on: PR 1 to 29b; PR 16 (`claudecode.lua`). Brief:
`brief-nvim-trigger-claudecode.md`. Closes 48 (part).

- [ ] **Step 1:** `claudecode.lua` gains `event = "VeryLazy"` (the lock file must exist before the CLI
  connects, so `cmd` alone is wrong). Commit: `perf(nvim): lazy-load claudecode.nvim`.
- [ ] **Step 2:** the shared gates above; in a TUI session `claude --ide` still connects after
  `VeryLazy` and `<leader>Cc`, `Cs`, `Cx`, `Cp` fire live, recorded.

### Task 61: PR 30d, the lazy flip (spec 9, 9.1)

Lane: last. Depends on: PR 30a, 30b1, 30b2, 30c1 to 30c9. Brief: `brief-nvim-lazy-flip.md`. Closes
48.

- [ ] **Step 1, the set, before editing:** `:Lazy` under the harness lists every spec; the ones with
  no `event`, `keys`, `ft` or `cmd` and no `lazy` key are the candidates, expected to be exactly the
  section 9 "gets `lazy = false`" row: `catppuccin`, `bufferline`, `deadcolumn`, `herdr-nvim`,
  `hlslens`, `mini.move`, `quick-scope`, `ts-comments`, `vim-rsi`, `vim-repeat`, `dial`. Any other
  name needs a stated reason in the body. Pasted.
- [ ] **Step 2:** `lazy.lua:41` `defaults.lazy = true`; `lazy = false` written ONLY onto that set.
  Never onto `which-key`, `noice`, `blink-cmp`, `textobjects`, `unimpaired` or any spec with a
  trigger (an explicit `lazy` overrides the trigger, `lazy/core/plugin.lua:235-241`), and not onto
  the specs that already carry `lazy = false` (`snacks`, `smart-splits`, `oil`, `markview`,
  `witch-line`, `helpview`, `nvim-treesitter`, `blink.nvim`). One commit: `perf(nvim): flip
  defaults.lazy with the untriggered set pinned eager`.
- [ ] **Step 3:** Gates G1 to G6; G4 unchanged; G5 is the strict form: `after <= before + 10` AND
  `after < baseline - 10`; cold and the TUI number recorded beside it, and the TUI run confirms
  which-key, noice and blink-cmp load after `VeryLazy` and `InsertEnter` rather than at startup
  (`:Lazy` shows them as lazy-loaded with their trigger). A spec that stopped loading is fixed
  in-round with an explicit trigger, or with `lazy = false` only if it has none.

### Task 62: PR 31, the bootstrap and the clean-home apply (spec 3.9, 10.10, 10.11)

Lane: last. Depends on: PR 30d. Brief: `brief-nvim-bootstrap.md`. Closes 50, 66 (bootstrap), 69, 70,
72.

**Files:** Create `.chezmoiscripts/run_onchange_after_80-bootstrap-nvim.sh.tmpl`,
`dot_local/libexec/executable_verify-nvim-bootstrap.sh`, `test/unit/nvim-bootstrap-verify.bats`.

**Interfaces:** `missing_lazy_dirs <lazy-lock.json> <lazy-dir>` prints each pinned name without a
directory; `missing_mason_packages <names-file> <mason-packages-dir>` prints each absent name; main
prints the missing names with their tool and exits 1 when any, else nothing.

- [ ] **Step 1, red:** bats fed a fixture lock and a scratch tree: all present prints nothing; one
  missing prints exactly that name; a name present as a file, not a directory, is missing. FAIL.
- [ ] **Step 2:** implement the library (bash 3.2, jq for the lock); the template: embedded
  `include | sha256sum` of `lazy-lock.json` and `lua/plugins/lsp.lua`, the retry marker mtime as
  `run_onchange_after_58` does, `command -v nvim` or defer, no darwin guard, no timeout,
  `+Lazy! restore`, `+MasonToolsInstallSync`, the runner with `--config "$HOME/.config/nvim"`, the
  verify library, the clangd `cmd` assertion, quiet on no-op; the 3.9 prerequisites in its header.
- [ ] **Mutants:** treat a file as a directory (case 3 red); print nothing when one is missing (red).
- [ ] **Step 3:** Gates G1 to G6 (the rendered template is shellchecked by the treefmt wrapper); G5 in
  its strict form, `after < baseline - 10` as well.
- [ ] **Step 4, OPERATOR:** a throwaway user on dresden, full `chezmoi apply`; the bootstrap exits 0;
  acceptance items 1 to 4 pass in that home; a second apply prints nothing; transcript in the body.
- [ ] **Step 5:** task 63 lands its record as this branch's last commit; then merge. Commits:
  `feat(chezmoi): bootstrap the Neovim plugin and tool state at apply time`, `test(nvim): pin the
  bootstrap verification`, then task 63's.

### Task 63: Acceptance bar (spec 10, appendix A)

Depends on: PR 31 gated and applied (its step 4), on PR 31's branch before its merge. Brief:
`brief-nvim-acceptance.md`. Output: `docs/research/2026-09-nvim-overhaul-acceptance.md`, committed as
`docs(nvim): record the overhaul acceptance run`. A record, no code.

- [ ] **Step 1:** `nvim --headless +qa`, five runs, every stderr file 0 bytes. Pasted.
- [ ] **Step 2:** `checkhealth` to a file from the fixed directory: the three none-ls errors and the
  treesitter runtimepath error are gone; every remaining ERROR line is a 10.2 exception (`luarocks`,
  `gs`, `tectonic`, `pdflatex`, `mmdc`, `lazygit`, kitty graphics), listed with its line.
- [ ] **Step 3:** the 9.1 method from `$BENCH` with no agent running, `User VeryLazy` fired by hand,
  the number labelled synthetic: `after < baseline - 10` against the PR 2 import-day baseline; cold
  recorded, not gated. Then the TUI acceptance check (10.3): `nvim --startuptime` inside a herdr
  pane starts with no error and `:Lazy` shows every `VeryLazy` plugin loaded; its number recorded,
  not gated.
- [ ] **Step 4:** which-key: `<leader>` shows every group named (`A` herdr, `C` claude, `X` xcode,
  `t` test, `d` do, `L` lazy among them); each is entered and lists its maps; `<leader>b?` lists the
  buffer-local maps. The popup text goes in the record.
- [ ] **Step 5:** the 7.3 row taken, which of PR 10a and 10b shipped and registered the server and
  which was skipped, the 10.8 loop from both harnesses as that registering PR recorded it.
- [ ] **Step 6:** the live bug checks (10.6), Swift (10.7), custom plugins (10.9: the Discord card,
  the ledger, the idle and working sends), the clean-home apply (10.10) and the quiet second apply
  (10.11), each with the PR that proved it.
- [ ] **Step 7:** walk appendix A: all 78 inventory items plus A to H and custom #1 to #4 closed, each
  with its merged PR number; the five struck items listed as struck; 22, 38, 64 recorded as
  informational, deferred, deferred. Nothing open means the program is done.
