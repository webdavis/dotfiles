# Agent Client Protocol, evaluated against the pinned claudecode.nvim setup

**Date:** 2026-09-15 · **Ledger task:** "Revisit Neovim's explicitly deferred conform.nvim/nvim-lint
migration and Agent Client Protocol option only after a separate decision, using the v4 design"
(`docs/remaining-work.md:3746`) · **Subject:** the Agent Client Protocol specification and its Neovim
clients, compared against `coder/claudecode.nvim` pinned at commit `2390c6e4` in
`dot_config/nvim/lua/plugins/claudecode.lua`

This pull request adds this document and nothing else. No plugin was declared through chezmoi, no Lua was
touched, and no `chezmoi apply` was run. The ledger task gates this revisit on a separate decision; this
document does not make that decision, it gives the operator the facts to make it.

______________________________________________________________________

## 1. What Agent Client Protocol is

Agent Client Protocol is a JSON-RPC protocol that standardizes communication between an editor and a
coding agent, so one editor can drive any agent that speaks the protocol and one agent can be driven by
any editor that speaks it. It originated at Zed Industries, the same shop that mirrors Model Context
Protocol's client/server split onto the editor/agent relationship. The specification, its Rust and
TypeScript crates, and the reference documentation now live under a dedicated GitHub organization,
`agentclientprotocol/agent-client-protocol` (Apache-2.0, 4,243 stars, pushed 2026-09-14, confirmed via
`gh-axi api repos/agentclientprotocol/agent-client-protocol`).

The protocol is versioned and calls itself stable: the documentation states "the current stable ACP
protocol version is `1`" (agentclientprotocol.com/overview/introduction), and the repository tags real
releases, the newest read being `v1.7.0` (Rust crate, published 2026-08-20) alongside a parallel
`schema-v1.21.0` tag. Those are release tags on a maintained package, the exact thing the pinned commit
in `claudecode.lua` cannot offer, and the exact gap that file's own comment names ("the pin is a commit,
not a tag"). One area is still explicitly unfinished: the docs describe full support for remote (as
opposed to local, same-machine) agents as "a work in progress."

Sources fetched: `agentclientprotocol.com/overview/introduction` (what it is, publisher framing, version
claim), `github.com/agentclientprotocol/agent-client-protocol` via WebFetch and `gh-axi api` (org,
license, star count, push date, release tags).

## 2. Whether Claude Code speaks it

This is the load-bearing question and the answer is no, not directly.
`agentclientprotocol.com/get-started/agents` lists "Claude Agent" as requiring "Zed's SDK adapter," and
does not list Claude Code, the terminal CLI itself, as an agent at all. The adapter is a separate
package, `@agentclientprotocol/claude-agent-acp` (npm, source at
`github.com/agentclientprotocol/claude-agent-acp`, Apache-2.0, 2,530 stars, pushed 2026-09-15, 174 open
issues). It wraps the Claude Agent SDK, not the `claude` binary the operator runs interactively in a
herdr pane: the adapter is its own process that an editor plugin spawns, authenticates, and drives over
the protocol. It originated as `zed-industries/claude-agent-acp` and was later handed to the
`agentclientprotocol` organization.

That distinction matters for every feature comparison below. The claudecode.nvim setup in this config
connects Neovim, as an IDE, to the same `claude` process already running interactively in the herdr pane
(question 4 in the spec's own comment: Neovim starts the WebSocket server and writes the lock file, then
the operator's existing session runs `claude --ide` or types `/ide`). The Agent Client Protocol adapter
is a different arrangement: the editor plugin launches its own Claude Agent SDK subprocess, a second mind
with no connection to whatever is already running in the pane.

Sources fetched: `agentclientprotocol.com/get-started/agents` (adapter requirement, Claude Code absent
from the agent list), `github.com/agentclientprotocol/claude-agent-acp` via WebFetch and `gh-axi api`
(publisher, push date, star count, what it wraps), a WebSearch pass confirming the Zed origin and the
`agentclientprotocol` org's current ownership.

## 3. Neovim clients

The official clients page lists four Neovim plugins with Agent Client Protocol support: CodeCompanion,
`carlos-algms/agentic.nvim`, `yetone/avante.nvim`, and `hermes.nvim`. avante.nvim is already evaluated
and rejected in `docs/research/2026-09-avante-nvim-evaluation.md`; that document's verdict is reused
rather than repeated here (see section 4). The dedicated, Agent Client Protocol-only option is
`agentic.nvim`: a chat interface for AI agents in Neovim, with Claude (via `claude-agent-acp`), Gemini,
Codex, OpenCode, and Cursor Agent as built-in providers.

Its health, read from `gh-axi api repos/carlos-algms/agentic.nvim`: 620 stars, 260 commits, last push
2026-08-23 (about three weeks before this document), not archived, 16 open issues, no GitHub release and
only one pre-refactor tag (`pre-split-session-keyed`). It is not the kind of project a person would need
to write from scratch; it is a maintained plugin the operator would install, but it ships no tagged
release to pin, the same gap the pinned commit in `claudecode.lua` already has, only on the client side
this time instead of the agent side.

Its own architecture confirms the section 2 finding rather than working around it: the README describes
"complete isolation of sessions across Neovim tab pages," meaning each chat session is a Neovim-spawned
agent process, not a connection to something already running in a terminal.

Sources fetched: `github.com/carlos-algms/agentic.nvim` via WebFetch and `gh-axi api` (description,
providers, star count, push date, release and tag state), `agentclientprotocol.com/get-started/clients`
via WebSearch summary (the four-plugin list).

## 4. What the operator would gain

A documented, versioned wire protocol instead of a reverse-engineered one: the current pin exists because
nobody published the WebSocket handshake claudecode.nvim speaks, while Agent Client Protocol publishes
its schema, its stability guarantee, and its changelog. A tagged release to pin, on both sides, once one
exists for the Neovim client: `claude-agent-acp` already tags releases, `agentic.nvim` does not yet.
Multiple agents through one client: `agentic.nvim` treats Claude, Gemini, Codex, and others as
interchangeable providers behind the same chat interface, which claudecode.nvim does not attempt. And
upstream momentum: the protocol's own organization is active, pushed the day before this document, and
JetBrains has reportedly committed to adopting it across its IDE suite (from training, not verified: that
JetBrains detail came back in a WebSearch summary, not a primary source fetched directly, so treat it as
unconfirmed).

## 5. What the operator would lose

Every row below is a real feature of the pinned setup, read from
`dot_config/nvim/lua/plugins/claudecode.lua` in full.

| Feature in `claudecode.lua` today                                             | What it does                                                                                                              | Covered by the Agent Client Protocol path?                                                                                                          |
| ----------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| WebSocket IDE connection to the pane's own `claude` process                   | Neovim writes the lock file, the operator's already-running interactive `claude --ide` (or `/ide`) session attaches to it | No. The adapter spawns its own Claude Agent SDK subprocess; it does not attach to the terminal session already running in the herdr pane            |
| One continuous session, one context                                           | The same agent process that had the conversation also sees and edits the buffer; nothing forked                           | No. The Agent Client Protocol client's session is a second, separate agent instance with its own context                                            |
| `ClaudeCodeDiffAccept` / `ClaudeCodeDiffDeny` (`<leader>Cy` / `<leader>Cn`)   | Review and accept or deny a proposed diff in the editor                                                                   | Yes. Agent Client Protocol defines tool-permission presentation with editable choices, which `agentic.nvim` exposes as its own diff review          |
| `ClaudeCodeAdd %` (`<leader>Ca`)                                              | Add the current file to Claude's context                                                                                  | Yes. Agent Client Protocol clients pass file context and mentions the same way                                                                      |
| `ClaudeCodeSend` (`<leader>Cs`, visual mode)                                  | Send a visual selection into Claude's context                                                                             | Yes, equivalent context-passing exists in the Agent Client Protocol clients read                                                                    |
| `ClaudeCodeStatus`                                                            | Report connection state                                                                                                   | Yes, in substance; an Agent Client Protocol client tracks its own session state                                                                     |
| `<leader>Cp`, the herdr seam (`custom_api/herdr.send_selection_or_paragraph`) | Send raw text to the agent's herdr pane                                                                                   | Not applicable as designed. It targets the pane-based `claude` session; that session is exactly what the Agent Client Protocol path bypasses        |
| `<leader>Cc`, launch-or-attach (`custom_api/herdr.launch_or_attach`)          | Prompt `/ide` at the already-running pane agent, or start one                                                             | Not applicable. It is convenience code for attaching to the pane session the Agent Client Protocol path does not use                                |
| pns hooks and Discord/phone/banner notification wiring on the CLI session     | Fires from the `claude` CLI's own hook configuration in `~/.claude/settings.json`                                         | Unverified. Whether the Claude Agent SDK the adapter wraps fires the same harness hooks was not checked in this pass; treat as open, not as covered |

The pattern in that table is the same one `docs/research/2026-09-avante-nvim-evaluation.md` already named
and rejected for avante: the pinned setup puts the agent in the herdr pane and has it drive the live
Neovim buffer, while every Agent Client Protocol Neovim client available today puts the agent inside
Neovim itself, spawned by the plugin, disconnected from whatever the operator is already running in a
terminal. Agent Client Protocol does not change that shape; it is a different, better-specified version
of the same inverted design spec 7.6 already deferred.

## 6. The 2026-09-15 WebSocket warnings

The operator saw claudecode.nvim WebSocket handshake warnings and a `handle is already closing` error on
2026-09-15. The cause was not established. An earlier theory blaming headless Neovim runs is disproved:
`VeryLazy` never fires in `nvim --headless`, because lazy.nvim defers it to a `UIEnter` autocommand that
a headless run never triggers, so the plugin never loads there and cannot be the source.

Whether Agent Client Protocol would make this class of error easier to diagnose, in principle, yes: it
publishes a schema and an error vocabulary, where claudecode.nvim's handshake is undocumented and read
from source. That is a property of protocols with a specification, not a claim about this specific
incident. It would not have fixed this error, and nothing in this pass identifies what did cause it.

## 7. Recommendation

Do not adopt Agent Client Protocol as a replacement for claudecode.nvim in this configuration. Claude
Code the CLI does not speak the protocol; the only path in is the `claude-agent-acp` adapter, which
launches a separate Claude Agent SDK process instead of attaching to the same `claude` session already
running in the herdr pane, so every feature built around that one continuous session (the herdr seam, the
launch-or-attach convenience, and the session continuity itself) has no Agent Client Protocol equivalent
today. The protocol's documented schema and tagged releases are real improvements over a
reverse-engineered commit pin, and worth another look if this config's own agent-lane design changes, but
they do not answer the question this pull request was asked to research: whether adopting Agent Client
Protocol retires the pin. It does not, because it requires giving up the pane-attached session the pin
exists to protect.

## 8. Questions for the operator

- Keep the claudecode.nvim commit pin as is: yes or no.
- Revisit this once the agent-lane design itself changes (spec 7.6's deferred chat-buffer model
  reconsidered): yes or no.
- Worth separately checking whether Claude Code's own harness hooks fire through the Claude Agent SDK the
  adapter wraps, independent of any Neovim decision: yes or no.
- Track `agentic.nvim` for a tagged release before any future re-evaluation: yes or no.
