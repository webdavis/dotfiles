#!/usr/bin/env bash
# macos-defaults-shape-guard.sh, defaults_records_declared_count must decide
# whether .macos.defaults IS a list of records, not whether it is LABELLED one.
#
# yq answers two different questions about a node and they are not the same
# question:
#   tag  reports the node's REPRESENTATION, which the document author writes.
#   kind reports what the node IS after parsing.
# An explicit `!!seq` tag sets the tag on a node of any shape, so a MAP and a
# SCALAR both answer `!!seq` while remaining a map and a scalar. A check that
# asks the tag therefore admits both.
#
# That is not a cosmetic difference. The shape check exists because the runner
# template reads the same file with Go's `range`, which walks a map in sorted KEY
# order, while this library's yq stream yields DOCUMENT order. Two readers, two
# orders, neither complaining, and order decides which write lands last when two
# records touch the same domain and key. On a `!!seq`-tagged map this library
# emitted both records and exited 0 while the runner template refused the file,
# which made this library the MORE PERMISSIVE of the two consumers: the same
# asymmetry test/integration/macos-defaults-shape-agreement.sh exists to end.
#
# What yq answers, measured directly with yq v4.53.3, and what each case pins:
#
#   .macos.defaults           kind    tag       verdict
#   - a, b (a real list)      seq     !!seq     accept  (case 1)
#   [] (an empty list)        seq     !!seq     accept  (case 2)
#   !!seq {a: ..., b: ...}    map     !!seq     REFUSE  (case 3)
#   {a: ..., b: ...}          map     !!map     REFUSE  (case 4)
#   !!seq "abc"               scalar  !!seq     REFUSE  (case 5)
#
# Cases 1 and 2 are the false-positive direction and carry the file: a guard that
# refuses everything passes cases 3 through 5 and nothing else. Case 2 also keeps
# the legitimate empty list working, which is a state an operator is entitled to
# declare.
#
# This file does NOT pin the absent-declaration refusal (a file with no
# .macos.defaults at all), which lives in
# test/unit/macos-defaults-declaration-guard.sh, nor the record count's digit
# ceiling, which lives in test/unit/macos-defaults-count-guard.sh.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIB="$REPO_ROOT/dot_local/bin/macos-defaults-lib.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# The expression this guard replaced. Every fixture the OLD check let through is
# put through it below, so each refusal case records WHY it was reachable rather
# than only asserting today's behaviour. A regression test whose fixture cannot
# reach the old behaviour pins nothing.
readonly SUPERSEDED_SHAPE_EXPRESSION='(.macos.defaults // []) | tag'

# require_superseded_check_satisfied <path>, fail unless this fixture SATISFIES
# the superseded tag check. Asserted on the fail-open fixtures, so a future edit
# to a fixture that quietly stops reproducing the hole is reported here instead
# of leaving a green test that guards nothing.
require_superseded_check_satisfied() { # <path>
  local path="$1" superseded_answer
  superseded_answer="$(yq eval -r "$SUPERSEDED_SHAPE_EXPRESSION" "$path")" ||
    fail "could not evaluate the superseded shape expression against $path"
  [[ $superseded_answer == '!!seq' ]] ||
    fail "fixture $path answers $superseded_answer to the superseded check, so it does not reproduce the hole this case exists to pin"
}

# refute_shape_accepted <path> <description>, require defaults_records_declared_count
# to refuse this file with status 2 and a message that names .macos.defaults.
#
# A helper rather than an inline `if ! ...`: under `set -e` an inverted command
# only decides the test in final position, so a bare negation inside a case body
# is a position lottery. It prints the refusal, so a caller can make further
# assertions about the message.
refute_shape_accepted() { # <path> <description>
  local path="$1" description="$2" status=0 output
  output="$(defaults_records_declared_count "$path" 2>&1)" || status=$?
  [[ $status -eq 2 ]] ||
    fail "$description must be refused with status 2, got $status (output: $output)"
  printf '%s' "$output" | grep -q '\.macos\.defaults' ||
    fail "$description was refused without naming .macos.defaults, so the operator cannot tell what to edit: $output"
  printf '%s' "$output"
}

# refute_output_contains <needle> <haystack> <message>, fail when <haystack>
# contains <needle>. A named helper rather than a bare `! grep` or a `grep -v`:
# under `set -e` an inverted command only decides the test in final position, and
# `grep -qv` answers "some LINE does not match", which is a different question
# and is satisfied by any multi-line output.
refute_output_contains() { # <needle> <haystack> <message>
  local needle="$1" haystack="$2" message="$3"
  if printf '%s' "$haystack" | grep -qF -- "$needle"; then
    fail "$message"
  fi
}

# require_shape_accepted <path> <expected-count> <description>, the
# false-positive direction: this file must be ACCEPTED and answer the count.
require_shape_accepted() { # <path> <expected-count> <description>
  local path="$1" expected_count="$2" description="$3" status=0 count
  count="$(defaults_records_declared_count "$path" 2>&1)" || status=$?
  [[ $status -eq 0 ]] ||
    fail "$description must be accepted, got status $status ($count)"
  [[ $count == "$expected_count" ]] ||
    fail "$description must count $expected_count record(s), got: $count"
}

# require_verdict <yq-answer> <expected-verdict> <description>, assert the
# classifier's answer for one yq shape answer, called directly.
#
# The whole-file cases above cannot stand in for these. The classifier is one of
# TWO barriers on some inputs, so a whole-file case stays green while a branch
# of the classifier is broken and the other barrier does the refusing. Feeding
# the classifier its input directly is what makes each branch fail on its own.
require_verdict() { # <yq-answer> <expected-verdict> <description>
  local shape_answer="$1" expected_verdict="$2" description="$3" verdict
  verdict="$(records_declaration_verdict "$shape_answer")"
  [[ $verdict == "$expected_verdict" ]] ||
    fail "$description must classify as $expected_verdict, got $verdict (input: $(printf '%q' "$shape_answer"))"
}

[[ -f $LIB ]] || fail "missing library: $LIB"
command -v yq >/dev/null 2>&1 || fail "yq is not on PATH; run inside the nix dev shell"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# shellcheck source=/dev/null
source "$LIB" >/dev/null 2>&1

# ---- 1: a real list is accepted, in its declared length --------------------
# The control. Without it every case below passes against a guard that refuses
# unconditionally, which would break every tracked setting on this machine.
cat >"$work/list.yaml" <<'EOF'
macos:
  defaults:
    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}
    - {domain: com.example.alpha, key: AKey, value: "1", type: bool, tier: enforce}
EOF
require_shape_accepted "$work/list.yaml" 2 "a real two-record list"

# ---- 2: an EMPTY list is accepted --------------------------------------------
# The legitimate way to say "track no records", and a state the guard must keep
# working. It is also the shape most likely to be broken by a fix aimed at
# refusing files that declare nothing.
cat >"$work/empty-list.yaml" <<'EOF'
macos:
  defaults: []
EOF
require_shape_accepted "$work/empty-list.yaml" 0 "an explicitly empty record list"

# ---- 3: a !!seq-tagged MAP is refused ----------------------------------------
# The finding. The tag is written by the document author and says nothing about
# what the node is; this fixture is a map that answers !!seq.
cat >"$work/tagged-map.yaml" <<'EOF'
macos:
  defaults: !!seq
    zebra: {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}
    alpha: {domain: com.example.alpha, key: AKey, value: "1", type: bool, tier: enforce}
EOF
require_superseded_check_satisfied "$work/tagged-map.yaml"
tagged_map_refusal="$(refute_shape_accepted "$work/tagged-map.yaml" "a !!seq-tagged map")"
# The refusal must name the ORDER divergence, not just the shape: that is the
# consequence an operator needs in order to understand why a map is not merely
# an unsupported spelling of a list.
printf '%s' "$tagged_map_refusal" | grep -qi 'order' ||
  fail "the refusal of a !!seq-tagged map does not say that the two readers would apply records in different orders: $tagged_map_refusal"

# ---- 4: a plain map is still refused -----------------------------------------
# The case the superseded check already caught, and a false-positive direction
# for the move from tag to kind: a fix that keys on the wrong field could refuse
# the tagged map while dropping this one.
cat >"$work/plain-map.yaml" <<'EOF'
macos:
  defaults:
    zebra: {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}
    alpha: {domain: com.example.alpha, key: AKey, value: "1", type: bool, tier: enforce}
EOF
plain_map_refusal="$(refute_shape_accepted "$work/plain-map.yaml" "a plain map")"
printf '%s' "$plain_map_refusal" | grep -qi 'order' ||
  fail "the refusal of a plain map does not say that the two readers would apply records in different orders: $plain_map_refusal"

# ---- 5: a !!seq-tagged SCALAR is refused BY THE SHAPE CHECK ------------------
# The other half of the same hole, and the one that reaches further: yq reports a
# scalar's length in BYTES, so a tagged scalar arrives at the record count with
# the tag check fully satisfied and publishes a count bounded only by the size of
# the file. Asserting WHICH check refused is what makes this a pin on the shape
# check rather than an observation that something said no.
cat >"$work/tagged-scalar.yaml" <<'EOF'
macos:
  defaults: !!seq "abc"
EOF
require_superseded_check_satisfied "$work/tagged-scalar.yaml"
tagged_scalar_refusal="$(refute_shape_accepted "$work/tagged-scalar.yaml" "a !!seq-tagged scalar")"
refute_output_contains 'unusable record count' "$tagged_scalar_refusal" \
  "a !!seq-tagged scalar was refused by the COUNT check, not the shape check, so this case pins the wrong guard: $tagged_scalar_refusal"

# ---- 6: the classifier itself, one yq answer at a time ----------------------
# records_declaration_verdict is pure, so its branches are pinned directly
# instead of through a data file. Two of them are not reachable through a file
# at all once the other checks in the function are doing their job, and one of
# them (the multi-line answer) is DOUBLE-guarded end to end: the record count
# refuses a multi-document file as well, so a whole-file case stays green while
# this branch is broken. That is exactly the pin a direct call provides and a
# fixture cannot.
require_verdict 'seq !!seq' list "a plain sequence"
require_verdict 'seq !!foo' list "a sequence wearing an unrecognized tag"
require_verdict 'map !!map' map "a plain mapping"
require_verdict 'map !!seq' map "a mapping wearing a !!seq tag"
require_verdict 'scalar !!seq' other "a scalar wearing a !!seq tag"
require_verdict 'scalar !!str' other "a plain string"
require_verdict $'seq !!seq\n---\nseq !!seq' other "yq's per-document answers for a multi-document file"
require_verdict '' other "an empty answer, which is what yq prints when .macos is not a mapping"
require_verdict 'seq' other "an answer missing its tag field"
require_verdict 'seq !!seq extra' other "an answer carrying an unexpected third field"

printf 'macos-defaults-shape-guard: OK (a real list and an empty list are accepted; a !!seq tag on a map or on a scalar no longer passes for a list; the classifier answers every yq shape directly)\n'
