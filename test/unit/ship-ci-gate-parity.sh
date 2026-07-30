#!/usr/bin/env bash
# ship-ci-gate-parity.sh, `just ship` must run exactly the gates CI runs.
#
# `ship` exists so a developer can rehearse CI before opening a PR, and its
# whole value is that a green ship predicts a green CI. It shipped promising
# "everything CI will run" while skipping two of CI's three gates: the flake
# check ran without --all-systems and the zizmor workflow audit was absent
# entirely. Nothing caught that, because one contract was written down twice,
# once in .github/workflows/lint.yml and once in the justfile.
#
# So this test reads BOTH lists from their real files and diffs them:
#
#   CI side:   the `run:` command of every step of every gate workflow, in file
#              order.
#   ship side: the command lines the `ship` recipe pulls in, dependencies first,
#              in the order just executes them, read from `just --dump` (which
#              parses the justfile and runs nothing).
#
# The assertion is EXACT ORDERED EQUALITY, not "ship covers CI". Extra local
# work would be harmless to run but would make a green ship mean something other
# than "CI would pass", which is the claim the recipe comment makes; teaching
# this test about a deliberate difference is the cheap way to keep that claim
# honest.
#
# Scope limit, stated plainly: only the workflows named in CI_GATE_WORKFLOWS are
# treated as gates. A NEW gate workflow must be added there, and the
# classification guard below fails on any workflow file that is in neither list
# so the decision cannot be skipped by accident.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
JUSTFILE="$REPO_ROOT/justfile"
WORKFLOW_DIRECTORY="$REPO_ROOT/.github/workflows"
# The recipe under test.
SHIP_RECIPE="ship"
# Workflows whose steps are code gates: `ship` must run all of them.
CI_GATE_WORKFLOWS=("lint.yml")
# Workflows that automate the repository rather than gate its code: `ship` must
# not run them (dependabot-automerge only merges an already-green bot PR).
NON_GATE_WORKFLOWS=("dependabot-automerge.yml")
# First character of a step command this reader can compare literally. A YAML
# block scalar (| or >) or a quoted scalar would need a real YAML parser, and
# comparing the raw text would silently compare the wrong string.
UNREADABLE_SCALAR_LEADERS="|>'\""

fail() {
  printf 'ship-ci-gate-parity: FAIL -- %s\n' "$*" >&2
  exit 1
}

# is_member <needle> <candidate...>
is_member() {
  local needle="$1" candidate
  shift
  for candidate in "$@"; do
    [[ $candidate == "$needle" ]] && return 0
  done
  return 1
}

# is_readable_inline_command <value>, true when the value is a non-empty plain
# scalar this reader can compare byte for byte.
is_readable_inline_command() {
  local value="$1"
  [[ -n $value ]] || return 1
  [[ $UNREADABLE_SCALAR_LEADERS != *"${value:0:1}"* ]]
}

# Every workflow file has to be classified as a gate or not a gate. A file in
# neither list is an unanswered question, not a pass.
assert_every_workflow_is_classified() {
  local path name
  for path in "$WORKFLOW_DIRECTORY"/*; do
    [[ -f $path ]] || continue
    name="${path##*/}"
    is_member "$name" "${CI_GATE_WORKFLOWS[@]}" && continue
    is_member "$name" "${NON_GATE_WORKFLOWS[@]}" && continue
    fail "$name is in neither CI_GATE_WORKFLOWS nor NON_GATE_WORKFLOWS; decide whether ship must run its steps and list it"
  done
}

# workflow_step_commands <workflow-file>, one command per line, in file order.
workflow_step_commands() {
  local file="$1" line value found=0
  [[ -f $file ]] || fail "missing gate workflow: $file"
  while IFS= read -r line || [[ -n $line ]]; do
    [[ $line =~ ^[[:space:]]*run:[[:space:]]*(.*)$ ]] || continue
    value="${BASH_REMATCH[1]}"
    # Strip trailing whitespace and any carriage return.
    value="${value%$'\r'}"
    value="${value%"${value##*[![:space:]]}"}"
    is_readable_inline_command "$value" ||
      fail "$file has a step command this reader cannot compare literally ('$value'); keep workflow commands as plain one-line scalars, or teach this reader YAML"
    printf '%s\n' "$value"
    found=1
  done <"$file"
  ((found == 1)) ||
    fail "$file declares no run: step, so the CI side of this comparison would be empty and everything would match"
}

# assert_recipe_is_comparable <recipe-name>, refuse recipe shapes whose body is
# not a list of shell commands.
assert_recipe_is_comparable() {
  local recipe="$1" shape
  shape="$(jq -r --arg recipe "$recipe" '
    if .recipes[$recipe] == null then "absent"
    elif .recipes[$recipe].shebang then "shebang"
    elif (.recipes[$recipe].parameters | length) > 0 then "parameterized"
    else "comparable" end' <<<"$just_dump")" ||
    fail "could not inspect recipe $recipe in $JUSTFILE"
  [[ $shape != absent ]] || fail "$JUSTFILE has no recipe named $recipe"
  [[ $shape != shebang ]] ||
    fail "recipe $recipe is a shebang recipe, so its body is a script rather than a list of commands; this comparison cannot read it"
  [[ $shape != parameterized ]] ||
    fail "recipe $recipe takes parameters, so what it runs depends on the call site; this comparison cannot read it"
}

# recipe_dependencies <recipe-name>, dependency names in execution order.
recipe_dependencies() {
  jq -r --arg recipe "$1" '.recipes[$recipe].dependencies[]?.recipe' <<<"$just_dump"
}

# recipe_body_lines <recipe-name>, the recipe's own command lines in order.
recipe_body_lines() {
  jq -r --arg recipe "$1" '
    .recipes[$recipe].body[]?
    | if any(.[]; type != "string") then
        error("recipe \($recipe) has a command line with an interpolation this reader cannot render literally")
      else join("") end' <<<"$just_dump"
}

# recipe_command_lines <recipe-name>, every command the recipe runs, in
# execution order: each dependency in full first, then the recipe body. just
# rejects a circular dependency at parse time, so this recursion terminates.
recipe_command_lines() {
  local recipe="$1" dependency_list body_lines dependency
  assert_recipe_is_comparable "$recipe"
  dependency_list="$(recipe_dependencies "$recipe")" ||
    fail "could not read the dependencies of recipe $recipe"
  while IFS= read -r dependency; do
    [[ -n $dependency ]] || continue
    recipe_command_lines "$dependency"
  done <<<"$dependency_list"
  body_lines="$(recipe_body_lines "$recipe")" ||
    fail "could not read the command lines of recipe $recipe"
  [[ -n $body_lines ]] && printf '%s\n' "$body_lines"
  return 0
}

report_lists_and_fail() {
  printf '\nCI gates (%s):\n' "${CI_GATE_WORKFLOWS[*]}" >&2
  printf '  %s\n' "${ci_commands[@]}" >&2
  printf 'just %s runs:\n' "$SHIP_RECIPE" >&2
  printf '  %s\n' "${ship_commands[@]}" >&2
  fail "$1"
}

command -v just >/dev/null 2>&1 ||
  fail "just is not on PATH; every supported way of running this suite goes through just, so this is a broken environment rather than a reason to skip"
command -v jq >/dev/null 2>&1 ||
  fail "jq is not on PATH; it is this repo's JSON tool everywhere, so this is a broken environment rather than a reason to skip"

just_dump="$(just --justfile "$JUSTFILE" --dump --dump-format json)" ||
  fail "just could not parse $JUSTFILE"

assert_every_workflow_is_classified

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT

# Both sides are written with a plain redirection (no pipe, no process
# substitution): a `fail` inside a subshell would exit only that subshell and
# leave a short list behind, and two short lists match each other perfectly.
: >"$workdir/ci"
for workflow in "${CI_GATE_WORKFLOWS[@]}"; do
  workflow_step_commands "$WORKFLOW_DIRECTORY/$workflow" >>"$workdir/ci"
done
recipe_command_lines "$SHIP_RECIPE" >"$workdir/ship"

mapfile -t ci_commands <"$workdir/ci"
mapfile -t ship_commands <"$workdir/ship"

((${#ci_commands[@]} > 0)) || fail "no CI gate commands were collected"
((${#ship_commands[@]} > 0)) ||
  fail "just $SHIP_RECIPE runs no commands at all, so it rehearses nothing"

if ! cmp -s "$workdir/ci" "$workdir/ship"; then
  report_lists_and_fail "just $SHIP_RECIPE does not run the same gates as CI, in the same order; a green ship would not predict a green CI"
fi

printf 'ship-ci-gate-parity: OK (just %s runs CI %s gates in order: %s)\n' \
  "$SHIP_RECIPE" "${#ci_commands[@]}" "${ci_commands[*]}"
