#!/usr/bin/env bash
# Claude Code PreToolUse hook (matcher: Bash): append one line per Bash
# invocation to ~/.claude/audit.log. Non-blocking; always exits 0 so the
# tool call is never held up.
#
# Hook input: JSON on stdin. For Bash tool invocations, the command is in
# .tool_input.command. Session info is at the top level.

input=$(cat)
cmd=$(printf '%s' "$input" | jq -r '.tool_input.command // ""' 2>/dev/null)
cwd=$(printf '%s' "$input" | jq -r '.cwd // ""' 2>/dev/null)

# macOS BSD date lacks -Is; prefer gdate (GNU coreutils; in manifest) with a
# portable fallback.
ts=$(gdate -Is 2>/dev/null || date -u +"%Y-%m-%dT%H:%M:%SZ")

mkdir -p "$HOME/.claude"
printf '%s\t%s\t%s\n' "$ts" "$cwd" "$cmd" >>"$HOME/.claude/audit.log"
exit 0
