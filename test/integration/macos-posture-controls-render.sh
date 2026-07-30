#!/usr/bin/env bash
# macos-posture-controls-render.sh -- the posture-controls declaration file
# (.chezmoidata/macos_posture_controls.yaml) is rendered by
# dot_local/libexec/osquery/posture-controls.json.tmpl into the JSON the
# security-posture poller reads at runtime. Two properties are pinned here:
#
#   1. AGREEMENT: the rendered JSON is exactly the declared record list (via a
#      yq conversion of the same YAML), so what the poller consumes IS what the
#      data file declares; there is no second list to drift.
#   2. REFUSAL, fail-closed: a record that is malformed or mis-tiered ABORTS
#      the render (and with it the apply), naming the offender. A verify-only
#      file must reject enforce and manual records outright: the poller only
#      READS controls, so a record carrying any other tier is lying about what
#      consumes it. Absent, blank, and wrong-value tiers are three distinct
#      refusals (a blank YAML scalar is nil and would otherwise stringify to
#      the literal text <nil>). Missing/malformed/duplicate ids, unknown
#      readers, out-of-domain expects, and missing descriptions abort too.
#
# Real chezmoi and yq; renders into a sandbox HOME. Never applies anything.
#
# This test writes LITERAL fixture text with $-free content only, but keeps the
# refusal-message fragments exact.
set -euo pipefail

# Scrubbed at SCRIPT scope, before any git or chezmoi call. Git exports GIT_DIR
# to every hook it runs and this suite can still inherit one from its caller.
unset MACOS_DEFAULTS_SOURCE_DIR GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATE="$REPO_ROOT/dot_local/libexec/osquery/posture-controls.json.tmpl"
DATA_FILE="$REPO_ROOT/.chezmoidata/macos_posture_controls.yaml"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

assert_file_contains() { # <file> <fixed-string> <message>
  grep -qF -- "$2" "$1" || fail "$3"
}

for tool in chezmoi yq jq; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'SKIP: %s not on PATH; cannot exercise the posture-controls render\n' "$tool"
    exit 0
  }
done
[[ -f $TEMPLATE ]] || fail "missing template: $TEMPLATE"
[[ -f $DATA_FILE ]] || fail "missing data file: $DATA_FILE"

# Canonicalize away macOS's /var -> /private/var symlink.
sandbox="$(cd "$(mktemp -d)" && pwd -P)"
trap 'rm -rf "$sandbox"' EXIT
render_home="$sandbox/render-home"
mkdir -p "$render_home"
render_error="$sandbox/render.err"

# make_source -- create a chezmoi source tree whose one data file is read from
# stdin; print the tree's path.
make_source() {
  local source_dir
  source_dir="$(mktemp -d "$sandbox/posture-src.XXXXXX")"
  mkdir -p "$source_dir/.chezmoidata"
  cat >"$source_dir/.chezmoidata/macos_posture_controls.yaml"
  printf '%s\n' "$source_dir"
}

render_template() { # <source-dir> <out-file> <err-file>
  HOME="$render_home" chezmoi --source "$1" execute-template --no-tty <"$TEMPLATE" >"$2" 2>"$3"
}

# assert_rejects <label> <stderr-fragment...> -- feed a fixture on stdin,
# require the render to FAIL, and require the message to carry every named
# fragment.
assert_rejects() {
  local label="$1"
  shift
  local source_dir out_file status=0 fragment
  source_dir="$(make_source)"
  out_file="$sandbox/rejected"
  render_template "$source_dir" "$out_file" "$render_error" || status=$?
  [[ $status -ne 0 ]] ||
    fail "$label: the render must fail (got 0, render: $(cat "$out_file"))"
  for fragment in "$@"; do
    assert_file_contains "$render_error" "$fragment" \
      "$label: the failure must name '$fragment' (stderr: $(cat "$render_error"))"
  done
}

# ---- 1: agreement -- the real data renders to exactly the declared records ---

real_source="$(make_source <"$DATA_FILE")"
render_template "$real_source" "$sandbox/rendered.json" "$render_error" ||
  fail "the real macos_posture_controls.yaml must render (stderr: $(cat "$render_error"))"
jq -e 'type == "array" and length > 0' "$sandbox/rendered.json" >/dev/null 2>&1 ||
  fail "the render must be a non-empty JSON array; got: $(cat "$sandbox/rendered.json")"
yq -o=json '.macos.posture_controls' "$DATA_FILE" | jq -S . >"$sandbox/declared.json" ||
  fail "yq could not convert the declared records"
jq -S . "$sandbox/rendered.json" >"$sandbox/rendered.sorted.json" ||
  fail "the render is not valid JSON"
diff -u "$sandbox/declared.json" "$sandbox/rendered.sorted.json" >&2 ||
  fail "the rendered JSON must equal the declared record list byte-for-byte (after key sort); the render and the data have drifted apart"

# Every declared record is verify: the repo data itself must never carry
# another tier (the render would refuse it, but pin the data too).
bad_tiers="$(jq -r '.[] | select(.tier != "verify") | .id' "$sandbox/rendered.json")"
[[ -z $bad_tiers ]] || fail "non-verify tier(s) in the shipped data: $bad_tiers"

# ---- 2: refusal, fail-closed ------------------------------------------------

# The top-level shape first: a present-but-blank key (nil) and a
# present-but-empty list both used to render CLEANLY (hasKey passes, and range
# over nil or an empty list is zero iterations), deploying a file that
# monitors nothing. A blank control set is not an empty control set: both
# shapes must abort the render.
assert_rejects 'blank posture_controls key' 'is blank' <<EOF
macos:
  posture_controls:
EOF

assert_rejects 'empty posture_controls list' 'declares zero controls' <<EOF
macos:
  posture_controls: []
EOF

# A valid record comes FIRST in each fixture, so a template that warned and
# skipped the offender instead of aborting would render cleanly, and the
# required render failure is what catches it.
valid_record='    - id: filevault
      description: "FileVault disk encryption"
      tier: verify
      reader: fdesetup_status
      expect: "on"'

assert_rejects 'enforce-tier record' 'guest' 'tier "enforce"' 'not verify' <<EOF
macos:
  posture_controls:
$valid_record
    - id: guest
      description: "The macOS Guest account"
      tier: enforce
      reader: sysadminctl_guest
      expect: "disabled"
EOF

assert_rejects 'manual-tier record' 'guest' 'tier "manual"' 'not verify' <<EOF
macos:
  posture_controls:
$valid_record
    - id: guest
      description: "The macOS Guest account"
      tier: manual
      reader: sysadminctl_guest
      expect: "disabled"
EOF

assert_rejects 'record with no tier' 'guest' 'has no tier' <<EOF
macos:
  posture_controls:
$valid_record
    - id: guest
      description: "The macOS Guest account"
      reader: sysadminctl_guest
      expect: "disabled"
EOF

assert_rejects 'record with a blank tier' 'guest' 'blank tier' <<EOF
macos:
  posture_controls:
$valid_record
    - id: guest
      description: "The macOS Guest account"
      tier:
      reader: sysadminctl_guest
      expect: "disabled"
EOF

assert_rejects 'record with no id' 'has no id' <<EOF
macos:
  posture_controls:
$valid_record
    - description: "The macOS Guest account"
      tier: verify
      reader: sysadminctl_guest
      expect: "disabled"
EOF

assert_rejects 'record with a malformed id' 'Guest Account' 'not lowercase' <<EOF
macos:
  posture_controls:
$valid_record
    - id: "Guest Account"
      description: "The macOS Guest account"
      tier: verify
      reader: sysadminctl_guest
      expect: "disabled"
EOF

assert_rejects 'record whose id collides with a built-in poller field' 'firewall' 'built-in' <<EOF
macos:
  posture_controls:
$valid_record
    - id: firewall
      description: "A collision with the legacy firewall field"
      tier: verify
      reader: sysadminctl_guest
      expect: "disabled"
EOF

assert_rejects 'duplicate id' 'duplicate id' 'filevault' <<EOF
macos:
  posture_controls:
$valid_record
    - id: filevault
      description: "A second FileVault record"
      tier: verify
      reader: fdesetup_status
      expect: "on"
EOF

assert_rejects 'unknown reader' 'guest' 'unknown reader' 'wombat_status' <<EOF
macos:
  posture_controls:
$valid_record
    - id: guest
      description: "The macOS Guest account"
      tier: verify
      reader: wombat_status
      expect: "disabled"
EOF

assert_rejects 'out-of-domain expect' 'guest' 'outside the sysadminctl_guest domain' <<EOF
macos:
  posture_controls:
$valid_record
    - id: guest
      description: "The macOS Guest account"
      tier: verify
      reader: sysadminctl_guest
      expect: "wombat"
EOF

assert_rejects 'record with no description' 'guest' 'has no description' <<EOF
macos:
  posture_controls:
$valid_record
    - id: guest
      tier: verify
      reader: sysadminctl_guest
      expect: "disabled"
EOF

# A boolean expect (unquoted YAML true) stringifies to "true", which lands
# outside every reader domain; the render must refuse it rather than quietly
# monitor for a value no reader ever returns. (Unquoted `on`/`off` are STRINGS
# under chezmoi's YAML 1.2 parser, verified empirically, so those are fine.)
assert_rejects 'boolean expect' 'filevault2' 'outside the fdesetup_status domain' <<EOF
macos:
  posture_controls:
$valid_record
    - id: filevault2
      description: "A FileVault record with a boolean expect"
      tier: verify
      reader: fdesetup_status
      expect: true
EOF

printf 'ok: posture-controls render agreement and fail-closed refusal\n'
