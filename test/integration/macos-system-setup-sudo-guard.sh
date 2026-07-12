#!/usr/bin/env bash
# macos-system-setup-sudo-guard.sh -- the Tier-2 runner template
# (run_onchange_after_41-macos-system-setup) must guard its per-record sudo prefix
# with `{{ if index . "sudo" }}`, NOT `{{ if .sudo }}`. Go's text/template throws
# "map has no entry for key \"sudo\"" on the `.field` form when a system_setup
# record omits the sudo key; the `index` form returns the empty value (falsy) and
# renders no prefix. This is the documented Tier-1/Tier-2 runner gotcha.
#
# Renders the REAL template against fixture chezmoidata carrying a record WITHOUT a
# sudo key alongside one WITH sudo: true, and asserts: the render succeeds, the
# keyless record's command is emitted with NO sudo prefix, and the sudo: true
# record still gets its prefix.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_onchange_after_41-macos-system-setup.sh.tmpl"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

command -v chezmoi >/dev/null 2>&1 || {
  printf 'SKIP: chezmoi not on PATH; cannot render after_41\n'
  exit 0
}
[[ -f $TEMPLATE ]] || fail "missing template: $TEMPLATE"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

src="$work/src"
mkdir -p "$src/.chezmoiscripts" "$src/.chezmoidata"
cp "$TEMPLATE" "$src/.chezmoiscripts/"
# A record with NO sudo key (the throwing case for `.sudo`) plus one WITH sudo:true.
# No tailnet_pins key: also exercises the absent-key `index` gotcha for the pins.
cat >"$src/.chezmoidata/macos_system_setup.yaml" <<'EOF'
macos:
  system_setup:
    - description: "keyless record (no sudo field)"
      command: "echo nosudo-marker"
    - description: "privileged record"
      command: "echo withsudo-marker"
      sudo: true
EOF

rendered="$work/rendered.sh"
render_home="$work/home"
mkdir -p "$render_home"
# A render FAILURE here is the red state: `.sudo` throws on the keyless record.
HOME="$render_home" CI=1 chezmoi --source "$src" execute-template --no-tty \
  <"$src/.chezmoiscripts/$(basename "$TEMPLATE")" >"$rendered" ||
  fail 'render failed -- the runner throws on a record with no sudo key (use {{ index . "sudo" }}, not {{ .sudo }})'

if [[ ! -s $rendered ]]; then
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi

# The keyless record must be emitted with NO sudo prefix.
grep -qxF 'echo nosudo-marker' "$rendered" ||
  fail "keyless record must render its command with no sudo prefix (rendered: $(cat "$rendered"))"
if grep -qxF 'sudo echo nosudo-marker' "$rendered"; then
  fail "keyless record wrongly got a sudo prefix"
fi
# The sudo:true record must keep its prefix (guard must not drop sudo wholesale).
grep -qxF 'sudo echo withsudo-marker' "$rendered" ||
  fail "sudo:true record lost its sudo prefix (rendered: $(cat "$rendered"))"

printf 'macos-system-setup-sudo-guard: OK (keyless record renders sans sudo; sudo:true keeps its prefix)\n'
