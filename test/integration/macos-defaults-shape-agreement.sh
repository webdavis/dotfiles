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

printf 'macos-defaults-shape-agreement: OK (both readers take a list in declaration order, both refuse a map naming the shape, both refuse a file carrying a byte order mark, and across %d tags on a real sequence the library never accepts a file the template refuses)\n' \
  "${#TAGS_ON_A_REAL_SEQUENCE[@]}"
