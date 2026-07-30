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

printf 'macos-defaults-shape-agreement: OK (both readers take a list in declaration order and both refuse a map, naming the shape)\n'
