#!/usr/bin/env bash
# macos-defaults-data-file-rules-guard.sh, the WHOLE-FILE rules, the ones that
# are true or false about macos_defaults.yaml before anything looks at
# `.macos.defaults` at all.
#
# The record-list shape guard next door asks what `.macos.defaults` is. This one
# asks a question that comes first: will the file's OTHER reader accept the file
# at all, and does the file use YAML this schema does not allow. Four whole-file
# states, measured against yq v4.53.3 and chezmoi v2.71.1 rather than reasoned
# about:
#
#   MULTIPLE DOCUMENTS   yq answers once per document, so every expression this
#                        library runs comes back with one line per document.
#                        chezmoi's data loader keeps the FIRST document and
#                        silently discards the rest, so `chezmoi apply` applies a
#                        subset of the tracked records and says nothing. The
#                        library refuses; the template renders document 1. That
#                        asymmetry is pinned in
#                        test/integration/macos-defaults-shape-agreement.sh,
#                        which drives both readers; this file pins the refusal
#                        and its message.
#
#   DUPLICATE MAPPING KEY  yq keeps BOTH entries in the node tree and answers
#                        traversal with the LAST one. chezmoi's loader refuses
#                        the whole file (`mapping key "x" already defined at
#                        [n:m]`). So the library read records while `chezmoi
#                        apply` would not read the file at all, which is the
#                        permissive direction this whole guard family exists to
#                        close.
#
#   COMPLEX MAPPING KEY  a key that is itself a sequence or a mapping (`? [a, b]
#                        : 1`). yq reads it; chezmoi's loader refuses the file
#                        (`found an invalid key for this map`). Same direction,
#                        same fix.
#
#   ALIAS                a YAML alias (`*anchor`), including the merge key
#                        (`<<: *anchor`). BOTH readers accept these, so this one
#                        is not a divergence: it is a deliberate schema
#                        restriction, and the library is the STRICTER reader on
#                        purpose. The reasons are in the library's own comment;
#                        what this file pins is that the refusal happens, names
#                        the alias, and does not fire on a file that merely
#                        DEFINES an anchor without referencing it.
#
# The alias rule is the one that can only be wrong in the expensive direction,
# so it carries the most false-positive cases below: refusing a legitimate file
# breaks every tracked setting on the machine, and an anchor with no alias is a
# legitimate file.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIB="$REPO_ROOT/dot_local/bin/macos-defaults-lib.sh"
TRACKED_DATA_FILE="$REPO_ROOT/.chezmoidata/macos_defaults.yaml"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# require_verdict <yq-answer> <expected-verdict> <description>, the classifier
# called directly, one answer at a time.
#
# The whole-file cases below cannot stand in for these. Several inputs are
# refused by more than one barrier, so a whole-file case stays green while a
# branch of the classifier is broken and a different barrier does the refusing.
# Feeding the classifier its input directly is what makes each branch fail alone.
require_verdict() { # <yq-answer> <expected-verdict> <description>
  local answer="$1" expected_verdict="$2" description="$3" verdict
  verdict="$(data_file_rules_verdict "$answer")"
  [[ $verdict == "$expected_verdict" ]] ||
    fail "$description must classify as $expected_verdict, got $verdict (input: $(printf '%q' "$answer"))"
}

# refute_data_file_accepted <path> <description>, require
# defaults_records_declared_count to refuse this file with status 2, and print
# the refusal so the caller can assert on the message.
#
# A helper rather than an inline `if ! ...`: under `set -e` an inverted command
# only decides the test in FINAL position, so a bare negation inside a loop body
# is a position lottery.
refute_data_file_accepted() { # <path> <description>
  local path="$1" description="$2" status=0 output
  output="$(defaults_records_declared_count "$path" 2>&1)" || status=$?
  [[ $status -eq 2 ]] ||
    fail "$description must be refused with status 2, got $status (output: $output)"
  printf '%s' "$output"
}

# require_data_file_accepted <path> <expected-count> <description>, the
# false-positive direction: this file must be ACCEPTED and answer the count.
require_data_file_accepted() { # <path> <expected-count> <description>
  local path="$1" expected_count="$2" description="$3" status=0 count
  count="$(defaults_records_declared_count "$path" 2>&1)" || status=$?
  [[ $status -eq 0 ]] ||
    fail "$description must be accepted, got status $status ($count)"
  [[ $count == "$expected_count" ]] ||
    fail "$description must count $expected_count record(s), got: $count"
}

# require_refusal_names <refusal> <pattern> <description>, the refusal must say
# WHICH whole-file rule was broken.
#
# Every one of these files is refused by more than one thing once the guard is
# in place (a duplicate `defaults` key also changes what the record list is), so
# a status-only assertion passes while the operator is sent to edit the wrong
# part of the file. The message is the assertion that carries the case.
require_refusal_names() { # <refusal> <pattern> <description>
  local refusal="$1" pattern="$2" description="$3"
  printf '%s' "$refusal" | grep -qiE -- "$pattern" ||
    fail "$description was refused without naming its defect (expected to match /$pattern/): $refusal"
}

# require_yq_reads_a_healthy_record_list <path> <expected-count> <description>,
# the SUPPLIER-behaviour guard, asserted on every fixture whose defect yq does
# not mind.
#
# Without it these cases could pass for the wrong reason. If a future yq started
# refusing a duplicate mapping key itself, the fixture would still be refused,
# every assertion below would stay green, and the guard being pinned here could
# be deleted without a single test noticing. This says out loud that yq still
# reads the file as a healthy N-record list, which is the only reason the guard
# has to exist.
require_yq_reads_a_healthy_record_list() { # <path> <expected-count> <description>
  local path="$1" expected_count="$2" description="$3" node_kind record_count
  node_kind="$(yq eval -r '.macos.defaults | kind' "$path" 2>/dev/null)" ||
    fail "could not read the record-list kind of $path"
  record_count="$(yq eval -r '.macos.defaults | length' "$path" 2>/dev/null)" ||
    fail "could not count the records of $path"
  [[ $node_kind == seq && $record_count == "$expected_count" ]] ||
    fail "$description: yq now answers kind $node_kind and count $record_count, not a healthy $expected_count-record sequence, so this fixture no longer reproduces the divergence the guard exists to close"
}

[[ -f $LIB ]] || fail "missing library: $LIB"
command -v yq >/dev/null 2>&1 || fail "yq is not on PATH; run inside the nix dev shell"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# shellcheck source=/dev/null
source "$LIB" >/dev/null 2>&1

# write_valid_file <path> <extra-yaml>, a file whose record list is always a
# healthy one-record sequence, plus whatever oddity a case is about. Keeping the
# record list constant is what makes each case a test of the whole-file rule
# rather than of the record list.
write_valid_file() { # <path> <extra-yaml>
  printf 'macos:\n  defaults:\n    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}\n  killall: []\n%s' \
    "${2:-}" >"$1"
}

# ---- 1: a clean file is accepted -------------------------------------------
# The control, and the case that carries every refusal below: a guard that
# refuses unconditionally passes cases 2 through 8 and breaks every tracked
# setting on this machine.
write_valid_file "$work/clean.yaml"
require_data_file_accepted "$work/clean.yaml" 1 "a clean one-record file"

# The REAL tracked data file, which is the file this guard runs against every
# time anyone types `just D`. A rule that refuses it is not a stricter rule, it
# is an outage.
[[ -f $TRACKED_DATA_FILE ]] || fail "missing tracked data file: $TRACKED_DATA_FILE"
tracked_status=0
tracked_count="$(defaults_records_declared_count "$TRACKED_DATA_FILE" 2>&1)" || tracked_status=$?
[[ $tracked_status -eq 0 ]] ||
  fail "the tracked data file must be accepted, got status $tracked_status ($tracked_count)"
[[ $tracked_count -gt 0 ]] ||
  fail "the tracked data file answered $tracked_count records, so this control no longer proves the guard admits real data"

# ---- 2: a DUPLICATE MAPPING KEY is refused, wherever it sits ----------------
# All four positions, because the rule is about the FILE and not about one node.
# A guard bolted to `.macos.defaults` would catch the second of these and miss
# the other three, and all four are refused by chezmoi's loader identically.
cat >"$work/dup-defaults.yaml" <<'EOF'
macos:
  defaults:
    - {domain: com.example.first, key: FKey, value: "1", type: bool, tier: enforce}
  defaults:
    - {domain: com.example.second, key: SKey, value: "1", type: bool, tier: enforce}
  killall: []
EOF
cat >"$work/dup-domain.yaml" <<'EOF'
macos:
  defaults:
    - {domain: com.example.first, key: ZKey, domain: com.example.second, value: "1", type: bool, tier: enforce}
  killall: []
EOF
cat >"$work/dup-killall.yaml" <<'EOF'
macos:
  defaults:
    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}
  killall: [Dock]
  killall: [Finder]
EOF
cat >"$work/dup-macos.yaml" <<'EOF'
macos:
  defaults:
    - {domain: com.example.first, key: FKey, value: "1", type: bool, tier: enforce}
macos:
  defaults:
    - {domain: com.example.second, key: SKey, value: "1", type: bool, tier: enforce}
EOF

for duplicate_case in dup-defaults dup-domain dup-killall dup-macos; do
  require_yq_reads_a_healthy_record_list "$work/$duplicate_case.yaml" 1 "$duplicate_case"
  duplicate_refusal="$(refute_data_file_accepted "$work/$duplicate_case.yaml" "a file with a duplicate mapping key ($duplicate_case)")"
  require_refusal_names "$duplicate_refusal" 'duplicate' "$duplicate_case"
  require_refusal_names "$duplicate_refusal" "$work/$duplicate_case.yaml" "$duplicate_case (the path)"
done

# ---- 3: a COMPLEX MAPPING KEY is refused ------------------------------------
# A key that is not a scalar. chezmoi's loader answers `found an invalid key for
# this map` and reads nothing; yq reads the file happily. Same permissive
# direction as a duplicate key, and found the same way: by asking chezmoi.
write_valid_file "$work/seq-as-key.yaml" $'probe:\n  ? [a, b]\n  : 1\n'
write_valid_file "$work/map-as-key.yaml" $'probe:\n  ? {a: 1}\n  : 1\n'
for complex_case in seq-as-key map-as-key; do
  require_yq_reads_a_healthy_record_list "$work/$complex_case.yaml" 1 "$complex_case"
  complex_refusal="$(refute_data_file_accepted "$work/$complex_case.yaml" "a file with a complex mapping key ($complex_case)")"
  require_refusal_names "$complex_refusal" 'key' "$complex_case"
done

# ---- 4: an ALIAS is refused, in every position it can appear -----------------
# The deliberate schema restriction, and the one rule here that BOTH readers
# would otherwise accept. Three positions, because an alias can stand for the
# whole record list, for one record inside it, or for a set of fields merged into
# a record, and a check aimed at any single position misses the other two.
cat >"$work/alias-list.yaml" <<'EOF'
records: &records
  - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}
macos:
  defaults: *records
  killall: []
EOF
cat >"$work/alias-record.yaml" <<'EOF'
base: &base {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}
macos:
  defaults:
    - *base
  killall: []
EOF
cat >"$work/merge-key.yaml" <<'EOF'
base: &base {key: ZKey, value: "1", type: bool, tier: enforce}
macos:
  defaults:
    - <<: *base
      domain: com.example.zebra
  killall: []
EOF

for alias_case in alias-list alias-record merge-key; do
  alias_refusal="$(refute_data_file_accepted "$work/$alias_case.yaml" "a file using a YAML alias ($alias_case)")"
  require_refusal_names "$alias_refusal" 'alias' "$alias_case"
done

# The alias rule must be reached BEFORE the record-level checks, or two of these
# three files are refused for a reason that is not true of them. Measured before
# the rule existed: `alias-record` and `merge-key` were both refused with
# "record 0 ... has a blank value", because yq's `has("value")` answers false
# through an alias while the join that renders the record resolves it. The
# record does declare a value; the reader was internally inconsistent. Naming
# the alias is what stops sending the operator to fix a field that is not broken.
for alias_case in alias-record merge-key; do
  alias_refusal="$(refute_data_file_accepted "$work/$alias_case.yaml" "$alias_case")"
  if printf '%s' "$alias_refusal" | grep -qi 'blank value'; then
    fail "$alias_case is still refused as a blank value, which is false of the record: the alias supplies one. $alias_refusal"
  fi
done

# ---- 5: an ANCHOR with no alias is ACCEPTED ---------------------------------
# The false-positive direction for the alias rule, and the one that decides
# whether it is shippable. An anchor is a label; on its own it changes nothing
# either reader sees, and both accept it. Refusing it would be strictness with
# no divergence behind it.
cat >"$work/anchor-unused.yaml" <<'EOF'
unused: &unused {a: 1}
macos:
  defaults:
    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}
  killall: []
EOF
require_data_file_accepted "$work/anchor-unused.yaml" 1 "a file defining an anchor it never references"

# ---- 6: MULTIPLE DOCUMENTS are refused, naming the documents ----------------
# The library already refused this, through the record-list shape check, with a
# message that showed yq's raw per-document answer and never said the file has
# more than one document in it. The refusal is not the deliverable here, the
# message is.
cat >"$work/multi-document.yaml" <<'EOF'
macos:
  defaults:
    - {domain: com.example.a, key: AKey, value: "1", type: bool, tier: enforce}
  killall: []
---
macos:
  defaults:
    - {domain: com.example.b, key: BKey, value: "1", type: bool, tier: enforce}
  killall: []
EOF
multi_document_count="$(yq eval-all -r '[.] | length' "$work/multi-document.yaml")"
[[ $multi_document_count == 2 ]] ||
  fail "the multi-document fixture no longer parses as 2 documents (yq answered $multi_document_count), so this case pins nothing"
multi_document_refusal="$(refute_data_file_accepted "$work/multi-document.yaml" "a multi-document file")"
require_refusal_names "$multi_document_refusal" 'document' "a multi-document file"
# The consequence, not just the fact. An operator who is told "more than one
# document" and not told what that costs has no reason to treat it as urgent:
# `chezmoi apply` applies the first document and drops the rest in silence.
require_refusal_names "$multi_document_refusal" 'first' "a multi-document file (the consequence)"

# ---- 7: a legal single document keeps its markers ---------------------------
# The false-positive direction for the document rule. A leading `---` and a
# trailing `...` are both legal spellings of ONE document, both accepted by both
# readers today, and both are what a naive scan for document markers refuses
# (measured: a `(?m)^(---|\.\.\.)\s*$` match answers true for both).
printf -- '---\nmacos:\n  defaults:\n    - {domain: com.example.a, key: AKey, value: "1", type: bool, tier: enforce}\n  killall: []\n' \
  >"$work/leading-marker.yaml"
printf -- 'macos:\n  defaults:\n    - {domain: com.example.a, key: AKey, value: "1", type: bool, tier: enforce}\n  killall: []\n...\n' \
  >"$work/trailing-end-marker.yaml"
cat >"$work/block-scalar-marker.yaml" <<'EOF'
note: |
  ---
  not a document separator
macos:
  defaults:
    - {domain: com.example.a, key: AKey, value: "1", type: bool, tier: enforce}
  killall: []
EOF
for single_document_case in leading-marker trailing-end-marker block-scalar-marker; do
  require_data_file_accepted "$work/$single_document_case.yaml" 1 "a single document written as $single_document_case"
done

# ---- 8: the classifier itself, one answer at a time -------------------------
# Pure, so its branches are pinned directly rather than through a data file.
# Every counted rule gets its own answer, and the priority between them is
# pinned too: a file can break more than one rule at once and the operator has
# to be told about one of them by name rather than about whichever the
# implementation happened to test first.
require_verdict '0 0 0' satisfied "a file that breaks no whole-file rule"
require_verdict '1 0 0' duplicate_mapping_key "a file with one duplicate mapping key"
require_verdict '9 0 0' duplicate_mapping_key "a file with several duplicate mapping keys"
require_verdict '0 1 0' complex_mapping_key "a file with a complex mapping key"
require_verdict '0 0 1' alias "a file using an alias"
require_verdict '1 1 1' duplicate_mapping_key "a file breaking all three rules at once"
require_verdict '0 1 1' complex_mapping_key "a file with both a complex key and an alias"
require_verdict $'0 0 0\n0 0 0' multiple_documents "yq's per-document answers for a two-document file"
require_verdict $'0 0 0\n1 0 0\n0 0 0' multiple_documents "a three-document file whose second document also breaks a rule"

# Fail-closed on anything this classifier cannot read. Unrecognized must never
# resolve to the accepted verdict: a yq release that reshaped its answer would
# otherwise turn every rule above into a silent no-op.
require_verdict '' unclassifiable "the empty answer"
require_verdict '0 0' unclassifiable "an answer missing its third count"
require_verdict '0 0 0 0' unclassifiable "an answer carrying a fourth count"
require_verdict 'a b c' unclassifiable "an answer whose counts are not numbers"
require_verdict '0 0 -1' unclassifiable "an answer carrying a negative count"
require_verdict ' 0 0 0' unclassifiable "an answer with leading whitespace"

# ---- 9: the rules read itself must FAIL CLOSED ------------------------------
# The rules expression is a yq call like any other and it can fail. No data file
# can reach that branch, though, and the difference matters: the shape read runs
# first and refuses every file yq cannot parse, so a fixture aimed at this branch
# is refused before the branch is reached and pins nothing. Mutation-checked, and
# it was not a hypothetical: making the read failure `return 0` left a fixture
# version of this case entirely green.
#
# A STUBBED yq reaches it. The stub answers the shape read normally and fails the
# rules expression, which is a state no real file produces and exactly the state
# the branch exists for: a yq that dies partway, is killed, or hits an expression
# a future release rejects.
#
# A separate process rather than a PATH change in this one: bash caches resolved
# command paths, so a stub that appears on PATH after the real yq has run may
# never be consulted, and a wiring pin that silently tested the real yq would be
# worse than no pin at all.
mkdir -p "$work/stub-bin"
cat >"$work/stub-bin/yq" <<'STUB'
#!/usr/bin/env bash
# A yq that answers the shape read and fails the whole-file rules read. Invoked
# as `yq eval -r <expression> <path>`, so the expression follows -r. It refuses
# anything it was not told about, so a library that reshapes an expression makes
# this case fail loudly instead of quietly stubbing a call nobody makes.
set -euo pipefail
expression=""
previous_argument=""
for argument in "$@"; do
  [[ $previous_argument == '-r' ]] && expression="$argument"
  previous_argument="$argument"
done
case $expression in
  "$STUB_YQ_SHAPE_EXPRESSION") printf 'seq !!seq\n' ;;
  "$STUB_YQ_RULES_EXPRESSION")
    printf 'stub yq: the rules read fails here\n' >&2
    exit 1
    ;;
  *)
    printf 'stub yq: asked an expression it was not told to answer: %q\n' "$expression" >&2
    exit 3
    ;;
esac
STUB
chmod +x "$work/stub-bin/yq"
# The file the stubbed case is asked about. Its CONTENT is irrelevant to the
# stub, but it must exist and carry no byte order mark: the mark predicate reads
# real bytes with `head` and is not stubbed.
printf 'macos:\n  defaults: []\n' >"$work/stubbed.yaml"

stubbed_status=0
stubbed_output="$(
  PATH="$work/stub-bin:$PATH" \
    STUB_YQ_SHAPE_EXPRESSION="$DEFAULTS_RECORDS_SHAPE_EXPRESSION" \
    STUB_YQ_RULES_EXPRESSION="$DEFAULTS_DATA_FILE_RULES_EXPRESSION" \
    bash -c 'source "$1"; defaults_records_declared_count "$2"' _ "$LIB" "$work/stubbed.yaml" 2>&1
)" || stubbed_status=$?
[[ $stubbed_status -eq 2 ]] ||
  fail "a failed whole-file rules read must refuse the file with status 2, got $stubbed_status ($stubbed_output); this branch is the difference between refusing a file nobody could check and streaming it"
# The message has to name THIS check, not merely be a status-2 refusal. Mutation
# -checked: with the read failure changed to `return 0` the caller runs on to the
# count read, which the stub also refuses, so the status stays 2 and a refusal
# that names only the file stays green while the branch under test is gone.
printf '%s' "$stubbed_output" | grep -qF 'cannot check the whole-file rules' ||
  fail "a failed rules read was not reported as one; something else refused the file and this case is pinning that instead: $stubbed_output"
printf '%s' "$stubbed_output" | grep -qF -- "$work/stubbed.yaml" ||
  fail "the failed rules read did not name the file, so an operator cannot tell which file could not be checked: $stubbed_output"

# And the file that fails BOTH reads keeps the shape read's message, because that
# is the question the caller asked and a YAML syntax error is the ordinary way to
# reach it. This is what the rules gate sitting after the shape READ buys.
printf 'macos:\n  defaults:\n  - a\n   bad: [\n' >"$work/unparseable.yaml"
unparseable_yq_status=0
yq eval -r "$DEFAULTS_DATA_FILE_RULES_EXPRESSION" "$work/unparseable.yaml" >/dev/null 2>&1 ||
  unparseable_yq_status=$?
[[ $unparseable_yq_status -ne 0 ]] ||
  fail "the unparseable fixture no longer makes the rules expression fail, so it no longer exercises the ordering this case is about"
unparseable_refusal="$(refute_data_file_accepted "$work/unparseable.yaml" "a file yq cannot parse")"
require_refusal_names "$unparseable_refusal" 'cannot determine the shape' "a file yq cannot parse"
require_refusal_names "$unparseable_refusal" "$work/unparseable.yaml" "a file yq cannot parse"

printf 'macos-defaults-data-file-rules-guard: OK (a clean file and the real tracked file are accepted; a duplicate mapping key is refused in all four positions and a complex key in both; an alias is refused wherever it appears while an unreferenced anchor is not; multiple documents are refused by name while a leading --- and a trailing ... are not; the classifier answers every shape directly and fails closed on the rest)\n'
