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
#   ship side: the command lines the `ship` recipe pulls in, in the order just
#              executes them, read from `just --dump` (which parses the justfile
#              and runs nothing).
#
# The assertion is EXACT ORDERED EQUALITY, not "ship covers CI". Extra local
# work would be harmless to run but would make a green ship mean something other
# than "CI would pass", which is the claim the recipe comment makes; teaching
# this test about a deliberate difference is the cheap way to keep that claim
# honest.
#
# Comparing command TEXT only proves the two lists name the same commands. Three
# guards below refuse the shapes where matching text would stop meaning matching
# work, because in each of them the comparison would still pass:
#
#   1. A step that is not a `run:` command at all. A gate delivered as a `uses:`
#      action has no command line for `ship` to mirror, so only the setup actions
#      named in CI_SETUP_ACTIONS are accepted and anything else has to be
#      classified by hand.
#   2. A step this reader cannot model: `if:` makes the step conditional and
#      `continue-on-error:` makes its failure non-fatal, so its `run:` string
#      would still match while CI stopped gating on it.
#   3. A change to when the workflow runs at all. That scope is asserted as text
#      against GATE_TRIGGER_BLOCK_LINES; see the comment on that constant.
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
# Actions a gate workflow may use that are not themselves gates: they put the
# environment in place for the `run:` steps. Matched on the action name, with the
# version pin and any trailing comment removed.
CI_SETUP_ACTIONS=("actions/checkout" "NixOS/nix-installer-action")
# Step keys that change whether a command gates, without changing the command.
UNMODELLED_STEP_KEYS=("if" "continue-on-error")
# When CI runs, as the block nested under the workflow's top-level `on:` key,
# dedented. Three prose comments depend on exactly this scope and nothing else
# checks them: .githooks/pre-push ("CI runs on pull requests and on pushes to
# main, so pushing a topic branch that has no open pull request runs the suite
# NOWHERE") and CLAUDE.md in two places. Deleting `pull_request:` here would
# leave all three silently false. Compared as TEXT, so any edit fails: a property
# check would have to re-implement a YAML reader to assert less.
GATE_TRIGGER_BLOCK_LINES=(
  'push:'
  '  branches:'
  '    - "main"'
  'pull_request:'
)

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

# strip_trailing_whitespace <value>
strip_trailing_whitespace() {
  local value="${1%$'\r'}"
  printf '%s' "${value%"${value##*[![:space:]]}"}"
}

# action_reference_name <uses-value>, the action's name with its version pin and
# any trailing comment removed. The comment is dropped FIRST because a pin
# comment can itself contain an @ ("# main @ 2026-04-06").
action_reference_name() {
  local value="${1%%#*}"
  value="$(strip_trailing_whitespace "$value")"
  printf '%s' "${value%%@*}"
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

# assert_gate_workflow_steps_are_comparable <workflow-file>, refuse any step
# whose presence in (or absence from) the command list would misrepresent what
# CI gates on.
assert_gate_workflow_steps_are_comparable() {
  local file="$1" line value name key
  while IFS= read -r line || [[ -n $line ]]; do
    if [[ $line =~ ^[[:space:]]*(-[[:space:]]+)?uses:[[:space:]]*(.*)$ ]]; then
      value="$(strip_trailing_whitespace "${BASH_REMATCH[2]}")"
      name="$(action_reference_name "$value")"
      is_member "$name" "${CI_SETUP_ACTIONS[@]}" ||
        fail "$file uses the action '$name', which is neither a \`run:\` command this test can compare nor a listed setup action; if it is a gate, ship has no way to rehearse it, and if it is setup, add it to CI_SETUP_ACTIONS"
      continue
    fi
    for key in "${UNMODELLED_STEP_KEYS[@]}"; do
      if [[ $line =~ ^[[:space:]]*(-[[:space:]]+)?"$key":[[:space:]]*(.*)$ ]]; then
        fail "$file carries '$key:' ('$(strip_trailing_whitespace "$line")'); this test compares command text, so a conditional or non-fatal step would keep matching \`ship\` while CI stopped gating on it"
      fi
    done
  done <"$file"
}

# extract_gate_trigger_block <workflow-file> <outfile>, the lines nested under
# the workflow's top-level `on:` key, comments and blank lines dropped and the
# block's own indent removed. Writes to a file rather than stdout because a
# `fail` inside a command substitution would exit only its subshell and leave a
# short block behind, and two short blocks match each other perfectly.
extract_gate_trigger_block() {
  local file="$1" outfile="$2"
  local line indent content base=-1 seen=0 inside=0
  : >"$outfile"
  while IFS= read -r line || [[ -n $line ]]; do
    if [[ $line =~ ^(on|\"on\"|\'on\'):[[:space:]]*(.*)$ ]]; then
      content="$(strip_trailing_whitespace "${BASH_REMATCH[2]}")"
      [[ -z $content ]] ||
        fail "$file declares its triggers inline ('on: $content'); this test compares the block form"
      inside=1
      seen=1
      continue
    fi
    ((inside == 1)) || continue
    # A line at column 0 is the next top-level key, so the block has ended.
    if [[ $line =~ ^[^[:space:]] ]]; then
      inside=0
      continue
    fi
    [[ $line =~ ^([[:space:]]*)([^[:space:]].*)$ ]] || continue
    indent="${#BASH_REMATCH[1]}"
    content="$(strip_trailing_whitespace "${BASH_REMATCH[2]}")"
    if [[ $content == \#* ]]; then
      continue
    fi
    ((base >= 0)) || base="$indent"
    ((indent >= base)) ||
      fail "$file's trigger block is indented inconsistently at '$content'"
    printf '%*s%s\n' "$((indent - base))" '' "$content" >>"$outfile"
  done <"$file"
  ((seen == 1)) || fail "$file declares no top-level 'on:' trigger block"
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

# recipe_dependencies <recipe-name>, dependency names in declaration order.
recipe_dependencies() {
  jq -r --arg recipe "$1" '.recipes[$recipe].dependencies[]?.recipe' <<<"$just_dump"
}

# recipe_prior_dependency_count <recipe-name>, how many of those dependencies run
# BEFORE the recipe body. just runs a dependency declared after `&&` AFTER the
# body, and the dump records that split only as this count, not on the entries
# themselves, so reading the list without it would order a subsequent dependency
# as if it ran first.
recipe_prior_dependency_count() {
  jq -r --arg recipe "$1" '.recipes[$recipe].priors' <<<"$just_dump"
}

# recipe_body_lines <recipe-name>, the recipe's own command lines in order.
recipe_body_lines() {
  jq -r --arg recipe "$1" '
    .recipes[$recipe].body[]?
    | if any(.[]; type != "string") then
        error("recipe \($recipe) has a command line with an interpolation this reader cannot render literally")
      else join("") end' <<<"$just_dump"
}

# recipe_command_lines <recipe-name>, every command the recipe runs, in execution
# order: prior dependencies in full, then the recipe body, then any dependency
# declared after `&&`. just rejects a circular dependency at parse time, so this
# recursion terminates.
recipe_command_lines() {
  local recipe="$1" dependency_list prior_count body_lines dependency index
  local dependencies=()
  assert_recipe_is_comparable "$recipe"
  dependency_list="$(recipe_dependencies "$recipe")" ||
    fail "could not read the dependencies of recipe $recipe"
  prior_count="$(recipe_prior_dependency_count "$recipe")" ||
    fail "could not read the dependency ordering of recipe $recipe"
  [[ $prior_count =~ ^[0-9]+$ ]] ||
    fail "recipe $recipe reports a prior-dependency count this reader cannot use ('$prior_count')"
  while IFS= read -r dependency; do
    [[ -n $dependency ]] || continue
    dependencies+=("$dependency")
  done <<<"$dependency_list"
  ((prior_count <= ${#dependencies[@]})) ||
    fail "recipe $recipe reports $prior_count prior dependencies but declares only ${#dependencies[@]}"
  for ((index = 0; index < prior_count; index++)); do
    recipe_command_lines "${dependencies[index]}"
  done
  body_lines="$(recipe_body_lines "$recipe")" ||
    fail "could not read the command lines of recipe $recipe"
  if [[ -n $body_lines ]]; then
    printf '%s\n' "$body_lines"
  fi
  for ((index = prior_count; index < ${#dependencies[@]}; index++)); do
    recipe_command_lines "${dependencies[index]}"
  done
  return 0
}

# workflow_step_commands <workflow-file>, one command per line, in file order.
workflow_step_commands() {
  local file="$1" line value found=0
  [[ -f $file ]] || fail "missing gate workflow: $file"
  while IFS= read -r line || [[ -n $line ]]; do
    [[ $line =~ ^[[:space:]]*run:[[:space:]]*(.*)$ ]] || continue
    value="$(strip_trailing_whitespace "${BASH_REMATCH[1]}")"
    is_readable_inline_command "$value" ||
      fail "$file has a step command this reader cannot compare literally ('$value'); keep workflow commands as plain one-line scalars, or teach this reader YAML"
    printf '%s\n' "$value"
    found=1
  done <"$file"
  ((found == 1)) ||
    fail "$file declares no run: step, so the CI side of this comparison would be empty and everything would match"
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

printf '%s\n' "${GATE_TRIGGER_BLOCK_LINES[@]}" >"$workdir/trigger.expected"

# Both sides are written with a plain redirection (no pipe, no process
# substitution): a `fail` inside a subshell would exit only that subshell and
# leave a short list behind, and two short lists match each other perfectly.
: >"$workdir/ci"
for workflow in "${CI_GATE_WORKFLOWS[@]}"; do
  workflow_path="$WORKFLOW_DIRECTORY/$workflow"
  [[ -f $workflow_path ]] || fail "missing gate workflow: $workflow_path"
  assert_gate_workflow_steps_are_comparable "$workflow_path"
  extract_gate_trigger_block "$workflow_path" "$workdir/trigger"
  cmp -s "$workdir/trigger" "$workdir/trigger.expected" ||
    fail "$workflow no longer runs on exactly the events this repo documents (expected: ${GATE_TRIGGER_BLOCK_LINES[*]}); .githooks/pre-push and CLAUDE.md both tell the reader CI runs on pull requests and on pushes to main and that a topic branch without a pull request is therefore tested nowhere, so fix those comments in the same commit and then update GATE_TRIGGER_BLOCK_LINES"
  workflow_step_commands "$workflow_path" >>"$workdir/ci"
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
