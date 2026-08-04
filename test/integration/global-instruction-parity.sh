#!/usr/bin/env bash
# global-instruction-parity.sh: the global ruleset reaches Claude Code and Codex
# from ONE source. Before this mechanism existed, ~/.claude/CLAUDE.md was tracked
# and ~/.codex/AGENTS.md was a hand-edited untracked file, so the two drifted:
# the live Codex copy still named the wrong commit-message model and the wrong
# git TUI months after the Claude copy was corrected.
#
# The mechanism: .chezmoitemplates/global-agent-rules.md holds the shared block,
# and both targets pull it in with `includeTemplate` between a pair of
# `shared-rules` markers. This test renders BOTH targets with the real chezmoi
# and byte-compares what lands between those markers, so inlining the text in one
# target, editing one copy, or dropping a marker fails here instead of drifting
# on the live machine.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PARTIAL="$REPO_ROOT/.chezmoitemplates/global-agent-rules.md"
PARTIAL_NAME="global-agent-rules.md"
BEGIN_MARKER='<!-- shared-rules:begin -->'
END_MARKER='<!-- shared-rules:end -->'

# Target label -> source template. Both are private_ because ~/.claude and
# ~/.codex are 0700 on the live machine.
TARGETS=(
  "claude:$REPO_ROOT/private_dot_claude/CLAUDE.md.tmpl"
  "codex:$REPO_ROOT/private_dot_codex/AGENTS.md.tmpl"
)

# An anchor that must survive rendering. Without it, two targets that BOTH lost
# their markers would extract two empty blocks and compare equal, turning a
# broken mechanism into a pass.
ANCHOR='## Destructive action gates'

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

if ! command -v chezmoi >/dev/null 2>&1; then
  printf 'SKIP: chezmoi not on PATH; cannot render the global instruction targets\n'
  exit 0
fi

[[ -f $PARTIAL ]] || fail "missing shared partial: $PARTIAL"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
render_home="$work/home"
mkdir -p "$render_home"

# Everything between the markers, exclusive. A marker line itself is never part
# of the compared bytes, so the two targets may keep different surrounding text.
extract_shared_block() { # <rendered-file>
  awk -v b="$BEGIN_MARKER" -v e="$END_MARKER" '
    $0 == b { inside = 1; next }
    $0 == e { inside = 0 }
    inside
  ' "$1"
}

for entry in "${TARGETS[@]}"; do
  label="${entry%%:*}"
  template="${entry#*:}"

  [[ -f $template ]] || fail "missing global instruction template: $template"

  # Mechanism guard: the shared text must arrive by inclusion. Two identical
  # inlined copies would satisfy the byte-compare below while reintroducing the
  # exact drift this file exists to prevent.
  grep -qF "includeTemplate \"$PARTIAL_NAME\"" "$template" ||
    fail "$label does not includeTemplate \"$PARTIAL_NAME\"; the shared block must not be inlined ($template)"

  HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
    <"$template" >"$work/$label.rendered" ||
    fail "$label render failed ($template)"

  [[ -s "$work/$label.rendered" ]] || fail "$label rendered empty ($template)"
  grep -qF '# Global Rules' "$work/$label.rendered" ||
    fail "$label render is missing its '# Global Rules' heading ($template)"

  extract_shared_block "$work/$label.rendered" >"$work/$label.shared"
  [[ -s "$work/$label.shared" ]] ||
    fail "$label rendered no shared block; check the $BEGIN_MARKER / $END_MARKER markers ($template)"
  grep -qF "$ANCHOR" "$work/$label.shared" ||
    fail "$label shared block is missing the '$ANCHOR' anchor, so it is not the shared ruleset ($template)"
done

cmp -s "$work/claude.shared" "$work/codex.shared" || {
  printf 'FAIL: the shared rules block differs between the rendered global targets\n' >&2
  diff -u "$work/claude.shared" "$work/codex.shared" >&2 || true
  exit 1
}

printf 'PASS: global-instruction-parity.sh (%s renders byte-identical across both global targets)\n' \
  "$PARTIAL_NAME"
