#!/usr/bin/env bash
# macos-defaults-tier-payload-guard.sh, a record's PAYLOAD must match the tier it
# declares, in the shared library as well as in the runner template.
#
# The tier decides what a record IS. The runner template has enforced that since
# tiers were introduced: a manual control must name its runbook section and must
# carry no write payload, and an enforce control must not carry a runbook,
# because silently ignored data is how a mislabeled tier hides. The library
# enforced only that the tier is one of the three names.
#
# So five record shapes were ACCEPTED by the library and REFUSED by the template
# (measured, yq v4.53.3 and chezmoi v2.71.1):
#
#   tier: manual with no runbook key
#   tier: manual with a blank runbook (a nil scalar)
#   tier: manual with an empty runbook ("")
#   tier: manual carrying any of type, value, host, scope, plist_path
#   tier: enforce carrying a runbook
#
# That is the permissive direction: `just D` and `just defaults-apply` read those
# records and act on them while `chezmoi apply` refuses the whole file, so the
# operator's drift report describes controls the machine will never be given.
#
# The library's own comment used to justify the gap, saying the runbook rules
# "live in the runner template alone: the runbook is not one of the eight fields
# the record stream carries, so this gate cannot see it". The stream cannot, but
# the gate is not limited to the stream: defaults_records_declare_a_value already
# asks the FILE about a property the joined line cannot answer. These rules are
# asked the same way.
#
# What this file pins: the two PURE predicates that hold the rules, and the
# whole-file refusals they produce. The other reader's half lives in
# test/integration/macos-defaults-shape-agreement.sh, which puts every fixture
# below through the real template too.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIB="$REPO_ROOT/dot_local/bin/macos-defaults-lib.sh"
TRACKED_DATA_FILE="$REPO_ROOT/.chezmoidata/macos_defaults.yaml"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# refute_stream_accepted <path> <description>, require the record stream to
# refuse this file with status 2 and emit NOTHING on stdout, then print the
# refusal so the caller can assert on the message.
#
# The empty-stdout half matters as much as the status. A caller must never act on
# part of a stream it is about to be told is malformed, and a refusal that still
# printed records would let `just defaults-apply` write the good records out of a
# file it had just rejected.
refute_stream_accepted() { # <path> <description>
  local path="$1" description="$2" status=0 emitted refusal
  emitted="$(defaults_records_unit_separated "$path" 2>/dev/null)" || status=$?
  [[ $status -eq 2 ]] ||
    fail "$description must be refused with status 2, got $status"
  [[ -z $emitted ]] ||
    fail "$description was refused but still emitted records, so a caller could act on a stream it was told is unusable: $(printf '%q' "$emitted")"
  refusal="$(defaults_records_unit_separated "$path" 2>&1 >/dev/null)" || true
  printf '%s' "$refusal"
}

# require_stream_accepted <path> <expected-records> <description>, the
# false-positive direction: this file must stream exactly this many records.
require_stream_accepted() { # <path> <expected-records> <description>
  local path="$1" expected_records="$2" description="$3" status=0 emitted line_count
  emitted="$(defaults_records_unit_separated "$path" 2>&1)" || status=$?
  [[ $status -eq 0 ]] ||
    fail "$description must be accepted, got status $status ($emitted)"
  line_count=0
  [[ -n $emitted ]] && line_count="$(printf '%s\n' "$emitted" | grep -c .)"
  [[ $line_count -eq $expected_records ]] ||
    fail "$description must stream $expected_records record(s), got $line_count: $(printf '%q' "$emitted")"
}

# refusal_text_without_file_names <text>, the refusal with every FIXTURE PATH
# removed, WHOLE, so the assertion below cannot be satisfied by the name of the
# file the refusal is about.
#
# Every refusal the library prints begins with the data file's path, and every
# fixture here is named after the rule it breaks, so `grep runbook` over the raw
# text matched `.../manual-no-runbook.yaml`. Mutation-proved on the tree that
# lacked this: replacing both new refusal messages with `this record is not
# usable` left the whole suite green.
#
# The BASENAME goes too, and that is the half worth spelling out: an earlier
# version removed only the fixture directory and the `.yaml` suffix, which leaves
# `manual-no-runbook` sitting in the text and satisfies /runbook/ on its own.
# Measured, on that version: the same generic-message mutation was killed only by
# the `com\.example\.zebra` assertion, never by the rule-name assertions this
# helper exists to make honest.
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
  printf '%s' "$text"
}

# require_refusal_names <refusal> <pattern> <description>, the refusal must name
# the rule that was broken, in its MESSAGE and not in the file name it opens with.
#
# Load-bearing here rather than decorative. Every fixture below is a complete,
# well-formed record apart from the one rule it breaks, and several of them are
# already refused by an unrelated check once a fix is half-written: a status-only
# assertion goes green while the operator is told to fix a field that is fine.
require_refusal_names() { # <refusal> <pattern> <description>
  local refusal="$1" pattern="$2" description="$3"
  grep -qiE -- "$pattern" <<<"$(refusal_text_without_file_names "$refusal")" ||
    fail "$description was refused without naming the rule (expected to match /$pattern/ outside the file name): $refusal"
}

# require_field_forbidden / refute_field_forbidden, the pure predicate, called
# directly with one tier and one field name.
#
# Called directly because the whole-file cases cannot separate the two tiers'
# tables: a predicate that ignored its tier argument and forbade the union of
# both tables would refuse every fixture below AND every legitimate record, and
# only the direct calls say which half is wrong.
require_field_forbidden() { # <tier> <field> <description>
  record_field_is_forbidden_for_tier "$1" "$2" ||
    fail "$3: $2 must be forbidden on a $1 record"
}

refute_field_forbidden() { # <tier> <field> <description>
  if record_field_is_forbidden_for_tier "$1" "$2"; then
    fail "$3: $2 must be allowed on a $1 record"
  fi
}

require_runbook_required() { # <tier> <description>
  record_tier_requires_a_runbook "$1" ||
    fail "$2: a $1 record must be required to name a runbook"
}

refute_runbook_required() { # <tier> <description>
  if record_tier_requires_a_runbook "$1"; then
    fail "$2: a $1 record must not be required to name a runbook"
  fi
}

[[ -f $LIB ]] || fail "missing library: $LIB"
command -v yq >/dev/null 2>&1 || fail "yq is not on PATH; run inside the nix dev shell"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# shellcheck source=/dev/null
source "$LIB" >/dev/null 2>&1

# write_record_file <path> <record-yaml>, one record in flow style, so each case
# differs from the control by exactly the field it is about.
write_record_file() { # <path> <record-yaml>
  printf 'macos:\n  defaults:\n    - %s\n  killall: []\n' "$2" >"$1"
}

# ---- 1: the legitimate records are accepted ---------------------------------
# The control, and it carries the whole file: every refusal below passes against
# a gate that refuses all three tiers, which would break every tracked setting on
# this machine. Each tier gets its own case because the rules differ per tier and
# a table applied to the wrong tier is exactly the mistake being guarded against.
write_record_file "$work/enforce-ok.yaml" '{domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}'
require_stream_accepted "$work/enforce-ok.yaml" 1 "an enforce record with a write payload and no runbook"

write_record_file "$work/verify-ok.yaml" '{domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: verify}'
require_stream_accepted "$work/verify-ok.yaml" 1 "a verify record with a read payload and no runbook"

# A verify record MAY carry a runbook: the posture check that consumes verify
# records points the operator at the fix for a detected drift. This is the case a
# rule written as "no record may carry a runbook" breaks, and the template
# renders it today.
write_record_file "$work/verify-with-runbook.yaml" '{domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: verify, runbook: "Section 1"}'
require_stream_accepted "$work/verify-with-runbook.yaml" 1 "a verify record carrying a runbook"

write_record_file "$work/manual-ok.yaml" '{domain: com.example.zebra, key: ZKey, tier: manual, runbook: "Section 1"}'
require_stream_accepted "$work/manual-ok.yaml" 1 "a manual record naming its runbook and carrying no payload"

# The REAL tracked data file. It carries records of every tier, so a rule that is
# wrong about any of them shows up here as an outage rather than as strictness.
[[ -f $TRACKED_DATA_FILE ]] || fail "missing tracked data file: $TRACKED_DATA_FILE"
tracked_status=0
tracked_stream="$(defaults_records_unit_separated "$TRACKED_DATA_FILE" 2>&1)" || tracked_status=$?
[[ $tracked_status -eq 0 ]] ||
  fail "the tracked data file must still stream, got status $tracked_status ($tracked_stream)"
tracked_record_count="$(printf '%s\n' "$tracked_stream" | grep -c .)"
[[ $tracked_record_count -gt 0 ]] ||
  fail "the tracked data file streamed $tracked_record_count records, so this control no longer proves the gate admits real data"
# WHICH tiers the tracked file exercises, asserted rather than assumed. Measured
# today it carries enforce and verify records and no manual one, so the manual
# rules are carried by the fixtures below alone. Pinning the set is what says so:
# if a manual record is ever added to the tracked file, this control starts
# covering that tier too and the assertion below is what notices.
tracked_tiers="$(printf '%s\n' "$tracked_stream" | cut -d$'\037' -f8 | sort -u | tr '\n' ' ')"
[[ $tracked_tiers == "enforce verify " ]] ||
  fail "the tracked data file now carries tiers [$tracked_tiers] rather than [enforce verify ]; remeasure which tiers this control exercises"

# There was a loop here walking every streamed tier and failing on one outside
# the three. It could not fail: the assertion above pins the tier SET to exactly
# `enforce verify `, so by the time the loop ran, every member of that set had
# already been compared against a literal. Deleted rather than kept as belt and
# braces, because an assertion that cannot fail reads like coverage and is not.

# ---- 2: a manual record must NAME a runbook ---------------------------------
# Absent, blank (a nil scalar) and empty are three ways of pointing nowhere, and
# the template refuses all three by name. Each is a separate fixture because a
# check written with `has("runbook")` catches only the first and a check written
# as a truthiness test catches only the last two.
write_record_file "$work/manual-no-runbook.yaml" '{domain: com.example.zebra, key: ZKey, tier: manual}'
write_record_file "$work/manual-blank-runbook.yaml" '{domain: com.example.zebra, key: ZKey, tier: manual, runbook: }'
write_record_file "$work/manual-empty-runbook.yaml" '{domain: com.example.zebra, key: ZKey, tier: manual, runbook: ""}'
for runbook_case in manual-no-runbook manual-blank-runbook manual-empty-runbook; do
  runbook_refusal="$(refute_stream_accepted "$work/$runbook_case.yaml" "a manual record whose runbook is $runbook_case")"
  require_refusal_names "$runbook_refusal" 'runbook' "$runbook_case"
  require_refusal_names "$runbook_refusal" 'com\.example\.zebra' "$runbook_case (the record)"
done

# ---- 3: a manual record must carry NO write payload -------------------------
# One fixture per forbidden field, not one fixture carrying all five. A gate
# written against a single field passes a case that declares all of them, and
# then the other four go unchecked.
for forbidden_on_manual in type value host scope plist_path; do
  write_record_file "$work/manual-carries-$forbidden_on_manual.yaml" \
    "{domain: com.example.zebra, key: ZKey, tier: manual, runbook: \"Section 1\", $forbidden_on_manual: \"x\"}"
  payload_refusal="$(refute_stream_accepted "$work/manual-carries-$forbidden_on_manual.yaml" "a manual record carrying $forbidden_on_manual")"
  require_refusal_names "$payload_refusal" "$forbidden_on_manual" "a manual record carrying $forbidden_on_manual"
  require_refusal_names "$payload_refusal" 'manual' "a manual record carrying $forbidden_on_manual (the tier)"
done

# ---- 4: an enforce record must carry NO runbook -----------------------------
# Nothing consumes a runbook on an enforced control, so one there means the
# declared tier is wrong. The mirror of case 3, and the case that proves the two
# tiers have DIFFERENT tables rather than one shared one.
write_record_file "$work/enforce-carries-runbook.yaml" '{domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce, runbook: "Section 1"}'
enforce_runbook_refusal="$(refute_stream_accepted "$work/enforce-carries-runbook.yaml" "an enforce record carrying a runbook")"
require_refusal_names "$enforce_runbook_refusal" 'runbook' "an enforce record carrying a runbook"
require_refusal_names "$enforce_runbook_refusal" 'enforce' "an enforce record carrying a runbook (the tier)"

# ---- 5: the forbidden-field predicate, called directly ----------------------
# Every cell of both tables, in both directions. A whole-file case can only show
# that SOMETHING refused the file; these say which tier forbids which field, and
# the refute_ direction is what keeps the tables from collapsing into their union.
for forbidden_on_manual in type value host scope plist_path; do
  require_field_forbidden manual "$forbidden_on_manual" "the manual table"
  refute_field_forbidden enforce "$forbidden_on_manual" "the enforce table"
  refute_field_forbidden verify "$forbidden_on_manual" "the verify table"
done
require_field_forbidden enforce runbook "the enforce table"
refute_field_forbidden manual runbook "the manual table"
refute_field_forbidden verify runbook "the verify table"

# The fields every tier is entitled to. Without these the tables could be
# written as "everything is forbidden except what one fixture happened to use".
for always_allowed in domain key tier; do
  for record_tier in enforce verify manual; do
    refute_field_forbidden "$record_tier" "$always_allowed" "the $record_tier table"
  done
done

# An unrecognized tier forbids nothing here, deliberately: the stream's tier gate
# refuses the record first and this predicate must not become a second, weaker
# opinion about which tiers exist.
refute_field_forbidden bogus type "an unrecognized tier"
refute_field_forbidden '' type "an empty tier"

# ---- 6: the runbook-required predicate, called directly ---------------------
require_runbook_required manual "the manual tier"
refute_runbook_required enforce "the enforce tier"
refute_runbook_required verify "the verify tier"
refute_runbook_required bogus "an unrecognized tier"
refute_runbook_required '' "an empty tier"

# ---- 7: the FIRST offending record is the one named -------------------------
# Every other refusal in this library names the first record at fault, and an
# operator fixing a file one record at a time needs the same answer on every run.
# The runbook rule is checked over a map keyed by record index, and iterating
# `"${!map[@]}"` walks bash's hash order rather than declaration order: measured
# on this fixture, that named the LAST offending record. A single-record fixture
# cannot see the difference, which is why this one carries two.
cat >"$work/two-offending-manual-records.yaml" <<'EOF'
macos:
  defaults:
    - {domain: com.example.aaa, key: AKey, value: "1", type: bool, tier: enforce}
    - {domain: com.example.bbb, key: BKey, tier: manual}
    - {domain: com.example.ccc, key: CKey, tier: manual}
  killall: []
EOF
ordering_refusal="$(refute_stream_accepted "$work/two-offending-manual-records.yaml" "a file with two runbook-less manual records")"
require_refusal_names "$ordering_refusal" 'com\.example\.bbb' "two runbook-less manual records (the FIRST is named)"
if printf '%s' "$ordering_refusal" | grep -qF 'com.example.ccc'; then
  fail "the refusal names the LAST offending record rather than the first, so the answer depends on hash order: $ordering_refusal"
fi

# ---- 8: a field NAME that cannot be carried is refused, not ignored ----------
# The facts these rules are decided from arrive as unit-separated lines, one per
# declared field, so a field NAME containing a newline or a unit separator
# forges lines in that stream the same way a record VALUE carrying one forges
# records in the record stream. The difference is what it could hide: this
# reader decides "does this record carry a forbidden field" by comparing names,
# and a name it cannot read reliably is a comparison it cannot make.
#
# Refused rather than skipped, and this is a DELIBERATE divergence in the safe
# direction: the runner template ignores a field it does not know about, so it
# renders both fixtures below. A checker that cannot check must not pass, and
# the invariant this family protects is one-directional, so the library being
# the stricter reader here is the choice, not an accident.
#
# Mutation-checked: the field-count guard is not defence in depth, it is the
# only thing standing between these files and a rules pass that read a forged
# field name.
printf 'macos:\n  defaults:\n    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce, "a\\nb": 1}\n  killall: []\n' \
  >"$work/newline-field-name.yaml"
printf 'macos:\n  defaults:\n    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce, "a\\x1fb": 1}\n  killall: []\n' \
  >"$work/unit-separator-field-name.yaml"
# The separator has to survive into the PARSED key, and asserting that is not
# ceremony: both fixtures reach the byte through YAML's own double-quoted escape
# rather than through printf, so the file on disk holds the escape text and a
# check on the file's bytes would answer no for a fixture that is perfectly
# correct. Ask the parser. (The near-miss worth recording: writing the separator
# with printf instead needs `\x1f` or `\0037`, because bash printf reads `\037`
# as a backslash followed by three literal characters.)
for escaped_field_name_case in newline-field-name unit-separator-field-name; do
  yq eval -r ".macos.defaults[0] | keys | .[]" "$work/$escaped_field_name_case.yaml" |
    grep -qU -e $'\x1f' -e '^a$' ||
    fail "the $escaped_field_name_case fixture does not parse to a field name carrying a newline or a unit separator, so it does not reach the guard it exists to pin"
done
for hostile_field_name_case in newline-field-name unit-separator-field-name; do
  # The fixture only pins anything while the record list itself stays healthy:
  # that is what makes the forged name the reason for the refusal.
  hostile_record_count="$(yq eval -r '.macos.defaults | length' "$work/$hostile_field_name_case.yaml")"
  [[ $hostile_record_count == 1 ]] ||
    fail "the $hostile_field_name_case fixture no longer holds one record (yq answered $hostile_record_count), so it does not reach the rules stream"
  hostile_refusal="$(refute_stream_accepted "$work/$hostile_field_name_case.yaml" "a record declaring a field name containing a separator ($hostile_field_name_case)")"
  require_refusal_names "$hostile_refusal" 'field name' "$hostile_field_name_case"
done

# ---- 8b: a field name that forges a COMPLETE line ---------------------------
# The case the per-line field count cannot catch, and the reason this stream
# needs a per-FILE line count beside it. A field name carrying a newline AND
# three unit separators splits into two lines that BOTH hold exactly four fields,
# so every one of them passes the per-line guard while the second is a fabricated
# record fact.
#
# Measured on the tree that had only the per-line count: the file below, a manual
# record with NO runbook plus one field named `x`, newline, `0`, US, `manual`,
# US, `true`, US, `z`, streamed with status 0 here while the runner template
# refused it with `manual record com.example.zebra ZKey has no runbook`. The
# forged line claimed the record's runbook was usable and, being last, won.
printf 'macos:\n  defaults:\n    - {domain: com.example.zebra, key: ZKey, tier: manual, "x\\n0\\x1fmanual\\x1ftrue\\x1fz": 1}\n  killall: []\n' \
  >"$work/forged-runbook-fact.yaml"
# The forgery has to still BE a forgery: both halves must survive into the parsed
# key, and the record list must still be the healthy one-record list that makes
# the forged fact the only reason the file is refused.
forged_field_name="$(yq eval -r '.macos.defaults[0] | keys | .[] | select(. != "domain" and . != "key" and . != "tier")' "$work/forged-runbook-fact.yaml")"
[[ $forged_field_name == *$'\n'* && $forged_field_name == *$'\x1f'* ]] ||
  fail "the forged-runbook-fact fixture no longer parses to a field name carrying BOTH a newline and a unit separator, so it no longer forges a complete line"
forged_record_count="$(yq eval -r '.macos.defaults | length' "$work/forged-runbook-fact.yaml")"
[[ $forged_record_count == 1 ]] ||
  fail "the forged-runbook-fact fixture no longer holds one record (yq answered $forged_record_count), so it does not reach the rules stream"
forged_refusal="$(refute_stream_accepted "$work/forged-runbook-fact.yaml" "a record whose field name forges a complete rules line")"
require_refusal_names "$forged_refusal" 'field' "a forged rules line"

# The same forgery aimed at the record INDEX rather than at the runbook fact, on
# an otherwise legitimate ENFORCE record. Its point is the MESSAGE: before the
# line count ran first, the reader believed the forged index and refused a
# perfectly good file while naming `record 99`, a record the file does not have,
# and then asked yq about that index and printed the `null` artifact back.
printf 'macos:\n  defaults:\n    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce, "x\\n99\\x1fmanual\\x1ffalse\\x1fz": 1}\n  killall: []\n' \
  >"$work/forged-record-index.yaml"
forged_index_refusal="$(refute_stream_accepted "$work/forged-record-index.yaml" "a record whose field name forges a record index")"
require_refusal_names "$forged_index_refusal" 'field' "a forged record index"
if printf '%s' "$forged_index_refusal" | grep -qE 'record 99|domain null'; then
  fail "the refusal describes a record the file does not have, so the forged index was believed: $forged_index_refusal"
fi

# The false-positive direction for case 7, and the one that keeps the field-count
# guard from being written as "refuse any record with an unfamiliar field". An
# EXTRA field with an ordinary name is not this defect: neither reader minds it,
# and the tracked file gains fields over time.
write_record_file "$work/extra-plain-field.yaml" '{domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce, note: "a comment field"}'
require_stream_accepted "$work/extra-plain-field.yaml" 1 "a record carrying an extra plainly-named field"

# The line-count invariant's own false-positive direction: a file with SEVERAL
# records, each declaring a different number of fields, must still stream. A
# count written per record rather than over the file, or one that assumed every
# record declares the same fields, breaks exactly here.
cat >"$work/uneven-field-counts.yaml" <<'EOF'
macos:
  defaults:
    - {domain: com.example.aaa, key: AKey, value: "1", type: bool, tier: enforce}
    - {domain: com.example.bbb, key: BKey, value: "1", type: bool, tier: enforce, host: "mac1"}
    - {domain: com.example.ccc, key: CKey, tier: manual, runbook: "Section 1"}
EOF
printf '  killall: []\n' >>"$work/uneven-field-counts.yaml"
require_stream_accepted "$work/uneven-field-counts.yaml" 3 "three records declaring five, six and four fields"

printf 'macos-defaults-tier-payload-guard: OK (every tier legitimate record and the real tracked file still stream; a manual record with an absent, blank or empty runbook is refused by name; each of the five fields forbidden on a manual record is refused on its own; an enforce record carrying a runbook is refused; a field name that forges a complete four-field rules line is refused rather than believed; both predicate tables are pinned cell by cell in both directions)\n'
