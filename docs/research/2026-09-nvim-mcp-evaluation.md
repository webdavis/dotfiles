# nvim-mcp evaluation, 2026-09-04

The record spec 7.3 of `docs/superpowers/specs/2026-09-01-nvim-overhaul-design-v4.md` asks PR 9 (plan
task 18) to produce. It answers one question: does `linw1995/nvim-mcp` meet the six criteria of 7.3
against the live herdr setup, and therefore which of PR 10a and PR 10b ship. Nothing was installed
through chezmoi, nothing was registered in either harness, and no `CLAUDE.md` was edited. The candidate
was built by hand into a throwaway cargo root and trashed at the end.

## Verdict

**Row taken: "5, 6, 4 pass; any of 1, 2, 3 fails or undecided", the resolver row.** Criteria 5 and 4 pass
outright. **Criterion 6 passes only under an amendment this record makes and states**: its "YAML-declared
toolchain" clause is replaced by the guarded rustup policy the pns and uu builders already use, because
no builder in this repository meets the clause as written. Against the clause as written, criterion 6
fails, and the reasoning is in its section below. Criterion 1 fails as shipped on this machine, criterion
2 passes only under a configuration workaround, and criterion 3 fails on the server's own heuristic.
Under the five-step selection design (7.3, decided 2026-09-03) both failures cost nothing, because the
resolver never uses the server's discovery: it pins by socket path (criterion 4), which is what passes.

- **PR 10a ships on the resolver row**: it installs `nvim-mcp` at `0b5ace3`, adds
  `~/.local/libexec/nvim-mcp-connect.sh` with its bats file, registers the resolver as the server command
  in both harnesses, and adds the 7.5 rule. **The resolver is not yet shown to close criterion 3.** Its
  steps were exercised by a hand script, not over the protocol, so PR 10a owes an end-to-end client
  transcript before that claim is made.
- **Criterion 6's amendment is the operator's to reject.** If it is rejected the decision table's "5 or 6
  fails" row applies instead and the custom crate ships, PR 10a as its design spec and PR 10b as its
  build.
- **PR 10b is skipped.** No crate is built; the second candidate is Linux-only and this one is usable.
- **The Codex registration lands in `private_dot_codex/modify_private_config.toml`, not in a
  `private_config.toml.tmpl`.** Spec 7.3 asked this record to name the pull request that lands
  `private_dot_codex/private_config.toml.tmpl` on `main`. No such file will land: PR #306
  (`codex-modify-config`, merged as `aef04428` on 2026-09-03) made `modify_private_config.toml` the one
  chezmoi source for `~/.codex/config.toml`, and it already declares four `mcp_servers` tables as stable
  fields with `setValueAtPath`. PR 10a adds `mcp_servers.nvim` there as a fifth. Adding a `.tmpl` beside
  it would be the two-sources-for-one-target conflict 7.3 warns about, in the other direction.

## What was evaluated

- **Candidate:** `https://github.com/linw1995/nvim-mcp`
- **Resolved commit:** `0b5ace3b0369801c9bcb8eec68864427e6b1599c` (2026-08-25, "add connection-tool
  compatibility mode (#195)")
- **Crate version:** `0.7.2`, edition 2024, `rust-version = "1.88.0"`
- **Binary version:** `nvim-mcp 0.7.2 (sha:"0b5ace3b…", build_time:"2026-09-05T01:30:04Z")[dirty]`
- **Tools advertised:** 33 by `tools/list` (spec 7.3 wrote 26 on 2026-09-01)
- **Host:** dresden, macOS, herdr 0.8.2, Neovim v0.12.5, cargo 1.92.0-nightly via rustup
- **Neovim run root:** `stdpath("run")` = `$TMPDIR/nvim.stephen/<random>/`, `$TMPDIR` is 49 bytes

`[dirty]` is cosmetic: cargo's git checkout carries an untracked `.cargo-ok` marker, so the crate's
`build.rs` sees a non-empty `git status`. The sha is exact.

## Method

Everything ran in two herdr workspaces created for the purpose and closed at the end (`mcp-eval`, id
`w13`, anchored to the `nvim-eval-mcp` worktree; `mcp-eval-2`, id `w14`, anchored to the main checkout).
The operator was working in workspace `wW` throughout; no pane outside `w13` and `w14` was split,
focused, written to or read from, and every workspace listing during the run showed `wW` still focused.
Pane ids below are herdr's compact live ids and are not stable across sessions. Transcript lines wider
than 105 columns are wrapped here with a hanging indent; each record is one line in the original output.

Neovim ran as `nvim --clean -u /tmp/mc/init.lua`, an init that does three things: prepends the `0b5ace3`
checkout to `runtimepath`, calls `require("nvim-mcp").setup(...)` in one of three modes (`shipped`: the
plugin's defaults; `tmp`: `opts.pipe` set to the same name scheme under `/tmp`; `noplugin`: no `setup`
call at all), and appends `pane_id socket cwd pid` to `/tmp/mc/registry.txt` on `VimEnter`. That last
line is the registry 7.3 says Neovim writes, with the deregister on `VimLeavePre` deliberately left out
so an exited instance leaves a stale entry, which is the case step 3 exists for.

The server was driven over its stdio transport by a 60-line Python script (`initialize`,
`notifications/initialized`, `tools/list`, `resources/read`, `tools/call`), so no harness registration
was needed. For criterion 2 the script ran inside each workspace's own agent pane through
`herdr pane run`; everywhere else it ran from a shell whose cwd was the agent pane's cwd, which is the
only input the server reads.

## The six criteria

Order is the decision table's: 5 and 6 first, then 4, then 1 to 3.

- **5, current buffer: pass.** Path, cursor and unsaved text read, edited and undone; `modified` stayed
  1\.
- **6, install: pass, under the amendment recorded in its section.**
  `cargo install --git --rev 0b5ace3 --locked`, exit 0, no node runtime, system libraries only. Against
  criterion 6's "YAML-declared toolchain" clause as written, it fails.
- **4, explicit socket: pass.** `--connect <abs path>` reached exactly the instance discovery cannot see.
- **1, discovery without `--listen`: fail as shipped.** The plugin socket name is 116 to 123 bytes and
  `sun_path` is 104; a config workaround restores it.
- **2, workspace match: pass, under that workaround.** The worktree and the main checkout each reached
  only their own instance.
- **3, native choice within workspace: fail on its own heuristic.** With two unpinned instances it
  connects to both and cannot name the pane's one. Moot under the design.

### Criterion 5, current buffer: pass

Setup: N1 (`w13:p2`, pid 99074) held `/tmp/mc/eval-N1.txt`; an unsaved line was typed by keystrokes, not
by API, so the buffer state is what a human leaves behind.

```text
$ nvim --server "$sock" --remote-send 'ggOunsaved first line<Esc>'
modified=1  bufname=/private/tmp/mc/eval-N1.txt
/tmp/mc/eval-N1.txt: OK          # shasum -c against the pre-edit hash: disk untouched
```

Then, through the MCP tools on connection `3d97fa3` (`--connect auto`, trimmed to the result text):

```text
cursor_position  {"buffer_id":1,"buffer_name":"/private/tmp/mc/eval-N1.txt","col":17,"row":0,
                  "window_id":1000}
list_buffers     [{"id":1,"name":"/private/tmp/mc/eval-N1.txt","line_count":4}]
read             "unsaved first line\nline one\nline two\nline three"
exec_lua         vim.api.nvim_buf_set_lines(0, 0, 1, false, {"edited through nvim-mcp exec_lua"})
read             "edited through nvim-mcp exec_lua\nline one\nline two\nline three"
exec_lua         vim.cmd.undo()
read             "unsaved first line\nline one\nline two\nline three"
after the loop: modified=1  /tmp/mc/eval-N1.txt: OK
```

The unsaved text was read, edited in place and undone, and the file on disk never changed. One fact the
shipping PR has to carry: **nvim-mcp has no purpose-built edit tool.** `read`, `list_buffers` and
`cursor_position` are first-class; writing goes through `exec_lua` (arbitrary Lua, as above) or
`lsp_apply_edit` (needs an attached LSP client). The 7.5 rule has to tell the agent that `exec_lua` with
`nvim_buf_set_lines` is the edit path, or it will fall back to writing the file.

This is criterion 5's hand check only. The recorded 10.8 loop (Claude in a herdr pane, and once from
Codex) belongs to PR 10a, the registering PR.

### Criterion 6, install: pass under a recorded amendment

```text
$ CARGO_HOME=/tmp/mc/cargo-home cargo install --git https://github.com/linw1995/nvim-mcp \
    --rev 0b5ace3 --root /tmp/mc/cargo --locked
   Installed package `nvim-mcp v0.7.2
     (https://github.com/linw1995/nvim-mcp?rev=0b5ace3#0b5ace3b)` (executable `nvim-mcp`)
exit=0
$ otool -L /tmp/mc/cargo/bin/nvim-mcp
	/System/Library/Frameworks/Security.framework/Versions/A/Security
	/System/Library/Frameworks/CoreFoundation.framework/Versions/A/CoreFoundation
	/usr/lib/libiconv.2.dylib
	/usr/lib/libSystem.B.dylib
```

No node runtime, a 21 MB static binary, `--locked` accepted (the repo ships `Cargo.lock`). The isolated
`CARGO_HOME` and `--root` kept the build out of `~/.cargo`; what was and was not verified about that tree
is in "What did not change" below, and it is metadata, not byte-identity.

**The criterion as written is not met, and this record amends it rather than failing it.** Spec 7.3
criterion 6 asks for "`cargo install` from the YAML-declared toolchain, wired by a `run_onchange` script
the way the pns and herdr plugins are built". The Rust toolchain is not YAML-declared:
`.chezmoidata/system_packages_autoinstall.yaml` has no rustup, rust or cargo entry (measured), and cargo
here is a rustup install.

The amendment, stated so it can be argued with: **criterion 6 reads "cargo reached by the guarded
`$HOME/.cargo/bin/cargo` path, wired by a `run_onchange` script the way the pns and uu builders are"**,
and on that wording it passes. The reason is that the criterion's own exemplars do not meet its own
clause. `run_onchange_after_58-build-pns-engine.sh.tmpl` and `run_onchange_after_59-build-uu.sh.tmpl`
both resolve `cargo_bin="$HOME/.cargo/bin/cargo"`, guard it on `[[ -x ]]`, and defer to the next apply
when it is absent, reporting `cargo not found at ~/.cargo/bin/cargo; build deferred to the next apply`
(measured). So "YAML-declared" describes no builder this repository has, and holding the candidate to it
would fail it for a property of how cargo is installed on this machine, which the candidate does not
control.

What the amendment costs, stated rather than hidden: **a bespoke rustup provisioning path that sits
outside the YAML package inventory.** Rust arrives through
`.chezmoiscripts/run_once_before_20-install-rustup.sh.tmpl`, which curls the upstream rustup installer,
rather than through `brew bundle` like every other tool in
`.chezmoidata/system_packages_autoinstall.yaml`. So the toolchain has its own installer, its own trust
decision and its own upgrade story, and none of that is visible where a reader looks for the package set.
That is the real price of the amendment, and it is one this repository is already paying for pns and uu.

**It does NOT cost a second apply**, and an earlier draft of this record said it did. The rustup script
is a `before` script, so it runs ahead of every `after` script in the same apply, and the builders
resolve the deterministic path rather than probing `PATH`:
`run_onchange_after_58-build-pns-engine.sh.tmpl` sets `cargo_bin="$HOME/.cargo/bin/cargo"` above the
comment "a fresh machine provisions rustup during THIS apply and `~/.cargo/bin` is not on the apply
shell's PATH yet". A fresh machine therefore builds on the first apply. A builder that guarded on
`command -v cargo` WOULD defer, for exactly the reason that comment gives, and it would also not be
implementing this amendment.

So the amendment carries a requirement: **PR 10a's install script must guard the direct path, never
`command -v cargo`.** As shipped on the `nvim-mcp-ship` branch it does:
`run_onchange_after_73-install-nvim-mcp.sh.tmpl` sets the same `cargo_bin="$HOME/.cargo/bin/cargo"` under
the same comment and defers on `[[ ! -x $cargo_bin ]]` (measured).

**This amendment is the operator's to reject.** Rejected, criterion 6 fails, the decision table's "5 or 6
fails" row applies, and the custom crate ships instead of the resolver row this record takes.

### Criterion 4, explicit socket: pass

N2 (`w13:p3`, pid 4277) ran in `noplugin` mode, so its only address was Neovim's default `stdpath("run")`
socket, which the server's discovery never lists. `--connect <that path>` reached it and nothing else:

```text
$ nvim-mcp --connect /var/folders/.../T/nvim.stephen/xAJthh/nvim.4277.0
nvim-connections://  [{"id":"fdc03c0","target":".../nvim.stephen/xAJthh/nvim.4277.0"}]
cursor_position      {"buffer_id":1,"buffer_name":"/private/tmp/mc/eval-N2.txt","col":0,"row":0,...}
get_targets          ["/tmp/nvim-mcp.%Users%stephen%.herdr%worktrees%dotfiles%nvim-eval-mcp.99074.sock",
                      "/tmp/nvim-mcp.%Users%stephen%workspaces%Ivy%webdavis%dotfiles.4607.sock"]
```

`get_targets` shows only the two plugin-registered sockets of other instances; N2's default socket is
invisible to discovery and reachable by pin. A wrapper can therefore choose, which is what the resolver
row depends on. `--connect` to a path that does not answer makes the server exit non-zero
(`Failed to connect to …`) rather than serve with nothing behind it, which is the fail-closed behavior
the resolver wants. A Neovim without the Lua plugin is tolerated with one warning
(`nvim-mcp Lua plugin is not installed, skipping dynamic tool discovery`); every built-in tool still
works.

### Criterion 1, discovery without `--listen`: fail as shipped

The plugin does not read the `stdpath("run")` socket Neovim 0.12 creates. It registers a second socket of
its own, named `<dir>/nvim-mcp.<git root with / replaced by %>.<pid>.sock` where `<dir>` is
`$XDG_RUNTIME_DIR`, else `$TMPDIR`, else `/tmp`; the server globs the same pattern in that dir and in
`/tmp`. On this machine the first launch with the plugin's defaults failed inside `init.lua`:

```text
E5113: Lua chunk: Vim:Failed to start server: invalid argument
        [C]: in function 'serverstart'
        .../nvim-mcp-src/lua/nvim-mcp/init.lua:144
```

The cause is the Unix socket path limit. `sizeof(sockaddr_un.sun_path)` is 104 on this host; a headless
`serverstart()` on a 104-byte path succeeds and on a 105-byte path fails with exactly that error. The
names the plugin generates for this operator's two repository roots are:

```text
123 bytes  $TMPDIR/nvim-mcp.%Users%stephen%.herdr%worktrees%dotfiles%nvim-eval-mcp.92516.sock
116 bytes  $TMPDIR/nvim-mcp.%Users%stephen%workspaces%Ivy%webdavis%dotfiles.92516.sock
 79 bytes  /tmp/nvim-mcp.%Users%stephen%.herdr%worktrees%dotfiles%nvim-eval-mcp.92516.sock
```

`$TMPDIR` above is the 49-byte `/var/folders/l5/8czjbbwx4pl1063q75rxypfr0000gn/T/`; the byte counts are
of the expanded path, which is what `serverstart()` binds.

So the shipped registration cannot bind for either real path here, and a lazy.nvim spec with `opts = {}`
would print that error on every start, failing acceptance item 10.1. The workaround is config-only and
uses a supported option: `setup({ pipe = "/tmp/nvim-mcp.<escaped root>.<pid>.sock" })` (79 bytes, and
`/tmp` is a directory the server already searches as its legacy fallback). Criteria 2 and 3 below were
run under that workaround, and the record says so. It is a workaround with a ceiling: the scheme encodes
the whole repository path in the socket name, so a worktree with a long branch name still overruns it,
and on Linux `$XDG_RUNTIME_DIR` (`/run/user/<uid>`) only buys about 35 bytes back.

Under the five-step design this failure is irrelevant: the registry records Neovim's own `v:servername`,
the `stdpath("run")` socket (about 70 bytes here), and the resolver connects to it by path. PR 10a should
not call `setup()` with defaults, and can skip the plugin spec entirely, since the plugin's only other
job is exporting custom Lua tools this program does not use.

### Criterion 2, workspace match: pass (under the criterion 1 workaround)

Two workspaces on two roots of the same git repository, the linked worktree and the main checkout, one
plugin-registered Neovim in each (N1 pid 99074 in `w13`; N4 pid 4607 in `w14`). The server ran inside
each workspace's own agent pane via `herdr pane run`:

```text
# from w13:p1, cwd /Users/stephen/.herdr/worktrees/dotfiles/nvim-eval-mcp
Found 1 Neovim instances for current project: /Users/stephen/.herdr/worktrees/dotfiles/nvim-eval-mcp
Auto-connected to project Neovim instance:
  /tmp/nvim-mcp.%Users%stephen%.herdr%worktrees%dotfiles%nvim-eval-mcp.99074.sock
nvim-connections://  [{"id":"3d97fa3","target":"...nvim-eval-mcp.99074.sock"}]

# from w14:p1, cwd /Users/stephen/workspaces/Ivy/webdavis/dotfiles
Found 1 Neovim instances for current project: /Users/stephen/workspaces/Ivy/webdavis/dotfiles
Auto-connected to project Neovim instance:
  /tmp/nvim-mcp.%Users%stephen%workspaces%Ivy%webdavis%dotfiles.4607.sock
nvim-connections://  [{"id":"21237f1","target":"...webdavis%dotfiles.4607.sock"}]
```

`git rev-parse --show-toplevel` returns the worktree path inside a linked worktree, so a worktree
workspace is its own root and matched only itself. No prompt, no cross-talk.

### Criterion 3, native choice within a workspace: fail on its own heuristic, moot under the design

Two unpinned, plugin-registered Neovims in `w13` (N1 in tab `t1`, N3 pid 9952 in tab `t2`), server
started from an agent pane in the worktree:

```text
Found 2 Neovim instances for current project: /Users/stephen/.herdr/worktrees/dotfiles/nvim-eval-mcp
Auto-connected to project Neovim instance: ...nvim-eval-mcp.99074.sock
Auto-connected to project Neovim instance: ...nvim-eval-mcp.9952.sock
nvim-connections://  [{"id":"7a99a07","target":"...9952.sock"},{"id":"3d97fa3","target":"...99074.sock"}]
```

The server connects to both and every tool takes a `connection_id`, so the agent must choose and the
server offers no basis for the choice: it knows cwd and git root, not panes or tabs. On the criterion's
own terms that is a fail. The plan asks the harder question, how much of this case is left once injection
covers both spawn directions, and the answer is in the next section.

## Criterion 3 under the five-step design

### How often the leftover case arises

The leftover case is two panes neither of which created the other: an agent started by hand in a
workspace that also holds two or more Neovims started by hand. Three facts bound it.

1. **Today, before task 23 lands `<leader>Cc`, every agent is started by hand**, so injection covers
   nothing yet and the resolver's topology step carries the whole load. That is temporary.
1. **One Neovim per workspace is the normal shape, and topology resolves it with no picker.** herdr
   anchors a workspace to one directory and one Neovim serves it; the second Neovim in a workspace is a
   deliberate act. **No count backs this, and the count this run took was worthless.** The census was
   `pgrep -f 'nvim --embed'`, which matches only a command line containing `nvim --embed`. It cannot
   match the `nvim --clean -u /tmp/mc/init.lua` instances this evaluation itself launched, and it cannot
   match an ordinary `nvim` the operator starts, so its zero is not evidence of zero. No complete process
   census was taken. The claim rests on the workspace model alone.
1. **The tab narrows it further.** Even with two Neovims in a workspace, the agent pane's tab usually
   holds only one of them, and step 2 picks that one deterministically (shown below). The picker only
   fires for two verified Neovims in the agent's own tab, which is the two-editors-side-by-side layout
   and nothing else.

So: **how often the leftover case arises is unmeasured, and this record forecasts nothing.** The three
facts above bound the case's shape, not its frequency, and nothing in this run measured a frequency. What
the design does claim is that the case is never answered wrongly, because ambiguity is enumerated for the
agent rather than guessed at; whether that enumeration reaches a client as a structured tool result is
itself unexercised, recorded below. nvim-mcp's criterion 3 failure does not by itself justify the crate
row, and this record does not take it on that failure.

### Step 1, injection: direction A exercised, direction B not

Direction A, Neovim spawns the agent pane and pins its own socket. Run from N4's pane in `w14`, with
`v:servername` read from the instance:

```text
$ herdr pane split w14:p2 --direction down --no-focus \
    --env NVIM_MCP_SOCKET=/var/folders/.../T/nvim.stephen/46nYQT/nvim.4607.0
spawned agent pane: w14:p3
$ herdr pane run w14:p3 'printf "%s\n" "$NVIM_MCP_SOCKET" > /tmp/mc/inject-a.txt'
the agent pane reads NVIM_MCP_SOCKET=/var/folders/.../T/nvim.stephen/46nYQT/nvim.4607.0
MATCH: equals N4's v:servername
```

Direction B, an agent pinning a Neovim it spawns, **was NOT exercised.** What the transcript below shows
is a Neovim bound to a socket path chosen outside any agent, by the `herdr pane split` command that
launched it:

```text
$ herdr pane split w14:p1 --direction down --no-focus --env NVIM_MCP_SOCKET=/tmp/mc/pinned.sock
spawned Neovim pane: w14:p4
$ herdr pane run w14:p4 \
    'nvim --clean -u /tmp/mc/init.lua --listen "$NVIM_MCP_SOCKET" /tmp/mc/eval-N5.txt'
registry: w14:p4 /tmp/mc/pinned.sock /Users/stephen/workspaces/Ivy/webdavis/dotfiles 8479
N5 answers on the pinned path: v:servername=/tmp/mc/pinned.sock pid=8479
$ nvim-mcp --connect /tmp/mc/pinned.sock
nvim-connections://  [{"id":"83ae453","target":"/tmp/mc/pinned.sock"}]
```

`herdr pane split --env KEY=VALUE` exists on 0.8.2, and its own help says it sets the variable "for the
launched process" (measured). That is the gap. The variable reaches the new pane's shell and the Neovim
it becomes, and it reaches nothing in the pane that ran the split. An already-running agent can therefore
set a socket for a child but cannot read that value back into its own environment, so its resolver never
sees the pin. Nothing in this run put a direction-B pin in front of an agent's resolver.

The registry step in PR 10a is what covers this case: Neovim writes `pane_id socket cwd pid` on
`VimEnter` and the resolver reads that file, so the handoff goes through something both sides can see
rather than through an environment the parent cannot receive. Direction A stands as exercised. Direction
B is owed a demonstration in PR 10a, through that registry or through a socket path both sides can derive
independently.

### Steps 2 to 5, the hand exercise

A 40-line bash script implemented steps 2 to 5 over the eval registry: read the agent pane's tab and
siblings with `herdr pane layout --pane <id>`, keep the siblings that have a registry line, ask each one
over RPC for its own pane id and pid, compare both to the registry, then resolve, enumerate or refuse. It
never calls `herdr pane current`, which answers the caller's own pane and would have matched nothing. The
identity expression is the one 7.3 gives:

```bash
nvim --server "$sock" --remote-expr 'join([getenv("HERDR_PANE_ID"), getpid()], " ")'
```

**Step 2 topology, and step 4 picker.** Agent pane `w13:p1` in tab `t1`, whose siblings N1 and N2 are
both registered and both alive:

```text
step 2 topology: agent w13:p1 is in tab w13:t1; sibling panes: w13:p1 w13:p3 w13:p2
step 3 identity: w13:p3 -> socket answers "w13:p3 4277", registry says "w13:p3 4277": MATCH, candidate
step 3 identity: w13:p2 -> socket answers "w13:p2 99074", registry says "w13:p2 99074": MATCH, candidate
step 4 PICKER (tool result, not an error): 2 verified candidates, pass --connect <socket> to choose:
  w13:p3 /var/folders/.../T/nvim.stephen/xAJthh/nvim.4277.0
         /Users/stephen/.herdr/worktrees/dotfiles/nvim-eval-mcp 4277
  w13:p2 /var/folders/.../T/nvim.stephen/sq8CWw/nvim.99074.0
         /Users/stephen/.herdr/worktrees/dotfiles/nvim-eval-mcp 99074
exit=4
```

**That "tool result, not an error" is the hand script's own wording, and it is not what happened.** What
happened is that a bash script printed two candidate lines and exited 4. The Model Context Protocol was
not involved: no `initialize`, no `notifications/initialized` and no `tools/call` ran against the
resolver, so nothing here shows what a client receives when two candidates tie, nor that a client reads
the tie as an enumeration rather than as a crashed server. **Protocol initialization and the picker's
transport are unexercised.** PR 10a owes an end-to-end client transcript of the two-candidate case before
the resolver is said to close criterion 3.

**Step 2 topology, lone sibling.** Agent pane `w13:p4` in tab `t2`, which holds only N3; the two Neovims
in `t1` are the same workspace and are correctly not candidates:

```text
step 2 topology: agent w13:p4 is in tab w13:t2; sibling panes: w13:p4 w13:p5
step 3 identity: w13:p5 -> socket answers "w13:p5 9952", registry says "w13:p5 9952": MATCH, candidate
resolved: w13:p5 /var/folders/.../T/nvim.stephen/Qdnf9m/nvim.9952.0
          /Users/stephen/.herdr/worktrees/dotfiles/nvim-eval-mcp 9952
exit=0
```

**Step 5, zero candidates.** Agent pane `w13:p6` in tab `t3`, which holds no Neovim:

```text
step 2 topology: agent w13:p6 is in tab w13:t3; sibling panes: w13:p6
step 5 REFUSE: no verified Neovim in tab w13:t3 of the agent pane w13:p6;
               launch the agent from Neovim (<leader>Cc) or export NVIM_MCP_SOCKET
exit=3
```

**Step 3, identity against a stale entry.** N2 (pid 4277, pane `w13:p3`) was killed with `SIGKILL`, so
its `VimLeavePre` never ran and its socket file stayed on disk. A different Neovim was then started in
pane `w13:p6` with `--listen` on that same path. Neovim 0.12.5 bound the dead instance's path without
complaint, so socket reuse is a real event and not a hypothetical:

```text
pid 4277 gone
stale socket file: srwxr-xr-x  1 stephen  staff  0 Sep  4 19:36
                   /var/folders/.../T/nvim.stephen/xAJthh/nvim.4277.0
N6 bound the reused path; registry now:
  2:w13:p3 /var/folders/.../T/nvim.stephen/xAJthh/nvim.4277.0
           /Users/stephen/.herdr/worktrees/dotfiles/nvim-eval-mcp 4277
  6:w13:p6 /var/folders/.../T/nvim.stephen/xAJthh/nvim.4277.0
           /Users/stephen/.herdr/worktrees/dotfiles/nvim-eval-mcp 10699
```

Resolving from `w13:p1` again, the socket the stale entry names answers, but as somebody else:

```text
step 2 topology: agent w13:p1 is in tab w13:t1; sibling panes: w13:p1 w13:p3 w13:p2
step 3 identity: w13:p3 -> socket answers "w13:p6 10699", registry says "w13:p3 4277":
                 MISMATCH, present but not the registered instance, entry pruned
step 3 identity: w13:p2 -> socket answers "w13:p2 99074", registry says "w13:p2 99074":
                 MATCH, candidate
resolved: w13:p2 /var/folders/.../T/nvim.stephen/sq8CWw/nvim.99074.0
          /Users/stephen/.herdr/worktrees/dotfiles/nvim-eval-mcp 99074
exit=0
```

Presence said yes, identity said no, and the presence-only answer would have handed the agent a Neovim in
a different tab holding a different file. This is the case the identity step exists for, and it turned
the earlier picker into a clean single resolution once the stale entry was pruned.

## Findings PR 10a must carry

1. **Do not call `require("nvim-mcp").setup()` with defaults on macOS.** It errors at startup for both of
   this operator's repository roots (criterion 1). Either omit the plugin spec, since the resolver pins
   by path and the registry uses `v:servername`, or pass `pipe` under a root short enough to keep the
   whole name at or under 104 bytes.
1. **The runtime-root fallback is a graveyard and must be filtered before any RPC.** The listing 7.3
   names, `${XDG_RUNTIME_DIR:-${TMPDIR}nvim.${USER}}/*/nvim.*.0`, returned 383 socket files on this
   machine with zero live Neovim behind any of them; Neovim leaves the file behind whenever it does not
   exit cleanly, and herdr pane closes are one such exit. A dead socket refuses in 17 ms, so a blind
   probe of all of them costs about 6.5 s; `kill -0` on the pid embedded in each filename filtered all
   383 in 17 ms and is the right pre-filter, with the identity check still run on whatever survives (a
   pid can be reused too).
1. **Editing is `exec_lua`.** The 7.5 rule and the resolver's tool description have to say that buffer
   edits go through `exec_lua` with `nvim_buf_set_lines` (undoable, in memory), because no `edit` tool
   exists and an agent that does not know this will write the file.
1. **The Codex table goes into `modify_private_config.toml`** as a stable `setValueAtPath` field beside
   the four existing `mcp_servers` entries (PR #306). Spec 7.3's sentence about a
   `private_config.toml.tmpl` predates that merge and is superseded by it.
1. **Pane ids are session-compact.** herdr renumbers pane ids as panes close, so the registry must hold
   the id Neovim itself read from `HERDR_PANE_ID` at start, and the identity check has to compare against
   what the instance reports now, which is what step 3 does.
1. **The resolver's protocol behavior is unexercised, and PR 10a owes the transcript.** Steps 2 to 5 were
   exercised by a bash script whose picker case is printed text and exit code 4. Nothing was run over the
   Model Context Protocol against the resolver, so PR 10a must record an end-to-end client transcript,
   initialization through the two-candidate tool result, before the resolver is said to close criterion
   3\.
1. **Direction-B injection is unexercised, and PR 10a owes that too.** `herdr pane split --env` sets the
   variable for the launched process only, so an agent cannot pin a Neovim it spawns and then read the
   pin itself. The registry is the intended cover and it needs a demonstration.
1. **Criterion 6 passes only under this record's amendment**, which swaps the spec's "YAML-declared
   toolchain" for the guarded `$HOME/.cargo/bin/cargo` path pns and uu already use. PR 10a carries that
   amendment forward or, if the operator rejects it, does not ship at all.

## What did not change

- Nothing was installed through chezmoi; `chezmoi apply` was not run.
- `~/.codex/config.toml` hashed the same before and after
  (`1e7a74e50c11b2e76a647c0858162d242775ee9121dc4de4470b7d7455575ddf`). `~/.claude.json` is rewritten by
  Claude Code as it runs, so its hash moved on its own twice in the first minute, before any evaluation
  work; the assertion that holds is that its `mcpServers` keys are still exactly `composio`,
  `cua-computer-use` and `workspace-mcp`, and neither file mentions `nvim-mcp` anywhere.
- `~/.cargo` was not used for the build: `CARGO_HOME=/tmp/mc/cargo-home` and `--root /tmp/mc/cargo`.
  **`~/.cargo` is not claimed byte-identical.** Three pieces of metadata were checked and all three were
  unchanged: `~/.cargo/bin` still held its 18 entries with no `nvim-mcp`; `.crates.toml` and
  `.crates2.json` kept their June mtimes; and `find ~/.cargo -newer <start marker>` listed nothing under
  `registry/` or `git/`. The binary set, the install metadata and the package and git caches are
  therefore what this record vouches for, and nothing wider. That same `find` did list
  `~/.cargo/.global-cache`, cargo's SQLite cache-usage tracker, at a 19:36:20 mtime, six minutes after
  the isolated install finished (19:30:04) and after the same file had already been written at 19:08,
  before this evaluation began. Another cargo user on the machine is the likely writer, but that is a
  reading of timestamps and this record does not claim to know who wrote it.
- Workspaces `w13` and `w14` were closed; the five evaluation Neovims were quit through their own
  sockets, no eval socket remained under `/tmp` or the runtime root, and the operator's workspace `wW`
  was focused before, during and after.
- `/tmp/mc` (the cargo root, the cargo home, the init, the driver and the outputs) was trashed.

## Budget

One working day, used in one sitting. No criterion is undecided, so the one-day extension the first row
of the 7.3 table allows is not taken.
