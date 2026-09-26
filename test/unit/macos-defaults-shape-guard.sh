#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIB="$REPO_ROOT/scripts/macos-defaults/helpers/defaults-records.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

readonly SUPERSEDED_TAG_ONLY_EXPRESSION='(.macos.defaults // []) | tag'
readonly SUPERSEDED_KIND_ONLY_EXPRESSION='.macos.defaults | kind'

readonly MISTAGGED_SEQUENCE_TAGS=(
  '!!map' '!!str' '!!int' '!!bool' '!!float' '!!binary' '!!set' '!!timestamp'
  '!!omap' '!!pairs' '!!merge' '!custom' '!!foo' '!<tag:example.com,2026:thing>'
)

readonly PLAIN_SEQUENCE_TAGS=('' '!!seq' '!')

require_superseded_tag_check_satisfied() {
  local path="$1" superseded_answer
  superseded_answer="$(yq eval -r "$SUPERSEDED_TAG_ONLY_EXPRESSION" "$path")" ||
    fail "could not evaluate the superseded tag-only expression against $path"
  [[ $superseded_answer == '!!seq' ]] ||
    fail "fixture $path answers $superseded_answer to the superseded tag-only check, so it does not reproduce the hole this case exists to pin"
}

require_superseded_kind_check_satisfied() {
  local path="$1" superseded_answer
  superseded_answer="$(yq eval -r "$SUPERSEDED_KIND_ONLY_EXPRESSION" "$path")" ||
    fail "could not evaluate the superseded kind-only expression against $path"
  [[ $superseded_answer == 'seq' ]] ||
    fail "fixture $path answers kind $superseded_answer to the superseded kind-only check, so it does not reproduce the mirror hole this case exists to pin"
}

refute_shape_accepted() {
  local path="$1" description="$2" status=0 output
  output="$(defaults_records_declared_count "$path" 2>&1)" || status=$?
  [[ $status -eq 2 ]] ||
    fail "$description must be refused with status 2, got $status (output: $output)"
  printf '%s' "$output" | grep -q '\.macos\.defaults' ||
    fail "$description was refused without naming .macos.defaults, so the operator cannot tell what to edit: $output"
  printf '%s' "$output"
}

refute_output_contains() {
  local needle="$1" haystack="$2" message="$3"
  if printf '%s' "$haystack" | grep -qF -- "$needle"; then
    fail "$message"
  fi
}

require_shape_accepted() {
  local path="$1" expected_count="$2" description="$3" status=0 count
  count="$(defaults_records_declared_count "$path" 2>&1)" || status=$?
  [[ $status -eq 0 ]] ||
    fail "$description must be accepted, got status $status ($count)"
  [[ $count == "$expected_count" ]] ||
    fail "$description must count $expected_count record(s), got: $count"
}

require_verdict() {
  local shape_answer="$1" expected_verdict="$2" description="$3" verdict
  verdict="$(records_declaration_verdict "$shape_answer")"
  [[ $verdict == "$expected_verdict" ]] ||
    fail "$description must classify as $expected_verdict, got $verdict (input: $(printf '%q' "$shape_answer"))"
}

write_record_list() {
  printf 'macos:\n  defaults:%s\n    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}\n    - {domain: com.example.alpha, key: AKey, value: "1", type: bool, tier: enforce}\n' \
    "${2:+ $2}" >"$1"
}

[[ -f $LIB ]] || fail "missing library: $LIB"
command -v yq >/dev/null 2>&1 || fail "yq is not on PATH; brew install yq (or run: just setup)"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# shellcheck source=/dev/null
source "$LIB" >/dev/null 2>&1

for plain_tag in "${PLAIN_SEQUENCE_TAGS[@]}"; do
  write_record_list "$work/list.yaml" "$plain_tag"
  require_shape_accepted "$work/list.yaml" 2 "a real two-record list tagged ${plain_tag:-(untagged)}"
done

cat >"$work/empty-list.yaml" <<'EOF'
macos:
  defaults: []
EOF
require_shape_accepted "$work/empty-list.yaml" 0 "an explicitly empty record list"

cat >"$work/tagged-map.yaml" <<'EOF'
macos:
  defaults: !!seq
    zebra: {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}
    alpha: {domain: com.example.alpha, key: AKey, value: "1", type: bool, tier: enforce}
EOF
require_superseded_tag_check_satisfied "$work/tagged-map.yaml"
tagged_map_refusal="$(refute_shape_accepted "$work/tagged-map.yaml" "a !!seq-tagged map")"
printf '%s' "$tagged_map_refusal" | grep -qi 'order' ||
  fail "the refusal of a !!seq-tagged map does not say that the two readers would apply records in different orders: $tagged_map_refusal"

cat >"$work/plain-map.yaml" <<'EOF'
macos:
  defaults:
    zebra: {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}
    alpha: {domain: com.example.alpha, key: AKey, value: "1", type: bool, tier: enforce}
EOF
plain_map_refusal="$(refute_shape_accepted "$work/plain-map.yaml" "a plain map")"
printf '%s' "$plain_map_refusal" | grep -qi 'order' ||
  fail "the refusal of a plain map does not say that the two readers would apply records in different orders: $plain_map_refusal"

cat >"$work/tagged-scalar.yaml" <<'EOF'
macos:
  defaults: !!seq "abc"
EOF
require_superseded_tag_check_satisfied "$work/tagged-scalar.yaml"
tagged_scalar_refusal="$(refute_shape_accepted "$work/tagged-scalar.yaml" "a !!seq-tagged scalar")"
refute_output_contains 'unusable record count' "$tagged_scalar_refusal" \
  "a !!seq-tagged scalar was refused by the COUNT check, not the shape check, so this case pins the wrong guard: $tagged_scalar_refusal"

for mistagged_tag in "${MISTAGGED_SEQUENCE_TAGS[@]}"; do
  write_record_list "$work/mistagged.yaml" "$mistagged_tag"
  require_superseded_kind_check_satisfied "$work/mistagged.yaml"
  mistagged_refusal="$(refute_shape_accepted "$work/mistagged.yaml" "a real sequence tagged $mistagged_tag")"
  printf '%s' "$mistagged_refusal" | grep -qi 'tag' ||
    fail "the refusal of a sequence tagged $mistagged_tag does not mention the tag, so the operator is sent to rewrite records that are already correct: $mistagged_refusal"
done

require_verdict 'seq !!seq' list "a plain sequence"
require_verdict 'seq !' list "a sequence wearing the non-specific tag, which yq >= 4.53.6 reports verbatim"
require_verdict 'map !!map' map "a plain mapping"
require_verdict 'map !!seq' map "a mapping wearing a !!seq tag"
require_verdict 'scalar !!seq' other "a scalar wearing a !!seq tag"
require_verdict 'scalar !!str' other "a plain string"
require_verdict $'seq !!seq\n---\nseq !!seq' other "yq's per-document answers for a multi-document file"
require_verdict '' other "an empty answer, which is what yq prints when .macos is not a mapping"
require_verdict 'seq' other "an answer missing its tag field"
require_verdict 'seq !!seq extra' other "an answer carrying an unexpected third field"

for mistagged_tag in "${MISTAGGED_SEQUENCE_TAGS[@]}"; do
  require_verdict "seq $mistagged_tag" mistagged "a real sequence wearing $mistagged_tag"
done

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
