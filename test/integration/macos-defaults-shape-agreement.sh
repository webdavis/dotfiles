#!/usr/bin/env bash
# macos-defaults-shape-agreement.sh, the runner template and the shared library
# must read the SAME records in the SAME order from the same data file, or refuse
# it together.
#
# Two independent readers exist for `.macos.defaults`: the chezmoi runner
# template, which applies the settings, and macos-defaults-lib.sh, which the
# apply, capture and drift tools stream records through. Nothing forced them to
# agree, and they did not.
#
# A MAP-valued `.macos.defaults` was accepted by both and read in OPPOSITE
# orders. Go's `range` over a map iterates in sorted KEY order; the library's yq
# stream yields document order. Two readers, two orders, no complaint from
# either. Order decides which write lands last when records touch the same
# domain and key, so this is a silent divergence in what the machine ends up
# holding.
#
# The shape is refused on both sides rather than reconciled. A map is not the
# declared schema, and picking one order to standardize on would leave the other
# reader's behavior a coincidence rather than a guarantee.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl"
LIB="$REPO_ROOT/dot_local/bin/macos-defaults-lib.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

for tool in chezmoi yq; do
  command -v "$tool" >/dev/null 2>&1 || fail "$tool is not on PATH; this suite renders a real template and cannot be meaningfully skipped"
done
[[ -f $TEMPLATE ]] || fail "missing template: $TEMPLATE"
[[ -f $LIB ]] || fail "missing library: $LIB"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# render_template <fixture-file> -> prints the render, returns chezmoi's status
render_template() {
  local src="$work/src"
  rm -rf "$src"
  mkdir -p "$src/.chezmoiscripts" "$src/.chezmoidata" "$work/home"
  cp "$TEMPLATE" "$src/.chezmoiscripts/runner.tmpl"
  cp "$1" "$src/.chezmoidata/macos_defaults.yaml"
  HOME="$work/home" CI=1 chezmoi --source "$src" execute-template --no-tty \
    <"$src/.chezmoiscripts/runner.tmpl" 2>&1
}

# library_stream <fixture-file> -> prints the record stream, returns its status
library_stream() {
  (
    # shellcheck source=/dev/null
    source "$LIB" >/dev/null 2>&1
    defaults_records_unit_separated "$1" 2>&1
  )
}

# ---- the table -------------------------------------------------------------
# Each fixture is written once and put through BOTH readers. A reader-specific
# expectation is what let these two drift in the first place, so the assertions
# below are about agreement, not about either reader's private behavior.

cat >"$work/seq.yaml" <<'EOF'
macos:
  defaults:
    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}
    - {domain: com.example.alpha, key: AKey, value: "1", type: bool, tier: enforce}
  killall: []
EOF

cat >"$work/map.yaml" <<'EOF'
macos:
  defaults:
    zebra: {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}
    alpha: {domain: com.example.alpha, key: AKey, value: "1", type: bool, tier: enforce}
  killall: []
EOF

# ---- 1: a LIST is accepted by both, in declaration order --------------------
# Declared zebra first, alpha second. Sorted key order would invert them, so the
# order assertion is what distinguishes "read the list" from "sorted something".
lib_out="$(library_stream "$work/seq.yaml")" ||
  fail "the library refused a well-formed list of records: $lib_out"
lib_domains="$(printf '%s\n' "$lib_out" | cut -d$'\037' -f1 | tr '\n' ' ')"
[[ $lib_domains == "com.example.zebra com.example.alpha " ]] ||
  fail "library read a list out of declaration order: [$lib_domains]"

tmpl_out="$(render_template "$work/seq.yaml")" ||
  fail "the template refused a well-formed list of records: $tmpl_out"
tmpl_domains="$(printf '%s\n' "$tmpl_out" | grep -oE 'com\.example\.[a-z]+' | tr '\n' ' ')"
[[ $tmpl_domains == "com.example.zebra com.example.alpha " ]] ||
  fail "template read a list out of declaration order: [$tmpl_domains]"

# The point of the whole file: same input, same order, from two readers.
[[ $lib_domains == "$tmpl_domains" ]] ||
  fail "the two readers disagree on record order for a list: library [$lib_domains] vs template [$tmpl_domains]"

# ---- 2: a MAP is refused by both -------------------------------------------
# Refused, not reconciled. Accepting it means one reader sorts and the other does
# not, and the disagreement decides which write lands last.
if lib_out="$(library_stream "$work/map.yaml")"; then
  lib_domains="$(printf '%s\n' "$lib_out" | cut -d$'\037' -f1 | tr '\n' ' ')"
  fail "the library ACCEPTED a map-valued .macos.defaults and emitted [$lib_domains]; the template reads the same file in sorted key order, so the two apply records in different orders"
fi

if tmpl_out="$(render_template "$work/map.yaml")"; then
  tmpl_domains="$(printf '%s\n' "$tmpl_out" | grep -oE 'com\.example\.[a-z]+' | tr '\n' ' ')"
  fail "the template ACCEPTED a map-valued .macos.defaults and rendered [$tmpl_domains]; a map is not the declared schema and Go's range sorts its keys"
fi

# Each refusal must name the shape, or an operator sees a generic parse failure
# and edits the wrong thing.
printf '%s' "$lib_out" | grep -qiE 'list|sequence|map' ||
  fail "the library refused the map without naming the shape problem: $lib_out"
printf '%s' "$tmpl_out" | grep -qiE 'list|sequence|map' ||
  fail "the template refused the map without naming the shape problem: $tmpl_out"

# ---- 3: a document-start byte order mark is refused by both -----------------
# The two readers do not even agree on the file's first key here. yq strips a
# UTF-8 byte order mark and reads every record; chezmoi's Go YAML reader keeps
# it bound into the key and cannot find .macos at all. That asymmetry is why the
# library refuses the mark instead of stripping it: stripping would leave this
# reader accepting a file `chezmoi apply` will not read.
#
# The template's half of the assertion is what makes this an agreement case
# rather than an assertion about the library alone. Without it, "refuse a BOM"
# is a preference; with it, it is the only way the two readers can agree.
printf '\xef\xbb\xbfmacos:\n  defaults:\n    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}\n  killall: []\n' \
  >"$work/leading-byte-order-mark.yaml"

# The divergence itself, measured on this fixture rather than assumed: yq must
# still read the marked file as a healthy one-record list. If a future yq stops
# stripping the mark, this case is no longer pinning a disagreement and says so
# here instead of passing for the wrong reason.
marked_shape="$(yq eval -r '(.macos.defaults // []) | tag' "$work/leading-byte-order-mark.yaml")"
marked_count="$(yq eval -r '(.macos.defaults // []) | length' "$work/leading-byte-order-mark.yaml")"
[[ $marked_shape == '!!seq' && $marked_count == '1' ]] ||
  fail "yq no longer reads the marked fixture as a healthy one-record list (shape $marked_shape, count $marked_count), so this case no longer pins a reader disagreement"

if lib_out="$(library_stream "$work/leading-byte-order-mark.yaml")"; then
  lib_domains="$(printf '%s\n' "$lib_out" | cut -d$'\037' -f1 | tr '\n' ' ')"
  fail "the library ACCEPTED a data file carrying a byte order mark and emitted [$lib_domains]; the template cannot read that file at all, so the two readers disagree about whether the settings exist"
fi

if tmpl_out="$(render_template "$work/leading-byte-order-mark.yaml")"; then
  fail "the template ACCEPTED a data file carrying a byte order mark and rendered [$tmpl_out]; this case exists because it does not"
fi

printf '%s' "$lib_out" | grep -qi 'byte order mark' ||
  fail "the library refused the marked file without naming the byte order mark, so an operator cannot find three invisible bytes: $lib_out"

# ---- 4: the library is never the MORE PERMISSIVE reader on a tagged list ----
# The invariant this whole file exists for, asserted directly instead of through
# one hand-picked fixture, and the one a kind-only shape check broke.
#
# A tag is written by the document author and is independent of what the node
# actually is, so a REAL sequence can wear any tag at all. The template's Go YAML
# reader refuses several of those files with a parse error while yq reads the
# records happily, so a library that judged shape by `kind` alone streamed
# records out of a file `chezmoi apply` cannot read at all.
#
# The assertion is one-directional on purpose: the library may refuse what the
# template renders (a loud refusal naming the file, and the direction this guard
# deliberately errs in, since matching the template exactly would mean
# transcribing which tags one Go YAML release happens to decode as a slice), but
# it must NEVER accept what the template refuses. Written this way the case stays
# green when the template gets stricter and fails only when the library drifts
# back toward permissive.
#
# Case 1 above is the control that keeps this from passing vacuously: a library
# that refuses everything satisfies the invariant and fails case 1.
TAGS_ON_A_REAL_SEQUENCE=(
  '!!map' '!!str' '!!int' '!!bool' '!!float' '!!binary' '!!set' '!!timestamp'
  '!!omap' '!!pairs' '!!merge' '!custom' '!!foo' '!<tag:example.com,2026:thing>'
)

# The subset the template is measured to refuse today. Named so the case carries
# a POSITIVE pin as well as an invariant: without it, both readers turning
# permissive at once would satisfy the one-directional assertion.
TAGS_BOTH_READERS_REFUSE=(
  '!!map' '!!str' '!!int' '!!bool' '!!float' '!!binary' '!!set' '!!timestamp'
)

tagged_sequence_fixture() { # <tag>
  printf 'macos:\n  defaults: %s\n    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}\n  killall: []\n' \
    "$1" >"$work/tagged-sequence.yaml"
  # The fixture only pins anything while it stays a genuine sequence: that is
  # what makes a kind-only check accept it.
  local node_kind
  node_kind="$(yq eval -r '.macos.defaults | kind' "$work/tagged-sequence.yaml")"
  [[ $node_kind == seq ]] ||
    fail "the fixture tagged $1 parses as $node_kind, not a sequence, so it no longer reproduces the hole this case exists to pin"
}

for sequence_tag in "${TAGS_ON_A_REAL_SEQUENCE[@]}"; do
  tagged_sequence_fixture "$sequence_tag"
  library_accepted=1
  lib_out="$(library_stream "$work/tagged-sequence.yaml")" || library_accepted=0
  template_accepted=1
  tmpl_out="$(render_template "$work/tagged-sequence.yaml")" || template_accepted=0
  if [[ $library_accepted -eq 1 && $template_accepted -eq 0 ]]; then
    fail "the library ACCEPTED a real sequence tagged $sequence_tag and emitted [$lib_out] while the template refused the same file [$tmpl_out]; the library must never be the more permissive of the two readers"
  fi
done

for sequence_tag in "${TAGS_BOTH_READERS_REFUSE[@]}"; do
  tagged_sequence_fixture "$sequence_tag"
  if lib_out="$(library_stream "$work/tagged-sequence.yaml")"; then
    fail "the library ACCEPTED a real sequence tagged $sequence_tag and emitted [$lib_out]; the template refuses that file outright"
  fi
  if tmpl_out="$(render_template "$work/tagged-sequence.yaml")"; then
    fail "the template ACCEPTED a real sequence tagged $sequence_tag and rendered [$tmpl_out]; this tag is in the both-refuse table because it was measured to refuse it, so the table needs remeasuring"
  fi
done

# ---- helpers for the whole-file and per-record rows below -------------------
# Each helper drives BOTH readers on one fixture. That is not a stylistic
# preference: PR #113 added a case for a multi-document data file that only ever
# called the library, so the template's silent drop-the-rest behaviour coexisted
# with a green test from the day it was written. A row asserted against one
# reader is not a row about agreement.

# DATA_FILE_NAME_IN_TEMPLATE_MESSAGES, the file name the runner template's own
# refusals open with. Named because it is a literal in another file, and the
# strip below has to know it to keep a message assertion honest.
DATA_FILE_NAME_IN_TEMPLATE_MESSAGES='macos_defaults.yaml'

# refusal_text_without_file_names <text>, the refusal with every FIXTURE PATH
# removed, WHOLE, and the template's own data-file name with it. Every message
# assertion below is made against this, never against the raw refusal.
#
# Load-bearing, not tidiness. Every refusal the library prints begins with the
# data file's path, and every fixture here is named after the defect it carries,
# so `grep runbook` over the raw text matched `.../manual-no-runbook.yaml` and
# went green whatever the message said. Mutation-proved: replacing the runbook
# and forbidden-field refusals with the text `this record is not usable` left
# this whole suite green without this strip, and is caught by the /runbook/
# assertion with it.
#
# It takes no fixture argument, deliberately. A helper handed the ONE path the
# caller happened to think of strips that path and leaves any other, so a row
# whose refusal names a second fixture keeps a free pass; taking every token that
# starts with the fixture directory needs nothing from the caller and cannot be
# passed the wrong file.
#
# Literal substitution rather than a regex or a `sed` program. The fixture
# directory is a mktemp path carrying `.`, which a regex reads as "any
# character", and a substitution that matches text literally needs no escaping at
# all. The token is taken as everything from the fixture directory up to the next
# whitespace, so trailing punctuation goes with it; no assertion here turns on
# punctuation.
refusal_text_without_file_names() { # <text>
  local text="$1" path_token
  [[ -n $work ]] ||
    fail 'refusal_text_without_file_names was called before the fixture directory existed, so it would strip nothing and every message assertion below would be satisfiable by a file name'
  while [[ $text == *"$work/"* ]]; do
    path_token="${text#*"$work/"}"
    path_token="$work/${path_token%%[[:space:]]*}"
    text="${text//"$path_token"/}"
  done
  printf '%s' "${text//"$DATA_FILE_NAME_IN_TEMPLATE_MESSAGES"/}"
}

# require_both_readers_refuse <fixture> <library-pattern> <template-pattern> <description>
# Both readers must refuse, and each refusal must name the defect. The message
# assertions are what stop the case passing for the wrong reason: most of these
# fixtures are refusable by more than one rule, so a status-only assertion goes
# green while the operator is sent to edit a field that is fine.
require_both_readers_refuse() { # <fixture> <library-pattern> <template-pattern> <description>
  local fixture="$1" library_pattern="$2" template_pattern="$3" description="$4"
  local library_output template_output
  if library_output="$(library_stream "$fixture")"; then
    fail "the library ACCEPTED $description and emitted [$library_output]; the template refuses the same file"
  fi
  if template_output="$(render_template "$fixture")"; then
    fail "the template ACCEPTED $description and rendered [$template_output]; this row exists because it does not"
  fi
  grep -qiE -- "$library_pattern" <<<"$(refusal_text_without_file_names "$library_output")" ||
    fail "the library refused $description without naming the defect (expected /$library_pattern/ outside the file name): $library_output"
  grep -qiE -- "$template_pattern" <<<"$(refusal_text_without_file_names "$template_output")" ||
    fail "the template refused $description without naming the defect (expected /$template_pattern/ outside the file name): $template_output"
}

# require_both_readers_accept <fixture> <expected-domains> <description>, the
# false-positive direction, and the half that decides whether the rows above
# bite. A guard that refuses everything satisfies every refusal case in this
# file; only a fixture that must still be read can catch it.
require_both_readers_accept() { # <fixture> <expected-domains> <description>
  local fixture="$1" expected_domains="$2" description="$3"
  local library_output template_output library_domains template_domains
  library_output="$(library_stream "$fixture")" ||
    fail "the library refused $description: $library_output"
  template_output="$(render_template "$fixture")" ||
    fail "the template refused $description: $template_output"
  library_domains="$(printf '%s\n' "$library_output" | cut -d$'\037' -f1 | tr '\n' ' ')"
  [[ $library_domains == "$expected_domains" ]] ||
    fail "the library read $description as [$library_domains], expected [$expected_domains]"
  template_domains="$(printf '%s\n' "$template_output" | grep -oE 'com\.example\.[a-zA-Z]+' | tr '\n' ' ')"
  [[ $template_domains == "$expected_domains" ]] ||
    fail "the template read $description as [$template_domains], expected [$expected_domains]"
}

# require_both_readers_write_the_same <fixture> <description>, the assertion the
# domain comparison above cannot make: for a ONE-record fixture, everything the
# two readers would actually write has to match, field by field.
#
# Comparing domain NAMES was measured to be too weak in both of the ways that
# matter, on files BOTH readers accept:
#
#   host: 0     the library streamed host "0", which sends `just defaults-apply`
#               to `defaults -currentHost write`; the template read Go's numeric
#               zero, which is falsy there, and rendered an ordinary `defaults
#               write`. Same record, same domain, two different STORES.
#   value: [a, b]
#               the library streamed an EMPTY value; the template rendered
#               `[a b]`. Same record, same domain, two different VALUES.
#
# So the target (whether -currentHost is selected), the key, the type option and
# the value are each compared. Both are refused now, and this helper is what
# would notice if either rule were relaxed into accepting them again.
require_both_readers_write_the_same() { # <fixture> <description>
  local fixture="$1" description="$2"
  local library_output template_output write_line
  local library_domain library_key library_type library_value library_host
  local expected_target expected_write
  library_output="$(library_stream "$fixture")" ||
    fail "the library refused $description: $library_output"
  template_output="$(render_template "$fixture")" ||
    fail "the template refused $description: $template_output"
  [[ $(printf '%s\n' "$library_output" | grep -c .) -eq 1 ]] ||
    fail "$description must be a one-record fixture for this helper; the library streamed [$library_output]"
  IFS=$'\037' read -r library_domain library_key library_type library_value library_host _ _ _ \
    <<<"$library_output"
  # The one place the two readers describe the same choice differently: the
  # library carries the host NAME, the template turns a present host into the
  # `-currentHost` word. Comparing the rendered target is what makes them
  # comparable at all.
  expected_target='defaults write'
  [[ -n $library_host ]] && expected_target='defaults -currentHost write'
  expected_write="$expected_target '$library_domain' '$library_key' -$library_type '$library_value'"
  write_line="$(printf '%s\n' "$template_output" | grep -E "^ *(sudo )?defaults( -currentHost)? write " | head -1)"
  [[ ${write_line# } == "$expected_write" ]] ||
    fail "the two readers would not write the same thing for $description: the library says [$expected_write], the template renders [$write_line]"
}

# require_yq_reads_a_healthy_record_list <fixture> <expected-count> <description>,
# the SUPPLIER-behaviour guard for every row whose template half is refused by
# chezmoi's DATA LOADER rather than by a `fail` in the template body.
#
# Those rows have a specific way of going quietly useless. The loader refuses
# before a single template action runs, so "the template refuses this fixture"
# stays true with the entire template body deleted, and it stays true if a future
# yq starts refusing the same file too. Either way the case would pin a supplier's
# behaviour instead of this repo's. Asserting that yq STILL reads the file as a
# healthy record list is what keeps the row a divergence: it is the only reason
# the library needs a rule of its own here.
require_yq_reads_a_healthy_record_list() { # <fixture> <expected-count> <description>
  local fixture="$1" expected_count="$2" description="$3" node_kind record_count
  node_kind="$(yq eval -r '.macos.defaults | kind' "$fixture" 2>/dev/null)" ||
    fail "could not read the record-list kind of the $description fixture"
  record_count="$(yq eval -r '.macos.defaults | length' "$fixture" 2>/dev/null)" ||
    fail "could not count the records of the $description fixture"
  [[ $node_kind == seq && $record_count == "$expected_count" ]] ||
    fail "the $description fixture now answers kind $node_kind and count $record_count to yq, not a healthy $expected_count-record sequence, so it no longer reproduces the divergence this row exists to close"
}

# write_data_file <path> <records-yaml> <extra-yaml>, one fixture with a record
# list and whatever else the row is about.
write_data_file() { # <path> <records-yaml> <extra-yaml>
  printf 'macos:\n  defaults:\n%s  killall: []\n%s' "$2" "${3:-}" >"$1"
}

VALID_RECORD='    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}
'

# ---- 5: chezmoi's data loader refuses files yq reads -------------------------
# Two whole-file rules that chezmoi's YAML reader enforces and yq does not, so
# the library read records out of a file `chezmoi apply` would not load at all.
# Both were found by asking chezmoi rather than by reasoning about YAML: every
# malformed-but-yq-readable shape that could be thought of was put through
# `chezmoi data`, and exactly these two came back refused.
#
# A duplicate key is the one an operator reaches by accident, by copying a block
# and editing half of it. yq keeps both entries and answers traversal with the
# LAST, so the library did not merely accept the file, it acted on different
# records than the ones the operator was looking at.
write_data_file "$work/dup-defaults.yaml" \
  '    - {domain: com.example.first, key: FKey, value: "1", type: bool, tier: enforce}
  defaults:
    - {domain: com.example.second, key: SKey, value: "1", type: bool, tier: enforce}
'
write_data_file "$work/dup-domain.yaml" \
  '    - {domain: com.example.first, key: ZKey, domain: com.example.second, value: "1", type: bool, tier: enforce}
'
write_data_file "$work/dup-killall.yaml" "$VALID_RECORD" '  killall: [Finder]
'
write_data_file "$work/dup-macos.yaml" "$VALID_RECORD" 'macos:
  defaults:
    - {domain: com.example.second, key: SKey, value: "1", type: bool, tier: enforce}
'
for duplicate_key_case in dup-defaults dup-domain dup-killall dup-macos; do
  require_yq_reads_a_healthy_record_list "$work/$duplicate_key_case.yaml" 1 "$duplicate_key_case"
  require_both_readers_refuse "$work/$duplicate_key_case.yaml" \
    'duplicate' 'already defined' "a file with a duplicate mapping key ($duplicate_key_case)"
done

# A key that is not a scalar. Rarer than a duplicate, same direction, and in the
# table because the rule the library needs is "the loader will not read this
# file", not "someone typed a key twice".
write_data_file "$work/seq-as-key.yaml" "$VALID_RECORD" $'probe:\n  ? [a, b]\n  : 1\n'
write_data_file "$work/map-as-key.yaml" "$VALID_RECORD" $'probe:\n  ? {a: 1}\n  : 1\n'
for complex_key_case in seq-as-key map-as-key; do
  require_yq_reads_a_healthy_record_list "$work/$complex_key_case.yaml" 1 "$complex_key_case"
  require_both_readers_refuse "$work/$complex_key_case.yaml" \
    'key' 'invalid key' "a file with a complex mapping key ($complex_key_case)"
done

# ---- 6: a record's payload must match its declared tier in BOTH readers ------
# The template has refused these since tiers were introduced; the library
# accepted every one of them, so `just D` reported on controls that `chezmoi
# apply` refuses to render.
write_data_file "$work/manual-no-runbook.yaml" \
  '    - {domain: com.example.zebra, key: ZKey, tier: manual}
'
write_data_file "$work/manual-blank-runbook.yaml" \
  '    - {domain: com.example.zebra, key: ZKey, tier: manual, runbook: }
'
write_data_file "$work/manual-empty-runbook.yaml" \
  '    - {domain: com.example.zebra, key: ZKey, tier: manual, runbook: ""}
'
for runbook_case in manual-no-runbook manual-blank-runbook manual-empty-runbook; do
  require_both_readers_refuse "$work/$runbook_case.yaml" \
    'runbook' 'runbook' "a manual record whose runbook is missing ($runbook_case)"
done

# One fixture per forbidden field. A shared fixture carrying all five would leave
# four of them unchecked the moment either reader's rule was narrowed to one.
for forbidden_on_manual in type value host scope plist_path; do
  write_data_file "$work/manual-carries-$forbidden_on_manual.yaml" \
    "    - {domain: com.example.zebra, key: ZKey, tier: manual, runbook: \"Section 1\", $forbidden_on_manual: \"x\"}
"
  require_both_readers_refuse "$work/manual-carries-$forbidden_on_manual.yaml" \
    "$forbidden_on_manual" "$forbidden_on_manual" "a manual record carrying $forbidden_on_manual"
done

write_data_file "$work/enforce-carries-runbook.yaml" \
  '    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce, runbook: "Section 1"}
'
require_both_readers_refuse "$work/enforce-carries-runbook.yaml" \
  'runbook' 'runbook' "an enforce record carrying a runbook"

# The false-positive direction for the whole of case 6. Three legitimate records,
# one per tier, including the verify record that MAY carry a runbook, which is
# the case a rule written as "no record may carry a runbook" breaks.
write_data_file "$work/manual-ok.yaml" \
  '    - {domain: com.example.zebra, key: ZKey, tier: manual, runbook: "Section 1"}
'
require_both_readers_accept "$work/manual-ok.yaml" "com.example.zebra " "a manual record naming its runbook"
write_data_file "$work/verify-with-runbook.yaml" \
  '    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: verify, runbook: "Section 1"}
'
verify_library_output="$(library_stream "$work/verify-with-runbook.yaml")" ||
  fail "the library refused a verify record carrying a runbook: $verify_library_output"
verify_template_output="$(render_template "$work/verify-with-runbook.yaml")" ||
  fail "the template refused a verify record carrying a runbook: $verify_template_output"

# ---- 7: a record missing its type is refused BY NAME in both readers ---------
# Both readers already refused this file, so the row is about the MESSAGE. The
# template read `.type` in the bare dotted form, which Go's text/template turns
# into `map has no entry for key "type"` when the field is absent: a template
# panic naming a Go construct instead of the specific refusal sitting two lines
# below it. The template's own comment forbids that form for exactly this reason.
# All three ways to name no type, because the template now separates them the way
# it already separates the three ways to name no tier, and only the first of them
# reached the bare-field panic.
write_data_file "$work/missing-type.yaml" \
  '    - {domain: com.example.zebra, key: ZKey, value: "1", tier: enforce}
'
write_data_file "$work/blank-type.yaml" \
  '    - {domain: com.example.zebra, key: ZKey, value: "1", type: , tier: enforce}
'
write_data_file "$work/bogus-type.yaml" \
  '    - {domain: com.example.zebra, key: ZKey, value: "1", type: nonsense, tier: enforce}
'
for type_case in missing-type blank-type bogus-type; do
  require_both_readers_refuse "$work/$type_case.yaml" \
    'type' 'type' "an enforce record whose type is $type_case"
  type_render="$(render_template "$work/$type_case.yaml")" || true
  if printf '%s' "$type_render" | grep -qF 'map has no entry for key'; then
    fail "the template panics on $type_case instead of refusing it by name; the bare dotted-field form is back: $type_render"
  fi
done

# ---- 8: multiple documents, a DOCUMENTED asymmetry, pinned in both readers ---
# The library refuses; the template renders the FIRST document and silently drops
# the rest. That is not a divergence anyone chose, and it is not one the template
# can close: chezmoi has already parsed the file by the time a template action
# runs and keeps only document 1, so the only evidence left is the raw bytes,
# which `include` does return, and every scan of those bytes is a hand-rolled
# YAML document lexer written in Go template actions. The library counts the
# documents in those bytes instead, which is why the naive form,
# `regexMatch "(?m)^(---|\.\.\.)[ \t]*$"`, is not what it uses: that form answers
# TRUE for a leading `---` and for a trailing `...`, both of which are single
# documents that both readers read correctly today, and case 10 below adds the
# shapes it answers FALSE for.
#
# This row is the direction where the template is the more permissive reader: it
# applies a strict SUBSET of the declared records and never a wrong value. Case
# 10 is the other direction, where the template refuses outright.
#
# So the asymmetry is pinned rather than closed, and pinned from BOTH sides. The
# template half asserts what it ACTUALLY does, record for record: if a future
# chezmoi starts refusing multi-document data files, this fails loudly and the
# library's comment can stop describing a limit that no longer exists.
cat >"$work/multi-document.yaml" <<'EOF'
macos:
  defaults:
    - {domain: com.example.first, key: FKey, value: "1", type: bool, tier: enforce}
  killall: []
---
macos:
  defaults:
    - {domain: com.example.second, key: SKey, value: "1", type: bool, tier: enforce}
  killall: []
EOF
multi_document_count="$(yq eval-all -r '[.] | length' "$work/multi-document.yaml")"
[[ $multi_document_count == 2 ]] ||
  fail "the multi-document fixture no longer parses as 2 documents (yq answered $multi_document_count), so this row pins nothing"

if lib_out="$(library_stream "$work/multi-document.yaml")"; then
  fail "the library ACCEPTED a multi-document data file and emitted [$lib_out]; the template reads only its first document, so the two would act on different record sets"
fi
printf '%s' "$lib_out" | grep -qi 'document' ||
  fail "the library refused the multi-document file without saying the file has more than one document in it: $lib_out"

tmpl_out="$(render_template "$work/multi-document.yaml")" ||
  fail "the template now REFUSES a multi-document data file: [$tmpl_out]. That is the safe direction and better than what it did, but this row asserts the measured asymmetry, so update it and the library's comment rather than deleting this case"
tmpl_domains="$(printf '%s\n' "$tmpl_out" | grep -oE 'com\.example\.[a-z]+' | tr '\n' ' ')"
[[ $tmpl_domains == "com.example.first " ]] ||
  fail "the template rendered [$tmpl_domains] from the multi-document file; this row exists because it renders the first document only, so remeasure it"

# ---- 9: a YAML alias, a DELIBERATE restriction, pinned in both readers -------
# The only rule in this file where the library refuses something the template
# renders happily, and the only one that is a schema decision rather than a
# reader disagreement. The reasons are in the library's comment; what matters
# here is that the asymmetry is pinned in the direction it was chosen in, so
# nobody has to guess later whether it was deliberate.
#
# Both fixtures render correctly through the template today, and one of them
# (the merge key) is what proves the restriction is not free: `<<: *base` is
# ordinary YAML, and the reader that refuses it is choosing to.
cat >"$work/alias-list.yaml" <<'EOF'
records: &records
  - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}
macos:
  defaults: *records
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
for alias_case in alias-list merge-key; do
  if lib_out="$(library_stream "$work/$alias_case.yaml")"; then
    fail "the library ACCEPTED $alias_case; aliases are a deliberate schema restriction, so this must refuse"
  fi
  printf '%s' "$lib_out" | grep -qi 'alias' ||
    fail "the library refused $alias_case without naming the alias, so the operator cannot tell it is a schema rule rather than a broken record: $lib_out"
  tmpl_out="$(render_template "$work/$alias_case.yaml")" ||
    fail "the template now refuses $alias_case: [$tmpl_out]. The restriction is then no longer one-sided, so say so in the library's comment rather than deleting this case"
  printf '%s' "$tmpl_out" | grep -qF "defaults write 'com.example.zebra' 'ZKey' -bool '1'" ||
    fail "the template rendered $alias_case as [$tmpl_out]; this row asserts that it resolves the alias into the same write, so remeasure it"
done

# ---- 10: a file whose FIRST document is empty -------------------------------
# The half of case 8 that runs the other way, and the reason the library counts
# documents from the bytes instead of from yq's answer. yq ELIDES an empty
# document, so all three spellings below look like ordinary one-document files to
# every yq expression; chezmoi's loader keeps the empty first document, cannot
# find `macos` in it, and fails the whole apply. Measured, before the byte
# counter: the library STREAMED the record while the template refused the file.
#
# The consequence is worse than case 8's, and the library's comment says so: a
# multi-document file whose FIRST document holds the records costs a subset of
# the writes, while this one costs the entire `chezmoi apply`, not merely this
# script.
for empty_leading_case in empty-first-document comment-first-document blank-first-document; do
  # ANSI-C quoting, so the leading documents are real newlines in a VALUE rather
  # than escapes in a printf FORMAT. A caller-supplied format string is the SC2059
  # hazard, and here it would also be a live one: the comment spelling carries a
  # `%` away from being read as a conversion.
  case $empty_leading_case in
    empty-first-document) leading_documents=$'---\n---\n' ;;
    comment-first-document) leading_documents=$'---\n# nothing but a comment\n---\n' ;;
    blank-first-document) leading_documents=$'---\n\n---\n' ;;
  esac
  printf '%smacos:\n  defaults:\n    - {domain: com.example.first, key: FKey, value: "1", type: bool, tier: enforce}\n  killall: []\n' \
    "$leading_documents" >"$work/$empty_leading_case.yaml"
  # The supplier-behaviour guard: the row is about a document yq cannot see, so
  # it stops being about anything the moment yq starts seeing it.
  elided_document_count="$(yq eval-all -r '[.] | length' "$work/$empty_leading_case.yaml")"
  [[ $elided_document_count == 1 ]] ||
    fail "yq now answers $elided_document_count documents for $empty_leading_case, so it no longer elides the empty document and this row no longer reproduces the divergence it exists to close"
  require_both_readers_refuse "$work/$empty_leading_case.yaml" \
    'document' 'macos' "a file whose first document is empty ($empty_leading_case)"
done

# ---- 11: .macos.killall, the other node the template reads ------------------
# The library reads no killall at all, which is exactly how a file the template
# refuses came to be streamed as usable. Two shapes, one per direction of the
# fix: an ABSENT key was a template panic (`map has no entry for key "killall"`,
# refusing the whole apply) and is now "nothing to restart" in both readers; a
# SCALAR key is refused by both, because `range` cannot iterate it.
printf 'macos:\n  defaults:\n%s  killall: Finder\n' "$VALID_RECORD" >"$work/killall-scalar.yaml"
require_yq_reads_a_healthy_record_list "$work/killall-scalar.yaml" 1 "killall-scalar"
require_both_readers_refuse "$work/killall-scalar.yaml" \
  'killall' 'killall' "a data file whose killall is a plain scalar"

printf 'macos:\n  defaults:\n%s' "$VALID_RECORD" >"$work/killall-absent.yaml"
require_both_readers_accept "$work/killall-absent.yaml" "com.example.zebra " "a data file with no killall key at all"
killall_absent_render="$(render_template "$work/killall-absent.yaml")" ||
  fail "the template refused a data file with no killall key"
if printf '%s' "$killall_absent_render" | grep -qF 'map has no entry for key'; then
  fail "the template panics on an absent killall instead of rendering nothing to restart; the bare dotted-field form is back: $killall_absent_render"
fi
if printf '%s' "$killall_absent_render" | grep -qE '^killall '; then
  fail "the template rendered a killall command for a file that declares none: $killall_absent_render"
fi

# ---- 12: both readers must WRITE the same thing, not merely accept ----------
# Every accept row above compares domain NAMES, which two measured divergences
# walk straight through: `host: 0` sent this library to -currentHost and the
# template to the ordinary domain, and `value: [a, b]` gave the library an empty
# value and the template `[a b]`. Both files were ACCEPTED by both readers, with
# matching domains, and wrote different things.
#
# The two are refused now, by the library's field-type rule, so the rows below do
# two jobs: the accept rows assert that the legitimate spellings still produce
# one identical write from both readers, and the refuse rows assert the divergent
# spellings stay out.
write_data_file "$work/write-plain.yaml" \
  '    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}
'
require_both_readers_write_the_same "$work/write-plain.yaml" "an ordinary enforce record"
write_data_file "$work/write-bool-value.yaml" \
  '    - {domain: com.example.zebra, key: ZKey, value: false, type: bool, tier: enforce}
'
require_both_readers_write_the_same "$work/write-bool-value.yaml" "a record whose value is YAML's own false, the tracked file's own spelling"
write_data_file "$work/write-host.yaml" \
  '    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce, host: "mac-one"}
'
require_both_readers_write_the_same "$work/write-host.yaml" "a record naming a host, which both readers must send to -currentHost"

write_data_file "$work/host-zero.yaml" \
  '    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce, host: 0}
'
require_yq_reads_a_healthy_record_list "$work/host-zero.yaml" 1 "host-zero"
host_zero_library_output="$(library_stream "$work/host-zero.yaml")" && {
  host_zero_render="$(render_template "$work/host-zero.yaml")" || true
  fail "the library ACCEPTED an unquoted host: 0 and streamed [$host_zero_library_output] while the template rendered [$host_zero_render]; the library sends that to defaults -currentHost and the template to the ordinary domain, so one file writes to two different stores"
}
grep -qi 'host' <<<"$(refusal_text_without_file_names "$host_zero_library_output")" ||
  fail "the library refused host: 0 without naming the host field: $host_zero_library_output"

write_data_file "$work/value-sequence.yaml" \
  '    - {domain: com.example.zebra, key: ZKey, value: [a, b], type: array, tier: enforce}
'
require_yq_reads_a_healthy_record_list "$work/value-sequence.yaml" 1 "value-sequence"
value_sequence_library_output="$(library_stream "$work/value-sequence.yaml")" && {
  value_sequence_render="$(render_template "$work/value-sequence.yaml")" || true
  fail "the library ACCEPTED a sequence value and streamed [$value_sequence_library_output] while the template rendered [$value_sequence_render]; the library writes an empty value there and the template writes the joined sequence, so one file writes two different values"
}
grep -qi 'value' <<<"$(refusal_text_without_file_names "$value_sequence_library_output")" ||
  fail "the library refused a sequence value without naming the value field: $value_sequence_library_output"

printf 'macos-defaults-shape-agreement: OK (both readers take a list in declaration order, both refuse a map naming the shape, both refuse a file carrying a byte order mark, and across %d tags on a real sequence the library never accepts a file the template refuses; both refuse a duplicate or complex mapping key, an empty leading document, a killall neither can walk, every tier/payload mismatch, and a record with no type, while still reading every legitimate record and writing it identically from both readers; the multi-document and alias asymmetries are pinned from both sides)\n' \
  "${#TAGS_ON_A_REAL_SEQUENCE[@]}"
