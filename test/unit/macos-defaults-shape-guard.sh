#!/usr/bin/env bash
# macos-defaults-shape-guard.sh, defaults_records_declared_count must decide
# whether .macos.defaults IS a list of records, not whether it is LABELLED one,
# and not whether it merely PARSES as one.
#
# yq answers two different questions about a node and NEITHER one answers this
# guard's question alone:
#   tag  reports the node's REPRESENTATION, which the document author writes.
#   kind reports what the node IS after parsing.
#
# Each is defeated by exactly the input the other catches, which is why this file
# pins the CONJUNCTION rather than either half:
#
#   A LYING TAG defeats `tag` alone. An explicit `!!seq` sets the tag on a node
#   of any shape, so a MAP and a SCALAR both answer `!!seq` while remaining a map
#   and a scalar. Cases 3 and 5.
#
#   A TRUTHFUL SHAPE wearing a wrong tag defeats `kind` alone. A real sequence
#   tagged `!!str` is still a sequence, so a kind check reads `seq` and accepts a
#   file the runner template refuses with a parse error. Case 6, the mirror hole,
#   and the reason this file exists in its current form: a kind-only check
#   shipped and made this library the MORE PERMISSIVE reader on eight measured
#   tags, which is the exact asymmetry it had just closed in the other direction.
#
# That is not a cosmetic difference either way. The shape check exists because
# the runner template reads the same file with Go's `range`, which walks a map in
# sorted KEY order, while this library's yq stream yields DOCUMENT order, and
# because the template's Go YAML reader refuses several tags outright that yq
# reads happily. Order decides which write lands last when two records touch the
# same domain and key, and a file only one reader will read means the settings
# silently stop being applied.
# test/integration/macos-defaults-shape-agreement.sh holds the two readers
# against each other on one fixture from each half.
#
# What yq answers, measured directly with yq v4.53.3, and what each case pins:
#
#   .macos.defaults           kind    tag       verdict
#   - a, b (a real list)      seq     !!seq     accept  (case 1)
#   [] (an empty list)        seq     !!seq     accept  (case 2)
#   !!seq {a: ..., b: ...}    map     !!seq     REFUSE  (case 3)
#   {a: ..., b: ...}          map     !!map     REFUSE  (case 4)
#   !!seq "abc"               scalar  !!seq     REFUSE  (case 5)
#   !!str - a, b              seq     !!str     REFUSE  (case 6, one per tag)
#
# Cases 1 and 2 are the false-positive direction and carry the file: a guard that
# refuses everything passes cases 3 through 6 and nothing else. Case 2 also keeps
# the legitimate empty list working, which is a state an operator is entitled to
# declare.
#
# This file does NOT pin the absent-declaration refusal (a file with no
# .macos.defaults at all) or the byte-order-mark predicate, which live in
# test/unit/macos-defaults-declaration-guard.sh, nor the record count's digit
# ceiling and its call site, which live in test/unit/macos-defaults-count-guard.sh.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIB="$REPO_ROOT/dot_local/bin/macos-defaults-lib.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# The two expressions this guard replaced, one per half of the hole. Every
# fixture is put through the check it defeats, so each refusal case records WHY
# it was reachable rather than only asserting today's behaviour. A regression
# test whose fixture cannot reach the old behaviour pins nothing.
readonly SUPERSEDED_TAG_ONLY_EXPRESSION='(.macos.defaults // []) | tag'
readonly SUPERSEDED_KIND_ONLY_EXPRESSION='.macos.defaults | kind'

# The tags a real SEQUENCE can wear that are not `!!seq`. Measured against
# yq v4.53.3: every one of these answers kind `seq`, so every one of them
# satisfies a kind-only check, and against chezmoi v2.71.1: the first eight are
# REFUSED by the runner template with a YAML parse error, so on those a
# kind-only check left this library reading records from a file `chezmoi apply`
# cannot read at all.
#
# The last six the template RENDERS, so refusing them makes this library the
# stricter reader. They are in the table deliberately: the accept set is the
# conjunction and nothing else, and a tag space is open (a document may write any
# application-specific tag), so the guard has to be closed by construction rather
# than by listing the spellings anybody happened to try.
#
# `!!null` is deliberately absent: on a sequence yq answers `scalar !!null`
# rather than `seq !!null` (measured), so that file is the ABSENT case and is
# pinned in test/unit/macos-defaults-declaration-guard.sh.
readonly MISTAGGED_SEQUENCE_TAGS=(
  '!!map' '!!str' '!!int' '!!bool' '!!float' '!!binary' '!!set' '!!timestamp'
  '!!omap' '!!pairs' '!!merge' '!custom' '!!foo' '!<tag:example.com,2026:thing>'
)

# The tag spellings that mean "a plain sequence" and must all still be accepted.
# The non-specific `!` is in the list because yq resolves it to `!!seq` on a
# sequence (measured), so an operator who writes it is writing a plain list.
readonly PLAIN_SEQUENCE_TAGS=('' '!!seq' '!')

# require_superseded_tag_check_satisfied <path>, fail unless this fixture
# SATISFIES the superseded tag-only check. Asserted on the lying-tag fixtures.
require_superseded_tag_check_satisfied() { # <path>
  local path="$1" superseded_answer
  superseded_answer="$(yq eval -r "$SUPERSEDED_TAG_ONLY_EXPRESSION" "$path")" ||
    fail "could not evaluate the superseded tag-only expression against $path"
  [[ $superseded_answer == '!!seq' ]] ||
    fail "fixture $path answers $superseded_answer to the superseded tag-only check, so it does not reproduce the hole this case exists to pin"
}

# require_superseded_kind_check_satisfied <path>, fail unless this fixture
# SATISFIES the superseded kind-only check. Asserted on the mistagged-sequence
# fixtures: without it, a fixture that stopped parsing as a sequence would leave
# a green case guarding nothing.
require_superseded_kind_check_satisfied() { # <path>
  local path="$1" superseded_answer
  superseded_answer="$(yq eval -r "$SUPERSEDED_KIND_ONLY_EXPRESSION" "$path")" ||
    fail "could not evaluate the superseded kind-only expression against $path"
  [[ $superseded_answer == 'seq' ]] ||
    fail "fixture $path answers kind $superseded_answer to the superseded kind-only check, so it does not reproduce the mirror hole this case exists to pin"
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

# write_record_list <path> <tag>, a two-record list carrying the given tag. An
# empty tag writes the plain untagged spelling.
write_record_list() { # <path> <tag>
  printf 'macos:\n  defaults:%s\n    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}\n    - {domain: com.example.alpha, key: AKey, value: "1", type: bool, tier: enforce}\n' \
    "${2:+ $2}" >"$1"
}

[[ -f $LIB ]] || fail "missing library: $LIB"
command -v yq >/dev/null 2>&1 || fail "yq is not on PATH; run inside the nix dev shell"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# shellcheck source=/dev/null
source "$LIB" >/dev/null 2>&1

# ---- 1: every plain-sequence spelling is accepted, in its declared length ----
# The control. Without it every case below passes against a guard that refuses
# unconditionally, which would break every tracked setting on this machine.
for plain_tag in "${PLAIN_SEQUENCE_TAGS[@]}"; do
  write_record_list "$work/list.yaml" "$plain_tag"
  require_shape_accepted "$work/list.yaml" 2 "a real two-record list tagged ${plain_tag:-(untagged)}"
done

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
# Half one of the hole. The tag is written by the document author and says
# nothing about what the node is; this fixture is a map that answers !!seq.
cat >"$work/tagged-map.yaml" <<'EOF'
macos:
  defaults: !!seq
    zebra: {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}
    alpha: {domain: com.example.alpha, key: AKey, value: "1", type: bool, tier: enforce}
EOF
require_superseded_tag_check_satisfied "$work/tagged-map.yaml"
tagged_map_refusal="$(refute_shape_accepted "$work/tagged-map.yaml" "a !!seq-tagged map")"
# The refusal must name the ORDER divergence, not just the shape: that is the
# consequence an operator needs in order to understand why a map is not merely
# an unsupported spelling of a list.
printf '%s' "$tagged_map_refusal" | grep -qi 'order' ||
  fail "the refusal of a !!seq-tagged map does not say that the two readers would apply records in different orders: $tagged_map_refusal"

# ---- 4: a plain map is still refused -----------------------------------------
# The case the superseded tag check already caught, and a false-positive
# direction for the move to a conjunction: a fix that keys on the wrong field
# could refuse the tagged map while dropping this one.
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
# The other half of the tag hole, and the one that reaches further: yq reports a
# scalar's length in BYTES, so a tagged scalar arrives at the record count with
# the tag check fully satisfied and publishes a count bounded only by the size of
# the file. Asserting WHICH check refused is what makes this a pin on the shape
# check rather than an observation that something said no.
cat >"$work/tagged-scalar.yaml" <<'EOF'
macos:
  defaults: !!seq "abc"
EOF
require_superseded_tag_check_satisfied "$work/tagged-scalar.yaml"
tagged_scalar_refusal="$(refute_shape_accepted "$work/tagged-scalar.yaml" "a !!seq-tagged scalar")"
refute_output_contains 'unusable record count' "$tagged_scalar_refusal" \
  "a !!seq-tagged scalar was refused by the COUNT check, not the shape check, so this case pins the wrong guard: $tagged_scalar_refusal"

# ---- 6: a MISTAGGED real sequence is refused, one tag at a time --------------
# The mirror hole, and the regression this file was rewritten for. Every fixture
# here is a genuine two-record sequence, so it satisfies a kind-only check, and
# every one of them must still be refused. The refusal has to say the tag is the
# problem: telling an operator their list "is not a list" when it plainly is one
# sends them to rewrite the records instead of deleting three characters.
for mistagged_tag in "${MISTAGGED_SEQUENCE_TAGS[@]}"; do
  write_record_list "$work/mistagged.yaml" "$mistagged_tag"
  require_superseded_kind_check_satisfied "$work/mistagged.yaml"
  mistagged_refusal="$(refute_shape_accepted "$work/mistagged.yaml" "a real sequence tagged $mistagged_tag")"
  printf '%s' "$mistagged_refusal" | grep -qi 'tag' ||
    fail "the refusal of a sequence tagged $mistagged_tag does not mention the tag, so the operator is sent to rewrite records that are already correct: $mistagged_refusal"
done

# ---- 7: the classifier itself, one yq answer at a time ----------------------
# records_declaration_verdict is pure, so its branches are pinned directly
# instead of through a data file. Two of them are not reachable through a file
# at all once the other checks in the function are doing their job, and one of
# them (the multi-line answer) is DOUBLE-guarded end to end: the record count
# refuses a multi-document file as well, so a whole-file case stays green while
# this branch is broken. That is exactly the pin a direct call provides and a
# fixture cannot.
require_verdict 'seq !!seq' list "a plain sequence"
require_verdict 'map !!map' map "a plain mapping"
require_verdict 'map !!seq' map "a mapping wearing a !!seq tag"
require_verdict 'scalar !!seq' other "a scalar wearing a !!seq tag"
require_verdict 'scalar !!str' other "a plain string"
require_verdict $'seq !!seq\n---\nseq !!seq' other "yq's per-document answers for a multi-document file"
require_verdict '' other "an empty answer, which is what yq prints when .macos is not a mapping"
require_verdict 'seq' other "an answer missing its tag field"
require_verdict 'seq !!seq extra' other "an answer carrying an unexpected third field"

# The accept set is `seq !!seq` and nothing else. Asserted over the same tag
# table the whole-file cases use, so the classifier and the library cannot drift
# into disagreeing about which tags are refused.
for mistagged_tag in "${MISTAGGED_SEQUENCE_TAGS[@]}"; do
  require_verdict "seq $mistagged_tag" mistagged "a real sequence wearing $mistagged_tag"
done

# ---- 8: a file yq cannot parse at all is refused, naming the shape read ------
# The `cannot determine the shape` branch, which is reachable in production by
# the most ordinary defect there is: a YAML syntax error. yq exits non-zero and
# prints no shape answer, so this branch is what stands between a hand-edited
# data file and a caller acting on an empty stream. Nothing else pins it: every
# other case here hands the classifier a real answer.
printf 'macos:\n  defaults:\n  - a\n   bad: [\n' >"$work/unparseable.yaml"
unparseable_yq_status=0
yq eval -r '.macos.defaults | [kind, tag] | join(" ")' "$work/unparseable.yaml" >/dev/null 2>&1 ||
  unparseable_yq_status=$?
[[ $unparseable_yq_status -ne 0 ]] ||
  fail "the unparseable fixture no longer makes yq fail, so this case is not reaching the shape-read failure branch"
unparseable_refusal="$(refute_shape_accepted "$work/unparseable.yaml" "a file yq cannot parse")"
printf '%s' "$unparseable_refusal" | grep -qF 'cannot determine the shape' ||
  fail "a file yq cannot parse was not refused by the shape read, so this case pins the wrong guard: $unparseable_refusal"
printf '%s' "$unparseable_refusal" | grep -qF "$work/unparseable.yaml" ||
  fail "the shape-read refusal does not name the file, so an operator running the tools from anywhere cannot tell which file failed: $unparseable_refusal"

printf 'macos-defaults-shape-guard: OK (every plain-sequence spelling and an empty list are accepted; a !!seq tag on a map or a scalar and any of the %d non-plain tags on a real sequence are all refused; the classifier answers every yq shape directly; an unparseable file is refused by the shape read)\n' \
  "${#MISTAGGED_SEQUENCE_TAGS[@]}"
