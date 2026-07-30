#!/usr/bin/env bash
# macos-system-setup-sudo-guard.sh -- the Tier 2 runner template reads the
# OPTIONAL per-record `sudo` field through `index`, never the bare dotted-field
# form. Go's text/template errors with `map has no entry for key "sudo"` on the
# field form when a record omits the key, which turned a legitimate record
# without the field into an opaque template panic instead of a render.
#
# The properties pinned, one per acceptance criterion:
#   1. A record with NO sudo key renders, and its command line carries no
#      `sudo ` prefix.
#   2. A `sudo: true` record keeps its `sudo ` prefix.
#   3. A `sudo: false` record renders without the prefix (falsy, not panicking,
#      is the field form's one working case; it must keep working).
#   4. Source form, the belt behind the behavioral pin above: the template
#      reads the field through `index . "sudo"` and never the bare dotted form.
#
# Real chezmoi; nothing is executed, only rendered.
set -euo pipefail

# Scrubbed at SCRIPT scope, before any chezmoi call. Git exports GIT_DIR to
# every hook it runs and this suite can still inherit one from its caller.
unset MACOS_DEFAULTS_SOURCE_DIR GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_onchange_after_41-macos-system-setup.sh.tmpl"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# A bare `! grep` is dead under `set -e` unless it happens to be the last
# statement, so every negative below goes through these helpers.
refute_file_contains() { # <file> <fixed-string> <message>
  if grep -qF -- "$2" "$1"; then
    fail "$3"
  fi
}

refute_file_matches() { # <file> <regex> <message>
  if grep -qE -- "$2" "$1"; then
    fail "$3"
  fi
}

command -v chezmoi >/dev/null 2>&1 || {
  printf 'SKIP: chezmoi not on PATH; cannot render the Tier 2 runner\n'
  exit 0
}
[[ -f $TEMPLATE ]] || fail "missing template: $TEMPLATE"

work="$(cd "$(mktemp -d)" && pwd -P)"
trap 'rm -rf "$work"' EXIT

# One fixture, three records: sudo absent, sudo true, sudo false. Each record
# carries `tier: enforce`, the field the tier-model slice makes required on
# every record.
source_dir="$work/src"
mkdir -p "$source_dir/.chezmoidata"
cat >"$source_dir/.chezmoidata/macos_system_setup.yaml" <<'EOF'
macos:
  system_setup:
    - description: "record without a sudo key"
      command: 'printf no-sudo-key-ran'
      tier: enforce
    - description: "record with sudo true"
      command: 'printf sudo-true-ran'
      sudo: true
      tier: enforce
    - description: "record with sudo false"
      command: 'printf sudo-false-ran'
      sudo: false
      tier: enforce
EOF

render_home="$work/render-home"
mkdir -p "$render_home"
rendered="$work/rendered"
render_error="$work/render.err"
HOME="$render_home" chezmoi --source "$source_dir" execute-template --no-tty \
  <"$TEMPLATE" >"$rendered" 2>"$render_error" ||
  fail "a record without a sudo key must render, not abort the template (stderr: $(cat "$render_error"))"
if [[ -z "$(tr -d '[:space:]' <"$rendered")" ]]; then
  # An empty render is only legitimate OFF darwin. On darwin this template
  # must produce output, so empty means the template broke, and skipping
  # would report a broken render as a pass. Assert the host, do not infer it.
  if [[ $(uname -s) == Darwin ]]; then
    printf 'FAIL: the render came back EMPTY on darwin, where this template must produce output; a broken darwin render must not pass as a skip\n' >&2
    exit 1
  fi
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi

# Criterion 1: no sudo key, no prefix. The -x match pins the WHOLE line, so a
# prefix of any kind fails it; the explicit refute below is the belt for a
# render that emitted the command twice.
grep -qxF 'printf no-sudo-key-ran' "$rendered" ||
  fail "a record without a sudo key must render its bare command (rendered: $(cat "$rendered"))"
refute_file_contains "$rendered" 'sudo printf no-sudo-key-ran' \
  'a record without a sudo key must never gain a sudo prefix'

# Criterion 2: sudo true keeps its prefix.
grep -qxF 'sudo printf sudo-true-ran' "$rendered" ||
  fail "a sudo: true record must keep its sudo prefix (rendered: $(cat "$rendered"))"

# Criterion 3: sudo false renders without the prefix.
grep -qxF 'printf sudo-false-ran' "$rendered" ||
  fail "a sudo: false record must render its bare command (rendered: $(cat "$rendered"))"
refute_file_contains "$rendered" 'sudo printf sudo-false-ran' \
  'a sudo: false record must never gain a sudo prefix'

# Criterion 4, source form: the field is read through index; the bare dotted
# form appears nowhere in the template, comments included, so it cannot creep
# back in behind a passing render.
grep -qF 'index . "sudo"' "$TEMPLATE" ||
  fail 'the template must read the sudo field through index . "sudo"'
refute_file_matches "$TEMPLATE" '\.sudo' \
  'the template must not use the bare dotted sudo field form anywhere'

printf 'macos-system-setup-sudo-guard: OK (a record without a sudo key renders its bare command; sudo: true keeps its prefix; sudo: false stays bare; the field is read through index)\n'
