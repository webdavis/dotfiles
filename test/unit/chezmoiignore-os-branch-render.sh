#!/usr/bin/env bash
# chezmoiignore-os-branch-render.sh, two invariants of the OS-gated branch in
# .chezmoiignore, asserted against the RENDERED output rather than the template
# text, because the defect this file pins is invisible in the template.
#
# .chezmoiignore is a Go template. Its OS branch is wrapped in trim markers, and
# a trim marker eats the newline next to it. `{{- if ... -}}` therefore welds the
# line ABOVE the directive onto the first line of the branch, which is not a
# syntax error and not visible in the source: it silently produces one fused
# pattern where two were written. Measured with chezmoi 2.62.3 on the state this
# test was added to fix, forcing the branch on:
#
#     .local/share/herdr/**/target.config/yabai
#
# Two patterns became one that matches nothing. Both the herdr build-artifact
# suppression and the branch's own first entry were dead on that OS, and the
# entry BELOW the weld (`Library`) was one edit away from taking its place, which
# would have deployed the whole Library tree to a Linux HOME.
#
# The invariants:
#   1. No rendered line is a weld. Every non-empty line the template produces, in
#      EITHER branch, has to be a line somebody wrote: it must appear verbatim as
#      a literal line of the template. A fused line is by construction not one,
#      so this catches the whole class rather than the one instance, and it needs
#      no list of expected patterns to stay current. The render must also end in
#      a newline, since the closing trim marker eats that too.
#   2. Every path the OS branch suppresses is a path this repo actually delivers.
#      The branch exists to stop chezmoi deploying macOS-only target paths onto
#      Linux; a name in it with no source behind it suppresses nothing and only
#      reads as though some file were handled. `chezmoi managed` is the authority
#      on what this repo delivers, not the presence of a source file, because it
#      applies .chezmoiignore and every source-name attribute prefix. This arm is
#      decidable only from the OTHER OS (on the gated OS those very paths are
#      suppressed, so their absence proves nothing), which is why the host OS is
#      checked first and a run on the gated OS is a hard failure rather than a
#      skip.
#
# Glob patterns are exempt from invariant 2 on purpose: `.local/share/herdr/**/
# target` and `.agents/skills/*/.git` name build output and vendored subtrees
# that no source entry produces, so `chezmoi managed` cannot answer for them.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IGNORE_TEMPLATE="$REPO_ROOT/.chezmoiignore"
# The predicate that gates the OS branch, and the OS it gates. Substituting this
# exact literal for `true`/`false` is how both branches get rendered on one host:
# chezmoi offers no way to override .chezmoi.os, and the substitution preserves
# every trim marker, which is the thing under test.
OS_GATE_PREDICATE='eq .chezmoi.os "linux"'
GATED_OS='linux'
# Go template actions this test's model of the file can account for. The
# invariants assume the template emits only literal lines and control flow; a
# value action would emit text that is legitimately not a source line, so its
# appearance means this test no longer models the file.
CONTROL_FLOW_ACTION_PATTERN='^\{\{-?[[:space:]]*(if|else|end|range|with|block|define|template)[[:space:]}]'
# Shell/chezmoi glob metacharacters. A pattern carrying one names a set of paths
# rather than a path, so `chezmoi managed` cannot be asked about it.
GLOB_METACHARACTERS='*?['
# chezmoi entry types that put something at a target path. Both count as
# delivered here: invariant 2 asks whether the repo produces the path at all,
# not what shape it lands in.
CHEZMOI_DELIVERING_ENTRY_TYPES='files,dirs'
# Appended to a render so command substitution cannot silently eat the trailing
# newline this test asks about. Any non-newline byte works.
RENDER_SENTINEL='X'

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# Bug class 11: this suite runs inside a linked worktree, where git hands hooks
# an absolute GIT_DIR. Nothing below shells git, but chezmoi is invoked with an
# explicit --source and --destination so no inherited state can redirect it.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE
export GIT_PAGER=cat PAGER=cat

# Answers "does this line consist of nothing but a Go template action?". Pure,
# and the one place the shape of a directive-only line is defined.
is_template_directive_line() {
  local line="$1"
  [[ $line =~ ^[[:space:]]*\{\{.*\}\}[[:space:]]*$ ]]
}

# Answers "does this pattern name a set of paths rather than one path?". Pure.
is_glob_pattern() {
  local pattern="$1" metacharacter
  local i
  for ((i = 0; i < ${#GLOB_METACHARACTERS}; i++)); do
    metacharacter="${GLOB_METACHARACTERS:i:1}"
    [[ $pattern == *"$metacharacter"* ]] && return 0
  done
  return 1
}

# Answers "is this line worth asserting on?", i.e. neither blank nor a comment.
is_significant_ignore_line() {
  local line="$1"
  [[ -n ${line//[[:space:]]/} && $line != \#* ]]
}

# Renders .chezmoiignore with the OS gate forced to a literal, so both arms are
# reachable from one host. Every trim marker survives untouched, which is what
# makes the render faithful to what chezmoi produces on the gated OS.
#
# Emits the render WITH a trailing sentinel byte, which the caller strips. The
# sentinel is load-bearing, not a flourish: command substitution strips trailing
# newlines, and whether the render ends in one is exactly what invariant 1's
# second half asks. It has to survive the caller's substitution too, so it is
# stripped there rather than here, and this stays ONE chezmoi call per branch.
render_ignore_with_gate() {
  local gate_literal="$1"
  sed "s|$OS_GATE_PREDICATE|$gate_literal|" "$IGNORE_TEMPLATE" |
    chezmoi execute-template --no-tty || return 1
  printf '%s' "$RENDER_SENTINEL"
}

# Answers "does this render end with a newline?". A final `-}}` eats it, which
# leaves the last pattern unterminated. Pure.
render_ends_with_newline() {
  local rendered="$1"
  [[ $rendered == *$'\n' ]]
}

command -v chezmoi >/dev/null 2>&1 ||
  fail "chezmoi is not on PATH; neither branch of $IGNORE_TEMPLATE can be rendered"
[[ -f $IGNORE_TEMPLATE ]] || fail "missing template: $IGNORE_TEMPLATE"

# ---- predicate self-tests --------------------------------------------------
# The two pure predicates decide what the invariants below look at, so a
# predicate that answered the same way for everything would empty both without
# failing anything. Both directions are fixtured.
assert_predicate() {
  local predicate="$1" input="$2" expected="$3" actual=no
  "$predicate" "$input" && actual=yes
  [[ $actual == "$expected" ]] ||
    fail "$predicate answered '$actual' for '$input', expected '$expected'; the filter that decides what these invariants examine no longer discriminates"
}
assert_predicate is_template_directive_line '{{- if eq .chezmoi.os "linux" -}}' yes
assert_predicate is_template_directive_line '{{- end -}}' yes
assert_predicate is_template_directive_line '.config/yabai' no
assert_predicate is_template_directive_line '' no
assert_predicate is_glob_pattern '.local/share/herdr/**/target' yes
assert_predicate is_glob_pattern '.agents/skills/*/.git' yes
assert_predicate is_glob_pattern 'tmp.*' yes
assert_predicate is_glob_pattern 'Library' no
assert_predicate is_glob_pattern '.local/bin/rotate-logs.sh' no
assert_predicate is_significant_ignore_line 'Library' yes
assert_predicate is_significant_ignore_line '# a comment' no
assert_predicate is_significant_ignore_line '   ' no
assert_predicate render_ends_with_newline 'Library
' yes
assert_predicate render_ends_with_newline 'Library' no

# ---- the template must still be one this test models -----------------------
gate_occurrences="$(grep -Fc -- "$OS_GATE_PREDICATE" "$IGNORE_TEMPLATE" || true)"
((gate_occurrences == 1)) ||
  fail "$IGNORE_TEMPLATE contains $gate_occurrences occurrences of the OS gate '$OS_GATE_PREDICATE', expected exactly 1; this test forces both branches by substituting that literal and cannot do so unambiguously"

TEMPLATE_LITERAL_LINES=()
while IFS= read -r line; do
  if is_template_directive_line "$line"; then
    [[ $line =~ $CONTROL_FLOW_ACTION_PATTERN ]] ||
      fail "$IGNORE_TEMPLATE has a template action this test cannot model: '$line'. Invariant 1 assumes every rendered line was written as a literal line, which a value action breaks"
    continue
  fi
  [[ $line == *'{{'* ]] &&
    fail "$IGNORE_TEMPLATE mixes a template action into the literal line '$line'; invariant 1 assumes actions occupy whole lines"
  TEMPLATE_LITERAL_LINES+=("$line")
done <"$IGNORE_TEMPLATE"
((${#TEMPLATE_LITERAL_LINES[@]} > 0)) ||
  fail "$IGNORE_TEMPLATE yielded no literal lines; invariant 1 would admit anything"

# ---- 1: no rendered line is a weld, in either branch -----------------------
declare -A RENDERED_BRANCH=()
for gate_literal in true false; do
  rendered="$(render_ignore_with_gate "$gate_literal")" ||
    fail "rendering $IGNORE_TEMPLATE with the OS gate forced to '$gate_literal' failed; neither invariant can be decided"
  [[ $rendered == *"$RENDER_SENTINEL" ]] ||
    fail "the render of $IGNORE_TEMPLATE with the OS gate forced to '$gate_literal' lost its sentinel byte, so whether it ends in a newline can no longer be told apart from a substitution artefact"
  rendered="${rendered%"$RENDER_SENTINEL"}"
  [[ -n $rendered ]] ||
    fail "rendering $IGNORE_TEMPLATE with the OS gate forced to '$gate_literal' produced nothing"
  RENDERED_BRANCH["$gate_literal"]="$rendered"

  while IFS= read -r line; do
    [[ -n ${line//[[:space:]]/} ]] || continue
    line_is_literal=no
    for literal in "${TEMPLATE_LITERAL_LINES[@]}"; do
      [[ $line == "$literal" ]] && {
        line_is_literal=yes
        break
      }
    done
    [[ $line_is_literal == yes ]] ||
      fail "with the OS gate forced to '$gate_literal', $IGNORE_TEMPLATE renders the line '$line', which nobody wrote. A trim marker welded two source lines into one; the fused pattern matches nothing, so both of the patterns it came from are dead on that OS. Change the offending '{{-' or '-}}' so it stops eating the newline between a literal line and the directive"
  done <<<"$rendered"

  render_ends_with_newline "$rendered" ||
    fail "with the OS gate forced to '$gate_literal', $IGNORE_TEMPLATE renders without a trailing newline, so its last pattern is unterminated. A closing '-}}' is eating it"
done

# ---- 2: every path the OS branch suppresses is one this repo delivers ------
host_os="$(chezmoi execute-template --no-tty '{{ .chezmoi.os }}')" ||
  fail "could not ask chezmoi for the host OS; invariant 2 cannot be decided"
[[ $host_os != "$GATED_OS" ]] ||
  fail "this suite is running on $GATED_OS, the very OS the branch gates, so every path in it is suppressed here and 'chezmoi managed' cannot say whether the repo delivers it. Invariant 2 is decidable only from another OS"

# The branch's own lines are the difference between the two renders, so nothing
# has to be kept in step with the template by hand.
os_branch_lines="$(comm -13 \
  <(printf '%s\n' "${RENDERED_BRANCH[false]}" | sort -u) \
  <(printf '%s\n' "${RENDERED_BRANCH[true]}" | sort -u))"
[[ -n ${os_branch_lines//[[:space:]]/} ]] ||
  fail "forcing the OS gate on added no lines to the render of $IGNORE_TEMPLATE; either the branch is empty or the substitution did not take, and invariant 2 would pass vacuously"

managed_target_paths="$(chezmoi managed \
  --source "$REPO_ROOT" \
  --destination "$HOME" \
  --include="$CHEZMOI_DELIVERING_ENTRY_TYPES" \
  --path-style=relative)" ||
  fail "chezmoi managed failed against source $REPO_ROOT; invariant 2 cannot be decided"
[[ -n $managed_target_paths ]] ||
  fail "chezmoi managed listed no entries at all for source $REPO_ROOT; invariant 2 would fail every path for the wrong reason"

while IFS= read -r pattern; do
  is_significant_ignore_line "$pattern" || continue
  is_glob_pattern "$pattern" && continue
  printf '%s\n' "$managed_target_paths" | grep -Fxq -- "$pattern" ||
    fail "the $GATED_OS branch of $IGNORE_TEMPLATE suppresses '$pattern', but this repo delivers nothing to that target path (checked with 'chezmoi managed' on $host_os, where the branch is inactive). The entry suppresses nothing and only reads as though that path were handled; delete it, or add the source entry it was meant to gate"
done <<<"$os_branch_lines"

printf 'chezmoiignore-os-branch-render: OK (no welded lines in either branch, both end in a newline, every %s-gated path is one this repo delivers)\n' "$GATED_OS"
