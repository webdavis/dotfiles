#!/usr/bin/env bash
# macos-defaults-declaration-guard.sh, a data file that declares NO record list
# must be refused, and the one legitimate way to declare an empty one must keep
# working.
#
# `(.macos.defaults // [])` turned a missing key into an empty list, so five
# distinct file states collapsed into one clean answer of zero records and an
# exit status of 0: no .macos key, a .macos that is not a mapping, no .defaults
# key, an explicitly null defaults, an empty file, and the one LEGITIMATE state,
# `defaults: []`. Only the last means "track no records". The others mean the
# file lost its records, and the tools then applied nothing and reported
# success, which is indistinguishable from a clean run of a file that genuinely
# tracks nothing. That is a fail-open on a declarative settings file: the
# settings stop being applied and nothing says so.
#
# The one route that must not be lost from the record here, because it is the
# one an operator reaches by accident: some editors write a UTF-8 byte order
# mark without asking. Two facts about it, both measured rather than assumed:
#
#   A BOM before a NESTED key is not stripped by anything. `macos:` followed by
#   a BOM'd `defaults:` line makes yq read the key as "﻿defaults", so
#   .macos.defaults is genuinely absent and the file read as zero records and
#   succeeded (measured, yq v4.53.3). Case 7 is that file.
#
#   A BOM at the START of the document is stripped by yq v4.53.3 and NOT by
#   chezmoi v2.71.1, whose Go YAML reader keeps it bound into the first key.
#   Measured on the same file: yq answers `macos` to `keys` and reads every
#   record, while `chezmoi execute-template` dies with `map has no entry for key
#   "macos"`. So the file did not read as zero records here, it read as a full
#   set of records that the runner template refuses outright, leaving this
#   library the MORE PERMISSIVE of the file's two readers. Case 8 is that file,
#   and test/integration/macos-defaults-shape-agreement.sh holds the two readers
#   against each other on it.
#
# A BOM is REFUSED rather than stripped. Stripping would leave this library
# reading a file the runner template will not read, which is the asymmetry being
# closed, and it cannot help case 7 anyway, since that mark is not at the start.
# A BOM is also legitimate content anywhere other than the first three bytes: a
# mark inside a record value survives into the value (measured), so there is no
# safe general strip.
#
# The shape of the record list itself (a map or a scalar wearing a `!!seq` tag)
# is pinned by test/unit/macos-defaults-shape-guard.sh.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIB="$REPO_ROOT/dot_local/libexec/macos-defaults/helpers/defaults-records.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# The pair of expressions this guard replaced, and the answers that made a file
# with no record list read as a clean success.
readonly SUPERSEDED_SHAPE_EXPRESSION='(.macos.defaults // []) | tag'
readonly SUPERSEDED_COUNT_EXPRESSION='(.macos.defaults // []) | length'

# require_superseded_read_as_zero_records <path>, fail unless this fixture
# reproduces the hole: the superseded expressions must answer a clean `!!seq`
# and a count of 0, which is what let the tools apply nothing and exit 0.
#
# Asserted on every absent-declaration fixture. Without it a fixture that
# quietly stopped reproducing the fail-open would leave a green test guarding
# nothing.
require_superseded_read_as_zero_records() { # <path>
  local path="$1" superseded_shape superseded_count
  superseded_shape="$(yq eval -r "$SUPERSEDED_SHAPE_EXPRESSION" "$path")" ||
    fail "could not evaluate the superseded shape expression against $path"
  superseded_count="$(yq eval -r "$SUPERSEDED_COUNT_EXPRESSION" "$path")" ||
    fail "could not evaluate the superseded count expression against $path"
  [[ $superseded_shape == '!!seq' && $superseded_count == '0' ]] ||
    fail "fixture $path answers shape $superseded_shape and count $superseded_count to the superseded expressions, so it does not reproduce the zero-records fail-open this case exists to pin"
}

# refute_declaration_accepted <path> <description>, require
# defaults_records_declared_count to refuse this file with status 2 and print
# the refusal, so a caller can assert on the message.
#
# A helper rather than an inline `if ! ...`: under `set -e` an inverted command
# only decides the test in final position, so a bare negation inside a case body
# is a position lottery.
refute_declaration_accepted() { # <path> <description>
  local path="$1" description="$2" status=0 output
  output="$(defaults_records_declared_count "$path" 2>&1)" || status=$?
  [[ $status -eq 2 ]] ||
    fail "$description must be refused with status 2, got $status (output: $output)"
  printf '%s' "$output"
}

# require_names_the_empty_list_spelling <refusal> <description>, the refusal must
# point at the legitimate way to declare no records. Without it an operator who
# genuinely tracks nothing has been handed a dead end.
require_names_the_empty_list_spelling() { # <refusal> <description>
  local refusal="$1" description="$2"
  printf '%s' "$refusal" | grep -qF 'defaults: []' ||
    fail "the $description refusal does not name \`defaults: []\` as the way to track no records: $refusal"
}

# require_declaration_accepted <path> <expected-count> <description>, the
# false-positive direction: this file must be ACCEPTED and answer the count.
require_declaration_accepted() { # <path> <expected-count> <description>
  local path="$1" expected_count="$2" description="$3" status=0 count
  count="$(defaults_records_declared_count "$path" 2>&1)" || status=$?
  [[ $status -eq 0 ]] ||
    fail "$description must be accepted, got status $status ($count)"
  [[ $count == "$expected_count" ]] ||
    fail "$description must count $expected_count record(s), got: $count"
}

# require_verdict <yq-answer> <expected-verdict> <description>, assert the
# classifier's answer for one yq shape answer, called directly.
require_verdict() { # <yq-answer> <expected-verdict> <description>
  local shape_answer="$1" expected_verdict="$2" description="$3" verdict
  verdict="$(records_declaration_verdict "$shape_answer")"
  [[ $verdict == "$expected_verdict" ]] ||
    fail "$description must classify as $expected_verdict, got $verdict (input: $(printf '%q' "$shape_answer"))"
}

# require_byte_order_mark_detected / refuted, the byte-level predicate, called
# directly. Its answer depends on a BYTE count, and `${#variable}` counts
# CHARACTERS: the mark is 3 bytes but ONE character under a UTF-8 locale
# (measured), so a length taken the obvious way reads one byte and matches any
# file starting with 0xEF. These two helpers pin both directions.
require_byte_order_mark_detected() { # <path> <description>
  data_file_begins_with_byte_order_mark "$1" ||
    fail "$2 must be detected as beginning with a byte order mark"
}

refute_byte_order_mark_detected() { # <path> <description>
  if data_file_begins_with_byte_order_mark "$1"; then
    fail "$2 must NOT be detected as beginning with a byte order mark"
  fi
}

# require_unreadable_path_refused <path> <description>, the OTHER half of the
# predicate's answer for a path whose bytes cannot be read. The predicate says
# "no mark", which read on its own is a fail-open; it is safe only because the
# shape read that follows refuses the same path with status 2 and names it. Both
# halves are asserted together, because it is the PAIRING that is the guarantee
# and either half alone can be broken without the other noticing.
require_unreadable_path_refused() { # <path> <description>
  local path="$1" description="$2" status=0 output
  output="$(defaults_records_declared_count "$path" 2>&1)" || status=$?
  [[ $status -eq 2 ]] ||
    fail "$description must be refused with status 2, got $status (output: $output)"
  printf '%s' "$output" | grep -qF -- "$path" ||
    fail "$description was refused without naming the path, so an operator cannot tell which file the tools could not read: $output"
}

# require_yq_strips_byte_order_mark / refutes, the measurement the byte-0-only
# scope RESTS on, asserted against yq rather than assumed. This guard exists to
# close positions where the two readers treat a mark DIFFERENTLY, and byte 0 is
# the only such position: elsewhere neither reader strips it, so both bind it
# into the following key and agree. If a future yq starts stripping a mark
# further into the file, that agreement ends and a byte-0 check is no longer
# complete; these two helpers are what say so.
require_yq_strips_byte_order_mark() { # <path> <description>
  local keys
  keys="$(yq eval -r 'keys | join(",")' "$1")" ||
    fail "could not read the top-level keys of $1"
  case $keys in
    *"$UTF8_BYTE_ORDER_MARK"*)
      fail "$2: yq no longer strips it, so this file is no longer a position where the two readers disagree and the guard's scope needs remeasuring (keys: $(printf '%q' "$keys"))"
      ;;
  esac
}

refute_yq_strips_byte_order_mark() { # <path> <description>
  local keys
  keys="$(yq eval -r 'keys | join(",")' "$1")" ||
    fail "could not read the top-level keys of $1"
  case $keys in
    *"$UTF8_BYTE_ORDER_MARK"*) ;;
    *)
      fail "$2: yq stripped it, so a mark in this position is now a reader DIVERGENCE that a byte-0 check does not catch (keys: $(printf '%q' "$keys"))"
      ;;
  esac
}

[[ -f $LIB ]] || fail "missing library: $LIB"
command -v yq >/dev/null 2>&1 || fail "yq is not on PATH; run inside the nix dev shell"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# shellcheck source=/dev/null
source "$LIB" >/dev/null 2>&1

# ---- 1: a real list is still accepted --------------------------------------
# The control. Every refusal below passes against a guard that refuses
# unconditionally, which would break every tracked setting on this machine.
cat >"$work/list.yaml" <<'EOF'
macos:
  defaults:
    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}
    - {domain: com.example.alpha, key: AKey, value: "1", type: bool, tier: enforce}
EOF
require_declaration_accepted "$work/list.yaml" 2 "a real two-record list"

# ---- 2: an explicitly EMPTY list is still accepted --------------------------
# The state the refusals below must not swallow, and the reason "no records" and
# "no record list" have to be two answers rather than one. An operator is
# entitled to track nothing, and says so with this exact spelling.
cat >"$work/empty-list.yaml" <<'EOF'
macos:
  defaults: []
EOF
require_declaration_accepted "$work/empty-list.yaml" 0 "an explicitly empty record list"

# ---- 3 to 6: a file that declares NO record list is refused -----------------
# All four used to reach the superseded expressions as a clean, empty `!!seq`,
# so all four read as zero records and exited 0 while applying nothing.
printf 'other: 1\n' >"$work/absent-macos.yaml"
printf 'macos:\n  killall: []\n' >"$work/absent-defaults.yaml"
printf 'macos:\n  defaults:\n' >"$work/null-defaults.yaml"
: >"$work/empty-file.yaml"

for absent_case in absent-macos absent-defaults null-defaults empty-file; do
  require_superseded_read_as_zero_records "$work/$absent_case.yaml"
  absent_refusal="$(refute_declaration_accepted "$work/$absent_case.yaml" "a file whose records are $absent_case")"
  printf '%s' "$absent_refusal" | grep -q '\.macos\.defaults' ||
    fail "the $absent_case refusal does not name .macos.defaults, so the operator cannot tell what to edit: $absent_refusal"
  require_names_the_empty_list_spelling "$absent_refusal" "$absent_case"
done

# ---- 7: a BOM before a NESTED key is refused --------------------------------
# The accidental route. The mark binds into the key, so `.macos.defaults` is
# genuinely absent while the file still looks correct in an editor. Nothing
# strips a mark in this position, so this file read as zero records and
# succeeded.
printf 'macos:\n  \xef\xbb\xbfdefaults:\n    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}\n' \
  >"$work/nested-byte-order-mark.yaml"
require_superseded_read_as_zero_records "$work/nested-byte-order-mark.yaml"
nested_mark_refusal="$(refute_declaration_accepted "$work/nested-byte-order-mark.yaml" "a byte order mark before the defaults key")"
require_names_the_empty_list_spelling "$nested_mark_refusal" "nested byte order mark"

# ---- 8: a DOCUMENT-START byte order mark is refused --------------------------
# A different defect with the same cause. yq strips this mark and reads every
# record; chezmoi's Go YAML reader does not, so the runner template cannot find
# .macos at all. The file therefore does NOT reach the absent-declaration
# refusal (yq sees a healthy list), which is why the byte-level check exists,
# and why this case asserts that the mark itself is named: an operator whose
# editor added three invisible bytes has no other way to find them.
printf '\xef\xbb\xbfmacos:\n  defaults:\n    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}\n' \
  >"$work/leading-byte-order-mark.yaml"
leading_shape="$(yq eval -r "$SUPERSEDED_SHAPE_EXPRESSION" "$work/leading-byte-order-mark.yaml")"
leading_count="$(yq eval -r "$SUPERSEDED_COUNT_EXPRESSION" "$work/leading-byte-order-mark.yaml")"
[[ $leading_shape == '!!seq' && $leading_count == '1' ]] ||
  fail "the leading-mark fixture no longer reproduces the divergence: yq answers shape $leading_shape and count $leading_count, so this case is not pinning a file yq reads and the runner template refuses"
leading_mark_refusal="$(refute_declaration_accepted "$work/leading-byte-order-mark.yaml" "a document-start byte order mark")"
printf '%s' "$leading_mark_refusal" | grep -qi 'byte order mark' ||
  fail "the refusal does not name the byte order mark, so the operator cannot see what to remove: $leading_mark_refusal"

# ---- 9: a .macos that is not a mapping is refused ----------------------------
# yq answers NOTHING at all here rather than a null node, so this is not the
# absent case and must not be told to write `defaults: []`. It is pinned because
# the superseded expressions read it as a clean zero too.
printf 'macos: 5\n' >"$work/macos-not-a-mapping.yaml"
require_superseded_read_as_zero_records "$work/macos-not-a-mapping.yaml"
not_a_mapping_refusal="$(refute_declaration_accepted "$work/macos-not-a-mapping.yaml" "a .macos that is not a mapping")"
if printf '%s' "$not_a_mapping_refusal" | grep -qF 'defaults: []'; then
  fail "the refusal tells the operator to write \`defaults: []\`, which will not fix a .macos that is not a mapping: $not_a_mapping_refusal"
fi

# ---- 10: the classifier's absent verdict, called directly -------------------
# The whole-file cases cannot separate these two: both a missing key and an
# explicit null arrive as the same yq answer, and a fifth file state would too.
require_verdict 'scalar !!null' absent "the answer yq gives for a missing or null node"
require_verdict 'seq !!seq' list "a real sequence, which the absent verdict must not swallow"
require_verdict 'scalar !!seq' other "a scalar wearing a !!seq tag, which is not the absent case"
require_verdict '' other "the empty answer yq gives when .macos is not a mapping"

# ---- 11: the byte-order-mark predicate, called directly ---------------------
# Its correctness is a BYTE comparison, and every plausible mistake in it makes
# it answer yes too often rather than too rarely, so the negative direction
# carries this block.
require_byte_order_mark_detected "$work/leading-byte-order-mark.yaml" "a file that starts with the mark"
refute_byte_order_mark_detected "$work/list.yaml" "a plain ASCII data file"
refute_byte_order_mark_detected "$work/nested-byte-order-mark.yaml" "a file whose mark is not at the start"
refute_byte_order_mark_detected "$work/empty-file.yaml" "an empty file"
# A file whose first byte is the mark's first byte but which is not a mark. A
# length taken as `${#mark}` under a UTF-8 locale counts ONE character, so a
# one-byte comparison would call this a mark.
printf '\xef\xac\x81rst: 1\n' >"$work/first-byte-collision.yaml"
refute_byte_order_mark_detected "$work/first-byte-collision.yaml" "a file starting with a different 0xEF sequence"
# Shorter than the mark, so the read returns fewer bytes than it compares.
printf '\xef\xbb' >"$work/truncated-mark.yaml"
refute_byte_order_mark_detected "$work/truncated-mark.yaml" "a file holding only the first two bytes of the mark"

# ---- 12: the predicate's READ-FAILURE branch, and what makes it safe ---------
# A path whose bytes cannot be read answers "no mark", the same as a clean file,
# because a predicate that cannot read the bytes cannot claim to have found one.
# Read alone that is a fail-open, and nothing pinned either the answer or the
# refusal that redeems it: flipping the branch to answer "mark found" would have
# misdiagnosed every missing or unreadable data file as a marked one, told the
# operator to delete three bytes that are not there, and left every case above
# green.
#
# A missing path and a directory both fail the read for every user. A mode-000
# file does not fail it for root, so that fixture is skipped there rather than
# asserted falsely.
mkdir -p "$work/a-directory"
printf 'macos:\n  defaults: []\n' >"$work/unreadable.yaml"
chmod 000 "$work/unreadable.yaml"

refute_byte_order_mark_detected "$work/missing.yaml" "a path that does not exist"
require_unreadable_path_refused "$work/missing.yaml" "a path that does not exist"
refute_byte_order_mark_detected "$work/a-directory" "a path that is a directory"
require_unreadable_path_refused "$work/a-directory" "a path that is a directory"
if [[ $EUID -ne 0 ]]; then
  refute_byte_order_mark_detected "$work/unreadable.yaml" "a file this user cannot read"
  require_unreadable_path_refused "$work/unreadable.yaml" "a file this user cannot read"
fi
chmod 644 "$work/unreadable.yaml"

# ---- 13: byte 0 is the whole of the divergence this guard closes -------------
# The scope claim, measured rather than assumed. The guard looks at the first
# three bytes only, which reads like an arbitrary narrowing; it is not one,
# because byte 0 is the only position where the two readers treat a mark
# differently. yq strips it there and chezmoi does not (case 8 above holds that
# half). ANYWHERE ELSE yq does not strip it either: it binds into the following
# key exactly as chezmoi's reader does, so the two agree and there is no
# divergence for a byte-level guard to close.
#
# Pinned from the yq side because that is the half that can change under us. A
# yq release that started stripping a mid-file mark would reopen a real
# divergence that a byte-0 check cannot see, and it would do it silently.
require_yq_strips_byte_order_mark "$work/leading-byte-order-mark.yaml" \
  "a mark at byte 0"
printf 'macos:\n  defaults: []\n\xef\xbb\xbfother: 1\n' >"$work/mark-before-later-key.yaml"
refute_yq_strips_byte_order_mark "$work/mark-before-later-key.yaml" \
  "a mark at the start of a later line"
printf 'macos:\n  defaults: []\n  \xef\xbb\xbfkillall: []\n' >"$work/mark-after-indent.yaml"
yq eval -r '.macos | keys | join(",")' "$work/mark-after-indent.yaml" |
  grep -qF "$UTF8_BYTE_ORDER_MARK" ||
  fail "yq now strips a mark that follows the indent of a nested key, so a mark in that position is a reader divergence a byte-0 check does not catch"

printf 'macos-defaults-declaration-guard: OK (an explicitly empty list still answers 0; a file that declares no record list is refused and pointed at an explicitly empty one; a byte order mark is refused whether it hides the defaults key or breaks the runner template; an unreadable path answers no-mark and is still refused by name; byte 0 is still the only position the two readers treat differently)\n'
