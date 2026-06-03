> **Accuracy note (read with the Adversarial Review at the foot of this doc).** The body below is the
> synthesizer's draft. An adversarial verification pass narrowed two claims: (1) proof that visual-
> selection send works belongs to the plugin source (`init.lua` broadcasts `at_mentioned` with
> {filePath,lineStart,lineEnd}), **not** to issue #154 (which only proves the *connection* works under
> `provider="none"`); and (2) under `provider="none"` there is **no auto-launch** — a send only lands
> when the external Claude is *already connected*; otherwise the at-mention queues and is **cleared on a
> connection timeout**. The "40+ ACP agents" figure is unverified (use the named ~8-10 list). The verdict
> (retire delegate.lua as the *Claude* channel) stands; the free-text / non-Claude / scratch-buffer niche
> is the only genuine loss. Full corrections at the end.

# Retiring delegate.lua: The 2026 Best-Practice Workflow for Sending Neovim Context to a tmux-Resident Claude CLI

## 1. Executive Summary

**Verdict: retire the hand-rolled `delegate.lua` as your primary agent channel.** By June 2026, the
Neovim community has decisively abandoned `tmux send-keys` glue in favor of structured-protocol
transports, and your planned adoption of
[coder/claudecode.nvim](https://github.com/coder/claudecode.nvim) lands you on the consensus path with
zero additional architectural commitment.

The decisive finding is that `claudecode.nvim` fits your exact setup natively. It implements the same
WebSocket plus Model Context Protocol (MCP) "IDE integration" that Anthropic's official VS Code and
JetBrains extensions speak — Neovim runs a WebSocket *server*, and your externally-launched Claude CLI
connects *into* it via `claude --ide` or `/ide`. Setting
[`terminal.provider = "none"`](https://github.com/coder/claudecode.nvim/issues/154) disables all window
management while keeping the server and lock-file broadcast running, so your Claude stays in its tmux
pane exactly as today. `:ClaudeCodeSend` then delivers your visual selection as a structured
`at_mentioned` message carrying `{filePath, lineStart, lineEnd}` — no shell-escaping, no timing defers,
no string blasting. This eliminates every failure mode that makes `delegate.lua` brittle.

The community also converged on a subtler point: you send a *pointer* (file:line as an at-mention), not
the bytes. The agent reads the file itself with its own tools, which is faster and immune to stale paste.
Raw selection-paste is the genuinely deprecated pattern; structured selection-send is alive and actively
maintained.

Two viable architectures exist. The Agent Client Protocol (ACP) is cleaner in the abstract but requires
the editor to *own* the agent subprocess — an architectural pivot that abandons your tmux-pane model. For
your stated workflow, `claudecode.nvim`'s server model is the better-aligned tool. Keep at most a thin
generic sender (vim-slime) as a fallback for non-Claude REPLs.

## 2. Comparison Table

| Approach                                                         | Maturity           | Context it sends                                                                                                              | Fit for external-tmux-Claude                                                          | Key limitation                                                                                                                               |
| ---------------------------------------------------------------- | ------------------ | ----------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| **claudecode.nvim** (WebSocket+MCP IDE protocol)                 | Mature             | Selection + file:line as `at_mentioned`; auto `selection_changed`; on-demand diagnostics/open-editors/diff via MCP tool calls | **Native** — `provider="none"`, CLI connects via `claude --ide`                       | Protocol reverse-engineered from VS Code extension, not Anthropic-guaranteed; fire-and-forget broadcast (no "is Claude attached?" indicator) |
| **ACP** (CodeCompanion / agentic.nvim / avante)                  | Established        | Selection/file/buffer/diagnostics as typed JSON-RPC ContentBlocks                                                             | **Awkward** — editor must spawn+own agent over stdio; cannot attach to a running pane | Architectural mismatch; relocates Claude into a Neovim chat buffer; Claude runs via an npm adapter                                           |
| **Generic tmux senders** (vim-slime, slimux, toggleterm, Snacks) | Mature (vim-slime) | Raw selection/region text only — never file:line or metadata                                                                  | **Native** (vim-slime) / **No** (Snacks)                                              | Zero code-aware semantics; fire-and-forget; file:line must be hand-assembled                                                                 |
| **Generic Neovim MCP servers** (linw1995, bigcodegen, etc.)      | Emerging           | Agent-pulled buffers/diagnostics/LSP; only bigcodegen exposes visual selection                                                | **Awkward** — pull-model; can't push selection on a keystroke                         | Inverts the desired UX; redundant with claudecode.nvim for selection                                                                         |

## 3. Per-Approach Analysis

### claudecode.nvim (WebSocket + MCP IDE protocol)

This is the intended, first-class replacement for `delegate.lua`. The plugin stands up an RFC6455
WebSocket server inside Neovim and writes a discovery lock file to `~/.claude/ide/[port].lock` containing
the port, workspace, and a CSPRNG auth token
([PROTOCOL.md](https://github.com/coder/claudecode.nvim/blob/main/PROTOCOL.md)). Both `:ClaudeCodeSend`
and `:ClaudeCodeAdd` are implemented as `server.broadcast("at_mentioned", params)` calls
([init.lua](https://raw.githubusercontent.com/coder/claudecode.nvim/main/lua/claudecode/init.lua)).
Critically, the maintainer confirmed in [issue #154](https://github.com/coder/claudecode.nvim/issues/154)
that `terminal.provider = "none"` keeps the server and lock-file broadcast alive while doing no window
management, so an external tmux Claude that connects via `claude --ide` receives sends identically to a
first-party IDE ([README](https://github.com/coder/claudecode.nvim)). The DeepWiki architecture overview
documents its 10 MCP tools and at-mention queue ([DeepWiki](https://deepwiki.com/coder/claudecode.nvim)).
The main caveats: the protocol is reverse-engineered and could break on a CLI update (mitigated by fast
maintenance), and `snacks.nvim` is a declared dependency even under `provider="none"`.

### ACP (Agent Client Protocol)

ACP is an open, LSP-style JSON-RPC standard created by Zed and co-developed with JetBrains, with genuine
cross-editor momentum — 40+ registered agents and a registry launched January 2026
([Introduction](https://agentclientprotocol.com/get-started/introduction),
[JetBrains](https://blog.jetbrains.com/ai/2026/01/acp-agent-registry/)). A selection becomes a typed
`resource` ContentBlock ([Prompt Turn](https://agentclientprotocol.com/protocol/prompt-turn)). Three
Neovim clients exist:
[CodeCompanion.nvim](https://codecompanion.olimorris.dev/configuration/adapters-acp),
[avante.nvim](https://www.mintlify.com/yetone/avante.nvim/features/acp-support), and the purpose-built
[agentic.nvim](https://github.com/carlos-algms/agentic.nvim). But the architecture is the inverse of your
workflow: the editor spawns and owns the agent over stdio, and Claude runs through the
[@zed-industries/claude-code-acp](https://www.npmjs.com/package/@zed-industries/claude-code-acp) adapter
([Zed blog](https://zed.dev/blog/claude-code-via-acp)). There is no shipped remote/attach transport, so
you cannot bolt ACP onto a Claude already running in a pane. Adopting it means abandoning the tmux-pane
model for a Neovim chat buffer.

### Generic tmux senders

These are dumb pipes with no file:line semantics. [vim-slime](https://github.com/jpalardy/vim-slime) is
the strong one: mature (2.06k stars, last commit 2026-02-27 per
[GitHub metadata](https://api.github.com/repos/jpalardy/vim-slime)), it targets an external tmux pane by
socket plus `session:window.pane` and delivers via `load-buffer`/`paste-buffer` rather than keystroke
injection
([tmux target docs](https://github.com/jpalardy/vim-slime/blob/main/assets/doc/targets/tmux.md)). That
mechanism alone fixes `delegate.lua`'s escaping and timing pain.
[slimux.nvim](https://github.com/EvWilson/slimux.nvim) does the same but is a tiny char-escaping hobby
project — no reason to prefer it. [toggleterm.nvim](https://github.com/akinsho/toggleterm.nvim) sends to
an *embedded* Neovim terminal, not your tmux pane.
[Snacks.terminal](https://github.com/folke/snacks.nvim/blob/main/docs/terminal.md) has no send-region API
at all. None understands file:line, so even vim-slime needs a wrapper to prepend `@path:line` — at which
point claudecode.nvim's structured at-mention is simply the better tool.

### Generic Neovim MCP servers

Several real servers exist — [linw1995/nvim-mcp](https://github.com/linw1995/nvim-mcp) (Rust,
`--connect auto`, [tools reference](https://github.com/linw1995/nvim-mcp/blob/main/docs/tools.md)),
[bigcodegen/mcp-neovim-server](https://github.com/bigcodegen/mcp-neovim-server),
[laktek](https://github.com/laktek/nvim-mcp-server),
[georgeharker](https://github.com/georgeharker/mcp-diagnostics.nvim) — but they are the wrong shape for a
keystroke-initiated send. Claude Code's MCP integration is pull-model: the agent calls a tool only when
it decides to ([Claude Code MCP docs](https://code.claude.com/docs/en/agent-sdk/mcp)). So you cannot
proactively push the current selection; you would have to prompt the agent to fetch it, which is more
friction than today. Selection coverage is also thin — only bigcodegen exposes `vim_visual`, and it
targets Claude Desktop over a manual `--listen` socket. These servers shine only for *agent-initiated*
pulls of diagnostics/LSP/multi-buffer context, a complementary later add via tools like
[mcphub.nvim](https://github.com/ravitemer/mcphub.nvim).

## 4. Recommended 2026 Workflow

For your vanilla lazy.nvim plus Claude-CLI-in-tmux setup, the concrete path is:

1. **Add `coder/claudecode.nvim`** to your lazy.nvim spec with `terminal = { provider = "none" }`. This
   makes the plugin a pure WebSocket/lock-file server and does zero window management
   ([issue #154](https://github.com/coder/claudecode.nvim/issues/154)). Declare `snacks.nvim` as a
   dependency for now; verify on your installed version whether it can be dropped under the none
   provider.

1. **Keep Claude in its tmux pane.** In that pane, connect it once with `claude --ide`, or run `/ide`
   from inside an already-running session. This authenticates against the lock-file token and attaches
   the CLI to Neovim's server.

1. **Bind the gestures.** Map `:ClaudeCodeSend` (visual mode) to push the current selection as an
   `at_mentioned` with file:line, and `:ClaudeCodeAdd <file> [start] [end]` for whole-file or range
   mentions. The agent reads the file itself, so you are sending a pointer, not stale bytes
   ([community consensus](https://github.com/coder/claudecode.nvim),
   [LakTEK](https://www.laktek.com/using-claude-code-with-neovim)).

1. **Optional convenience — let the plugin host the pane.** If you would rather have the plugin spawn the
   CLI in a tmux split instead of managing it yourself,
   [mr55p-dev/claude-tmux.nvim](https://github.com/mr55p-dev/claude-tmux.nvim) is a third-party terminal
   provider that does exactly this. Treat it as a thin, lightly-maintained shim, not a guaranteed
   component.

1. **Enable buffer auto-reload** so Neovim picks up files the agent writes (uv `fs_event` watchers),
   closing the round-trip `delegate.lua` never handled
   ([xata.io](https://xata.io/blog/configuring-neovim-coding-agents)).

This preserves your exact "Claude lives in a tmux pane" model while replacing simulated keystrokes with a
typed, authenticated protocol identical to the official IDE extensions.

## 5. delegate.lua Verdict

**Retire it as the primary agent channel; optionally keep a thin generic sender as a fallback — do not
keep the normalized send-keys version.**

The brittleness you describe (timing defers, shell-escaping, no structured protocol) is intrinsic to the
`tmux send-keys` mechanism, and the community treats this pattern as deprecated
([2026 consensus](https://github.com/coder/claudecode.nvim),
[Memory Leaks](https://memoryleaks.blog/tech/2026/02/28/nvmegachad-acp.html)). `claudecode.nvim` solves
all three at the protocol layer for your exact workflow, so there is no remaining justification for a
hand-rolled normalized sender aimed at Claude.

The one nuance worth preserving: a generic dumb pipe still has value for *non-Claude* targets — arbitrary
scratch text, a shell, or a REPL in another pane. If you use that, do not maintain your own
`delegate.lua` for it; adopt [vim-slime](https://github.com/jpalardy/vim-slime) instead, which already
solves escaping and timing via `paste-buffer` delivery and is actively maintained. So: retire
`delegate.lua`, route Claude context through `claudecode.nvim`, and if you need a generic pane-pipe, let
vim-slime own that narrow role rather than re-hand-rolling it.

## 6. Limitations & Open Questions

- **Protocol stability.** claudecode.nvim's WebSocket/MCP protocol is reverse-engineered from Anthropic's
  VS Code extension and is not version-guaranteed; a CLI update could break it. The repo's fast
  maintenance cadence (security fixes landing on the research date) mitigates but does not eliminate this
  risk ([PROTOCOL.md](https://github.com/coder/claudecode.nvim/blob/main/PROTOCOL.md),
  [releases](https://github.com/coder/claudecode.nvim/releases)).
- **No attach indicator.** Sends are fire-and-forget broadcasts; if no client is connected (Neovim
  started after Claude, wrong directory, or `/ide` not re-run), the send silently goes nowhere. There is
  no built-in "is Claude attached?" signal in the external-pane case
  ([issue #154](https://github.com/coder/claudecode.nvim/issues/154)).
- **Connection ordering.** The Neovim server must be up before the CLI connects; a Claude launched in a
  directory whose lock file it cannot find requires a manual `/ide` re-run.
- **snacks.nvim dependency.** It is declared even under `provider="none"`; whether it can be fully
  omitted on the user's version is unverified and should be tested against the installed release.
- **Diff auto-accept gotcha.** Diffs Claude proposes open inside Neovim and auto-close on disconnect;
  auto-save plugins can auto-accept them unless excluded — a documented hazard
  ([README](https://github.com/coder/claudecode.nvim)).
- **Beta labeling / open issues.** Parts of the plugin remain self-labeled beta despite v0.3.0, with a
  sizeable open-issue count; verify current `main` before relying on the newest features.
- **ACP as a future hedge.** If cross-agent portability (Gemini, Codex, Copilot) later becomes a priority
  and the user is willing to move the agent into a Neovim chat buffer, ACP is the standards-track option
  — but it is an architectural change, not an upgrade, and conflicts with the claudecode.nvim model
  ([ACP clients](https://agentclientprotocol.com/get-started/clients)).
- **Local-install Claude PATH.** A `~/.claude/local/claude` install may not be on Neovim's PATH; less
  relevant under `provider="none"` since the user launches Claude themselves, but worth noting.

## 7. Bibliography

- [coder/claudecode.nvim — README and repository](https://github.com/coder/claudecode.nvim)
- [claudecode.nvim PROTOCOL.md — reverse-engineered WebSocket IDE protocol](https://github.com/coder/claudecode.nvim/blob/main/PROTOCOL.md)
- [Issue #154 — Integrating With External Claude Code (maintainer confirms provider='none')](https://github.com/coder/claudecode.nvim/issues/154)
- [claudecode.nvim Releases](https://github.com/coder/claudecode.nvim/releases)
- [lua/claudecode/init.lua — send/add as server.broadcast('at_mentioned', ...)](https://raw.githubusercontent.com/coder/claudecode.nvim/main/lua/claudecode/init.lua)
- [coder/claudecode.nvim architecture (DeepWiki)](https://deepwiki.com/coder/claudecode.nvim)
- [mr55p-dev/claude-tmux.nvim — tmux terminal provider for claudecode.nvim](https://github.com/mr55p-dev/claude-tmux.nvim)
- [Agent Client Protocol — Introduction](https://agentclientprotocol.com/get-started/introduction)
- [Agent Client Protocol — Prompt Turn / ContentBlocks](https://agentclientprotocol.com/protocol/prompt-turn)
- [Agent Client Protocol — Agents list](https://agentclientprotocol.com/get-started/agents)
- [Agent Client Protocol — Clients](https://agentclientprotocol.com/get-started/clients)
- [agentclientprotocol/agent-client-protocol (GitHub)](https://github.com/agentclientprotocol/agent-client-protocol)
- [Zed — Agent Client Protocol (ACP landing)](https://zed.dev/acp)
- [Zed — Neovim ACP client](https://zed.dev/acp/editor/neovim)
- [How the Community is Driving ACP Forward — Zed Blog](https://zed.dev/blog/acp-progress-report)
- [Claude Code: Now in Beta in Zed (via ACP) — Zed Blog](https://zed.dev/blog/claude-code-via-acp)
- [@zed-industries/claude-code-acp — npm](https://www.npmjs.com/package/@zed-industries/claude-code-acp)
- [Configuring ACP Adapters — CodeCompanion.nvim docs](https://codecompanion.olimorris.dev/configuration/adapters-acp)
- [CodeCompanion.nvim — Agent Client Protocol (ACP) support docs](https://codecompanion.olimorris.dev/agent-client-protocol)
- [New in v18.4.0 — Select ACP Models — CodeCompanion discussion](https://github.com/olimorris/codecompanion.nvim/discussions/2643)
- [CodeCompanion.nvim Discussion #2030 — New in v17.18.0: ACP](https://github.com/olimorris/codecompanion.nvim/discussions/2030)
- [avante.nvim — ACP Support docs](https://www.mintlify.com/yetone/avante.nvim/features/acp-support)
- [carlos-algms/agentic.nvim — dedicated ACP Neovim client](https://github.com/carlos-algms/agentic.nvim)
- [Agent Client Protocol (ACP) — JetBrains](https://www.jetbrains.com/acp/)
- [JetBrains AI Blog — ACP Agent Registry Is Live (Jan 2026)](https://blog.jetbrains.com/ai/2026/01/acp-agent-registry/)
- [agentclientprotocol/claude-agent-acp — use Claude Agent SDK from any ACP client](https://github.com/agentclientprotocol/claude-agent-acp)
- [jpalardy/vim-slime (GitHub README)](https://github.com/jpalardy/vim-slime)
- [vim-slime tmux target docs (load-buffer/paste-buffer, bracketed paste)](https://github.com/jpalardy/vim-slime/blob/main/assets/doc/targets/tmux.md)
- [EvWilson/slimux.nvim (GitHub README)](https://github.com/EvWilson/slimux.nvim)
- [akinsho/toggleterm.nvim (GitHub README)](https://github.com/akinsho/toggleterm.nvim)
- [folke/snacks.nvim — terminal module docs](https://github.com/folke/snacks.nvim/blob/main/docs/terminal.md)
- [GitHub API repo metadata — last-commit dates (vim-slime)](https://api.github.com/repos/jpalardy/vim-slime)
- [linw1995/nvim-mcp — Rust MCP server for Neovim](https://github.com/linw1995/nvim-mcp)
- [nvim-mcp tools reference (no selection tool)](https://github.com/linw1995/nvim-mcp/blob/main/docs/tools.md)
- [bigcodegen/mcp-neovim-server — 19 tools incl. vim_visual](https://github.com/bigcodegen/mcp-neovim-server)
- [laktek/nvim-mcp-server — buffer-focused MCP server](https://github.com/laktek/nvim-mcp-server)
- [georgeharker/mcp-diagnostics.nvim — diagnostics/LSP sharing via MCP](https://github.com/georgeharker/mcp-diagnostics.nvim)
- [Connect to external tools with MCP — Claude Code docs (pull-model)](https://code.claude.com/docs/en/agent-sdk/mcp)
- [ravitemer/mcphub.nvim — MCP client/hub for Neovim chat plugins](https://github.com/ravitemer/mcphub.nvim)
- [calebfroese/mcpserver.nvim — MCP server via Neovim MessagePack-RPC](https://github.com/calebfroese/mcpserver.nvim)
- [Memory Leaks — Bringing Claude Code Skills into Neovim via ACP](https://memoryleaks.blog/tech/2026/02/28/nvmegachad-acp.html)
- [xata.io — Tips for configuring Neovim for coding agents](https://xata.io/blog/configuring-neovim-coding-agents)
- [LakTEK — How I Use Claude Code with NeoVim](https://www.laktek.com/using-claude-code-with-neovim)
- [samir-roy/code-bridge.nvim — send selection/buffers/diffs/diagnostics](https://github.com/samir-roy/code-bridge.nvim)
- [Nick Liu — My Terminal Setup in 2026: Ghostty, tmux, and Neovim](https://www.nick-liu.com/posts/my-terminal-setup-2026/)

______________________________________________________________________

## Adversarial Review (verification pass)

CORRECTIONS AND CAVEATS THE FINAL REPORT MUST INCORPORATE

**(a) The `provider="none"` selection-send claim is partially overread — narrow it.**

- VERIFIED: maintainer ThomasK33 confirms `provider="none"` makes the plugin "a no-op in Neovim, and only
  spin up the WebSocket server," and you launch/connect Claude yourself via `claude --ide` or `/ide`. The
  original asker — whose setup is exactly "Claude Code in a tmux window next to Neovim" — replied "This
  did the trick" (https://github.com/coder/claudecode.nvim/issues/154, comments dated 2025-11-17 and
  2026-01-17).
- OVERREAD: neither the maintainer nor the asker states that `:ClaudeCodeSend` *selection-send* works
  end-to-end to the external pane. "Did the trick" confirms the *connection/integration*, not
  specifically that a visual-selection at-mention lands. The report asserts "receives sends identically
  to a first-party IDE" and "delivers your visual selection as a structured at_mentioned" as if issue
  #154 proves it; #154 does not. The send mechanism IS proven by source (init.lua
  `M._broadcast_at_mention` → `server.broadcast("at_mentioned", {filePath,lineStart,lineEnd})`), but the
  report should attribute that to init.lua, not to #154, and should say "selection-send works *when a
  client is connected*," verified by code, not by the issue thread.

**(b) The "no attach indicator / fire-and-forget" caveat is UNDERSTATED and partly wrong — correct it.**

- The plugin is NOT pure fire-and-forget. `M.send_at_mention` checks `M.is_claude_connected()` (init.lua
  line 292). If connected, it broadcasts immediately. If NOT connected, it *queues* the mention and
  starts a `connection_timeout` timer that **clears the queue and logs an error** if no client connects
  in time (init.lua lines 108-115: `"Connection timeout - clearing N queued @ mentions"`). So a mis-timed
  send is not silently lost into the void forever — it is buffered, then dropped with a log on timeout.
  The report's "send silently goes nowhere" is directionally right (no UI feedback) but mischaracterizes
  the mechanism.
- CRITICAL GOTCHA the report misses: in the disconnected branch, the plugin calls `terminal.open()` to
  *launch Claude* (init.lua line 311) — but under `provider="none"` the `none` provider's `open()` is an
  explicit no-op ("intentionally no-op," "performs zero UI actions and never manages terminals,"
  none.lua). So in the external-pane setup, a send issued while Claude is disconnected will NOT
  auto-launch anything; it just sits in the queue until you manually connect within `connection_timeout`
  or it's cleared. The report must state this: under `provider="none"`, sends only reliably work when the
  external CLI is *already connected*; there is no fallback launch. This is the real operational
  constraint, stronger than "connection ordering."

**(c) The "retire delegate.lua" verdict is too hasty for non-Claude / free-text use — broaden the
retained-capability list.**

- claudecode.nvim sends only `{filePath, lineStart, lineEnd}` pointers (verified, init.lua) — it cannot
  send a free-text prompt, an ad-hoc string, or arbitrary non-file selection content, and it speaks ONLY
  the Claude IDE protocol. The report acknowledges the "non-Claude REPL" gap and points to vim-slime,
  which is fair, but it under-weights three genuine losses if delegate.lua is fully retired:
  1. Sending to NON-Claude CLIs/agents (Gemini, Codex, aider, a plain shell) that don't implement the
     Claude IDE WebSocket protocol — neither claudecode.nvim nor (without owning the subprocess) ACP
     covers this from an external pane.
  1. Free-text / composed prompts (not a file pointer) — claudecode.nvim has no path for "send this typed
     instruction to the pane."
  1. Sending literal *bytes* of a non-file or unsaved/scratch buffer — at-mentions require a real
     filePath; an unsaved selection has no path to mention.
- vim-slime fills #1 and #3 (raw text to any tmux pane) but NOT the code-aware file:line semantics. So
  the honest framing is: retire delegate.lua *as the Claude channel*, but recognize vim-slime is a
  different tool (dumb raw-text pipe), not a superset — the report's "let vim-slime own that narrow role"
  is fine as long as it's clear vim-slime cannot do file:line at-mentions and claudecode.nvim cannot do
  free-text/non-Claude. Don't imply the combination is lossless.

**(d) ACP momentum: genuine, but one specific figure is unverified — fix the citation.**

- GENUINE (keep): JetBrains + Zed co-launched the ACP Agent Registry in January 2026
  (https://blog.jetbrains.com/ai/2026/01/acp-agent-registry/, https://zed.dev/blog/acp-registry);
  JetBrains is rolling ACP across its IDE suite (12M+ developers); registry agents include Claude Code,
  Codex CLI, Copilot CLI, OpenCode, Gemini CLI, Goose, Cline, Auggie. Momentum is not overstated.
- UNVERIFIED: the report's specific "40+ registered agents" count. Current sources name roughly 8-10
  agents and describe the registry as "actively growing" but give no "40+" total. Either drop the number
  or replace with the verifiable named list; do not cite #154 or the JetBrains post as supporting "40+"
  because they don't.

**Additional citation issues:**

- Issue #154 is cited for the strong claim "receives sends identically to a first-party IDE." The issue
  supports only "connection works via provider='none'." Re-anchor the selection-send mechanics to
  `lua/claudecode/init.lua` (broadcast of `at_mentioned`) and the connection/queue/timeout behavior, and
  re-anchor the no-op-launch caveat to `lua/claudecode/terminal/none.lua`.
- The report's claim that vim-slime is "actively maintained" is VERIFIED: 2,059 stars, last commit
  2026-02-27, not archived (GitHub API). The report's "2.06k stars / 2026-02-27" is accurate.

**Net:** the recommendation (adopt claudecode.nvim with `provider="none"`, keep Claude in tmux, retire
delegate.lua as the Claude channel) is sound and the connection model is verified. But the report must
(1) stop attributing selection-send proof to issue #154, (2) add the `provider="none"` "no auto-launch;
sends only land when already connected; queue clears on timeout" operational caveat, (3) correct the "40+
agents" figure, and (4) be honest that neither claudecode.nvim nor vim-slime covers free-text prompts or
non-Claude agents from an external pane, so something delegate.lua-shaped retains a niche there.
