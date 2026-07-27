#!/usr/bin/env bash
# macos-defaults-system-scope.sh -- system-domain support across all four
# consumers of macos_defaults.yaml: the Tier 1 runner template and the three
# tools (apply, capture, drift), plus the shared library's record stream.
#
# A record may carry an optional `scope` field: `user` (the default, and the
# meaning of an absent field) or `system` (a root-owned plist, written through
# sudo). The properties pinned here, one per acceptance criterion:
#
#   1. With no system-scope record the rendered runner is SEMANTICALLY identical
#      to the pre-slice render and contains no `sudo`. Byte-identity was the
#      original claim and it no longer holds, deliberately: hardening every data
#      field from Go's %q double-quoting to POSIX single-quoting changes the
#      quote character on every write line. Both goldens are kept below, and the
#      two are run under identical stubs so the argument vectors `defaults` and
#      `killall` receive must match exactly. Argument vectors are what the claim
#      was ever about; the quote characters are not.
#   2. With a system-scope record the rendered runner contains exactly one
#      `sudo -v`, before any write, and that record's write is `sudo defaults
#      write '/Library/Preferences/<domain>' ...`.
#   3. An explicit ABSOLUTE plist_path is written instead of the default.
#   4. A RELATIVE plist_path is rejected (render fails; apply refuses), never
#      resolved against the ambient working directory.
#   5. drift reports an unreadable system-scope record as indeterminate, with a
#      marker distinct from <unset>, and does NOT count it as drift.
#   6. drift on a set-and-matching system-scope record reports no drift.
#   7. capture --scope system appends a record carrying `scope: system`.
#   8. capture rejects --scope system combined with --host current.
#   9. The template's scope guard tolerates records WITHOUT the field (the
#      `.scope` field form would abort the render of every existing record;
#      criterion 1's golden render is the behavioral pin, the source grep the
#      belt).
#
# Real chezmoi and yq; `defaults`, `sudo`, `osascript`, and `killall` are
# stubbed. Never runs real sudo, never touches /Library.
set -euo pipefail

# Scrubbed at SCRIPT scope, before any git or chezmoi call. Git exports GIT_DIR
# to every hook it runs and this suite runs from the pre-push hook; the
# library's own override may be exported on a developer machine.
unset MACOS_DEFAULTS_SOURCE_DIR GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl"
LIB="$REPO_ROOT/dot_local/bin/macos-defaults-lib.sh"
APPLY="$REPO_ROOT/dot_local/bin/executable_macos-defaults-apply.sh"
CAPTURE="$REPO_ROOT/dot_local/bin/executable_macos-defaults-capture.sh"
DRIFT="$REPO_ROOT/dot_local/bin/executable_macos-defaults-drift.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# refute_file_contains <file> <fixed-string> <message> -- the explicit negative
# assertion. A bare `! grep` is dead under `set -e` unless it happens to be the
# last statement, so every negative below goes through this helper.
refute_file_contains() { # <file> <fixed-string> <message>
  if grep -qF -- "$2" "$1"; then
    fail "$3"
  fi
}

assert_file_contains() { # <file> <fixed-string> <message>
  grep -qF -- "$2" "$1" || fail "$3"
}

# Host-tool guard: the de-homebrewed CI-faithful run has no chezmoi/yq on PATH,
# and this test cannot render templates or read records without them.
for tool in chezmoi yq; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'SKIP: %s not on PATH; cannot exercise system-scope support\n' "$tool"
    exit 0
  }
done
for required_file in "$TEMPLATE" "$LIB" "$APPLY" "$CAPTURE" "$DRIFT"; do
  [[ -f $required_file ]] || fail "missing file: $required_file"
done

# Canonicalize away macOS's /var -> /private/var symlink.
sandbox="$(cd "$(mktemp -d)" && pwd -P)"
trap 'chmod -R u+rwX "$sandbox" 2>/dev/null; rm -rf "$sandbox"' EXIT
mkdir -p "$sandbox/home"
render_home="$sandbox/render-home"
mkdir -p "$render_home"

# make_source_dir -- create a chezmoi source tree whose macos_defaults.yaml is
# read from stdin; prints the tree's path.
make_source_dir() {
  local source_dir
  source_dir="$(mktemp -d "$sandbox/src.XXXXXX")"
  mkdir -p "$source_dir/.chezmoidata"
  cat >"$source_dir/.chezmoidata/macos_defaults.yaml"
  printf '%s\n' "$source_dir"
}

# render_template <source-dir> <out-file> <err-file> -- render the Tier 1
# runner template against one source tree; returns chezmoi's status.
render_template() { # <source-dir> <out-file> <err-file>
  HOME="$render_home" chezmoi --source "$1" execute-template --no-tty <"$TEMPLATE" >"$2" 2>"$3"
}

# ---- criterion 1: user-only data renders with unchanged behavior ------------

user_only_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.alpha
      key: AlphaKey
      type: bool
      value: true
    - domain: com.example.beta
      key: BetaKey
      type: string
      value: BetaValue
      host: current
  killall:
    - Dock
    - cfprefsd
EOF
)"

# The PRE-SLICE golden: main's template (commit 5d70c94) rendered against the
# fixture above, byte for byte including the trailing blank line. It is no
# longer the byte-target, it is the BEHAVIORAL reference: the section below runs
# it and the current render under the same stubs and compares what `defaults`
# and `killall` actually receive.
pre_slice_golden="$sandbox/golden-user-only-pre-slice"
cat >"$pre_slice_golden" <<'EOF'
#!/bin/bash
# Tier 1, macOS user defaults runner.
# chezmoi hash-gates on the rendered template body; this script re-runs only
# when .chezmoidata/macos_defaults.yaml changes.

set -euo pipefail

# Pre-flight: close System Settings if open. macOS caches plist values inside
# Settings and writes them back on close, silently overwriting our writes.
osascript -e 'tell application "System Settings" to quit' 2>/dev/null || true

# Main loop: one `defaults write` per record.
defaults write "com.example.alpha" "AlphaKey" -bool "true"
defaults -currentHost write "com.example.beta" "BetaKey" -string "BetaValue"
# Post-loop: restart user-facing processes so changes take effect immediately.
# cfprefsd kill is non-negotiable (caches plist values in memory).
killall "Dock" 2>/dev/null || true
killall "cfprefsd" 2>/dev/null || true

EOF

# The CURRENT golden: the same fixture through the hardened template. Every data
# field is POSIX single-quoted, so bash performs no expansion inside it at all.
# Keeping this as a byte pin means any future change to the quoting shows up as
# a diff in this file rather than as a silent change in what gets executed.
hardened_golden="$sandbox/golden-user-only-hardened"
cat >"$hardened_golden" <<'EOF'
#!/bin/bash
# Tier 1, macOS user defaults runner.
# chezmoi hash-gates on the rendered template body; this script re-runs only
# when .chezmoidata/macos_defaults.yaml changes.

set -euo pipefail

# Pre-flight: close System Settings if open. macOS caches plist values inside
# Settings and writes them back on close, silently overwriting our writes.
osascript -e 'tell application "System Settings" to quit' 2>/dev/null || true

# Main loop: one `defaults write` per record.
defaults write 'com.example.alpha' 'AlphaKey' -bool 'true'
defaults -currentHost write 'com.example.beta' 'BetaKey' -string 'BetaValue'
# Post-loop: restart user-facing processes so changes take effect immediately.
# cfprefsd kill is non-negotiable (caches plist values in memory).
killall 'Dock' 2>/dev/null || true
killall 'cfprefsd' 2>/dev/null || true

EOF

rendered="$sandbox/rendered-user-only"
render_error="$sandbox/render.err"
render_template "$user_only_src" "$rendered" "$render_error" ||
  fail "user-only render must succeed (stderr: $(cat "$render_error"))"
if [[ -z "$(tr -d '[:space:]' <"$rendered")" ]]; then
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi
cmp -s "$hardened_golden" "$rendered" ||
  fail "user-only render must match the hardened golden (diff: $(diff "$hardened_golden" "$rendered" | head -20))"
refute_file_contains "$rendered" 'sudo' \
  'user-only render must contain no sudo invocation'

# Semantic identity with the pre-slice render, the claim byte-identity was
# standing in for. Both scripts run under the same stubs and the recorded
# argument vectors must be identical: same commands, same arguments, same order.
# The fixture is benign, so a quoting change that altered ANY argument, or
# reordered anything, shows up here.
equivalence_bin="$sandbox/equivalence-bin"
mkdir -p "$equivalence_bin"
for stubbed_command in defaults killall osascript; do
  cat >"$equivalence_bin/$stubbed_command" <<EOF
#!/bin/bash
printf '$stubbed_command'
printf ' [%s]' "\$@"
printf '\n'
exit 0
EOF
  chmod +x "$equivalence_bin/$stubbed_command"
done
run_render_capturing_arguments() { # <script> <out-file>
  (
    cd "$sandbox" || exit 1
    PATH="$equivalence_bin:$PATH" bash "$1"
  ) >"$2" 2>&1
}
run_render_capturing_arguments "$pre_slice_golden" "$sandbox/argv-pre-slice" ||
  fail 'the pre-slice golden must run cleanly under the stubs'
run_render_capturing_arguments "$rendered" "$sandbox/argv-hardened" ||
  fail 'the hardened render must run cleanly under the stubs'
cmp -s "$sandbox/argv-pre-slice" "$sandbox/argv-hardened" ||
  fail "the hardened render must pass byte-identical arguments to every command (diff: $(diff "$sandbox/argv-pre-slice" "$sandbox/argv-hardened" | head -20))"
[[ -s $sandbox/argv-hardened ]] ||
  fail 'the argument-vector capture is empty; the comparison above would pass on two dead runs'

# The repo's REAL data file declares no system-scope record in this slice, so
# the shipped render must contain no sudo either.
rendered_real="$sandbox/rendered-real-data"
render_template "$REPO_ROOT" "$rendered_real" "$render_error" ||
  fail "render against the repo's real data must succeed (stderr: $(cat "$render_error"))"
refute_file_contains "$rendered_real" 'sudo' \
  "render against the repo's real data must contain no sudo invocation"

# Criterion 9, source form: the guard reads the optional field through
# `index . "scope"`; the `.scope` field form aborts on every record without
# the field (the renders above already prove that behaviorally).
assert_file_contains "$TEMPLATE" 'index . "scope"' \
  'the template scope guard must read the field through index . "scope"'
if grep -qE '\.scope' "$TEMPLATE"; then
  fail 'the template must not use the .scope field form anywhere'
fi

# ---- criterion 2: a system record adds one sudo -v, before any write --------

system_default_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.alpha
      key: AlphaKey
      type: bool
      value: true
    - domain: com.example.sys
      key: SysKey
      type: bool
      value: false
      scope: system
  killall:
    - cfprefsd
EOF
)"

rendered_system="$sandbox/rendered-system"
render_template "$system_default_src" "$rendered_system" "$render_error" ||
  fail "system-record render must succeed (stderr: $(cat "$render_error"))"

sudo_validate_count="$(grep -cxF 'sudo -v' "$rendered_system" || true)"
[[ $sudo_validate_count -eq 1 ]] ||
  fail "system-record render must contain exactly one 'sudo -v' (got $sudo_validate_count)"
sudo_validate_line="$(grep -nxF 'sudo -v' "$rendered_system" | cut -d: -f1)"
first_write_line="$(grep -nE '^(sudo )?defaults ' "$rendered_system" | head -1 | cut -d: -f1)"
[[ -n $first_write_line ]] || fail 'system-record render must contain defaults writes'
[[ $sudo_validate_line -lt $first_write_line ]] ||
  fail "the sudo -v prelude must come before any write (sudo -v at line $sudo_validate_line, first write at line $first_write_line)"
grep -qxF "sudo defaults write '/Library/Preferences/com.example.sys' 'SysKey' -bool 'false'" "$rendered_system" ||
  fail "the system record's write must be sudo-prefixed and target /Library/Preferences/<domain> (got: $(grep -F 'com.example.sys' "$rendered_system"))"
grep -qxF "defaults write 'com.example.alpha' 'AlphaKey' -bool 'true'" "$rendered_system" ||
  fail 'the user record must keep its plain, un-sudoed write'
refute_file_contains "$rendered_system" "sudo defaults write 'com.example.alpha'" \
  'the user record must not be written through sudo'

# ---- criterion 3: an explicit absolute plist_path wins over the default -----

explicit_path_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.lulu
      key: LuLuKey
      type: string
      value: block
      scope: system
      plist_path: /Library/Objective-See/LuLu/preferences.plist
  killall: []
EOF
)"

rendered_explicit="$sandbox/rendered-explicit"
render_template "$explicit_path_src" "$rendered_explicit" "$render_error" ||
  fail "explicit-path render must succeed (stderr: $(cat "$render_error"))"
grep -qxF "sudo defaults write '/Library/Objective-See/LuLu/preferences.plist' 'LuLuKey' -string 'block'" "$rendered_explicit" ||
  fail "an explicit absolute plist_path must be the write target (got: $(grep -F LuLuKey "$rendered_explicit"))"
refute_file_contains "$rendered_explicit" '/Library/Preferences/com.example.lulu' \
  'an explicit plist_path must replace the /Library/Preferences default, not coexist with it'

# ---- criterion 4: a relative plist_path fails the render --------------------

relative_path_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.rel
      key: RelKey
      type: bool
      value: true
      scope: system
      plist_path: Library/Preferences/rel.plist
  killall: []
EOF
)"

relative_status=0
render_template "$relative_path_src" "$sandbox/rendered-relative" "$render_error" || relative_status=$?
[[ $relative_status -ne 0 ]] ||
  fail 'a relative plist_path must fail the render, never resolve silently'
assert_file_contains "$render_error" 'Library/Preferences/rel.plist' \
  "the relative-path render failure must name the offending path (stderr: $(cat "$render_error"))"

# A system record combined with host is meaningless (ByHost storage is
# per-user) and must fail the render rather than render something half-right.
system_host_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.syshost
      key: SysHostKey
      type: bool
      value: true
      scope: system
      host: current
  killall: []
EOF
)"
system_host_status=0
render_template "$system_host_src" "$sandbox/rendered-system-host" "$render_error" || system_host_status=$?
[[ $system_host_status -ne 0 ]] ||
  fail 'a system-scope record with a host must fail the render'

# An unknown scope must fail the render, not silently fall back to user scope.
bogus_scope_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.bogus
      key: BogusKey
      type: bool
      value: true
      scope: bogus
  killall: []
EOF
)"
bogus_scope_status=0
render_template "$bogus_scope_src" "$sandbox/rendered-bogus" "$render_error" || bogus_scope_status=$?
[[ $bogus_scope_status -ne 0 ]] ||
  fail 'an unknown scope must fail the render'
assert_file_contains "$render_error" 'unknown scope' \
  "the unknown-scope render failure must say so (stderr: $(cat "$render_error"))"

# ---- the shared record stream: one line, seven fields, empties survive ------

# The unit separator (0x1f) is not IFS whitespace, so an empty INTERIOR field
# (here: host absent while scope and plist_path follow it) must survive the
# read intact. Under a tab-separated stream the empty host would collapse and
# shift scope left into host, which is the regression this section pins.
columns_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.full
      key: FullKey
      type: string
      value: FullValue
      scope: system
      plist_path: /Library/Objective-See/full.plist
    - domain: com.example.minimal
      key: MinimalKey
      type: bool
      value: true
    - domain: com.example.byhost
      key: ByHostKey
      type: int
      value: 7
      host: current
  killall: []
EOF
)"
record_stream="$(bash -c 'source "$1"; defaults_records_unit_separated "$2"' _ \
  "$LIB" "$columns_src/.chezmoidata/macos_defaults.yaml")" ||
  fail 'defaults_records_unit_separated must succeed on a well-formed file'

IFS=$'\x1f' read -r got_domain got_key got_type got_value got_host got_scope got_plist_path \
  <<<"$(printf '%s\n' "$record_stream" | sed -n '1p')"
[[ $got_domain == com.example.full ]] || fail "record stream: field 1 must be the domain (got '$got_domain')"
[[ $got_key == FullKey ]] || fail "record stream: field 2 must be the key (got '$got_key')"
[[ $got_type == string ]] || fail "record stream: field 3 must be the type (got '$got_type')"
[[ $got_value == FullValue ]] || fail "record stream: field 4 must be the value (got '$got_value')"
[[ -z $got_host ]] ||
  fail "record stream: an absent host must survive as an EMPTY interior field, not collapse (got '$got_host')"
[[ $got_scope == system ]] || fail "record stream: field 6 must be the scope (got '$got_scope')"
[[ $got_plist_path == /Library/Objective-See/full.plist ]] ||
  fail "record stream: field 7 must be the plist path (got '$got_plist_path')"

IFS=$'\x1f' read -r got_domain got_key got_type got_value got_host got_scope got_plist_path \
  <<<"$(printf '%s\n' "$record_stream" | sed -n '2p')"
[[ $got_domain == com.example.minimal ]] || fail "record stream: minimal field 1 must be the domain (got '$got_domain')"
[[ -z $got_host ]] || fail "record stream: minimal host must be empty (got '$got_host')"
[[ $got_scope == user ]] ||
  fail "record stream: an ABSENT scope must default to user in the stream (got '$got_scope')"
[[ -z $got_plist_path ]] || fail "record stream: minimal plist path must be empty (got '$got_plist_path')"

IFS=$'\x1f' read -r got_domain got_key got_type got_value got_host got_scope got_plist_path \
  <<<"$(printf '%s\n' "$record_stream" | sed -n '3p')"
[[ $got_host == current ]] || fail "record stream: field 5 must be the host (got '$got_host')"
[[ $got_scope == user ]] || fail "record stream: byhost scope must default to user (got '$got_scope')"

# ---- tool stubs --------------------------------------------------------------

stub_bin="$sandbox/bin"
mkdir -p "$stub_bin"
defaults_log="$sandbox/defaults.log"
sudo_log="$sandbox/sudo.log"

# sudo stub: records the space-joined invocation, then runs it (so the
# defaults stub behind it still answers). Fixture values contain no spaces, so
# the space-joined log parses back per field unambiguously.
cat >"$stub_bin/sudo" <<EOF
#!/bin/bash
printf '%s\n' "\$*" >>"$sudo_log"
exec "\$@"
EOF
printf '#!/bin/bash\nexit 0\n' >"$stub_bin/osascript"
printf '#!/bin/bash\nexit 0\n' >"$stub_bin/killall"
chmod +x "$stub_bin/sudo" "$stub_bin/osascript" "$stub_bin/killall"

# write_defaults_stub <write-only|read-value|read-unset|read-denied|capture-bool>
# -- the per-section behavior of the `defaults` stub. Every variant logs its
# space-joined invocation first.
# SC2016: the single-quoted $1 lines are stub SOURCE, expanded when the stub
# runs, deliberately not here.
# shellcheck disable=SC2016
write_defaults_stub() {
  local mode="$1"
  {
    printf '#!/bin/bash\n'
    printf 'printf "%%s\\n" "$*" >>"%s"\n' "$defaults_log"
    case "$mode" in
      write-only)
        printf 'exit 0\n'
        ;;
      read-value)
        printf 'if [[ $1 == read ]]; then printf "1\\n"; exit 0; fi\nexit 0\n'
        ;;
      read-unset)
        printf 'if [[ $1 == read ]]; then printf "does not exist\\n" >&2; exit 1; fi\nexit 0\n'
        ;;
      read-denied)
        printf 'if [[ $1 == read ]]; then printf "Operation not permitted\\n" >&2; exit 1; fi\nexit 0\n'
        ;;
      capture-bool)
        printf 'if [[ $1 == read-type ]]; then printf "Type is boolean\\n"; exit 0; fi\n'
        printf 'if [[ $1 == read ]]; then printf "1\\n"; exit 0; fi\nexit 0\n'
        ;;
    esac
  } >"$stub_bin/defaults"
  chmod +x "$stub_bin/defaults"
}

# run_tool <source-dir> <script> [args...] -- run one tool against one source
# tree, stubs first on PATH. Runs from inside the sandbox so a relative-path
# bug that resolved against the working directory would land in the sandbox,
# observable and contained.
run_tool() { # <source-dir> <script> [args...]
  local source_dir="$1"
  shift
  (
    cd "$sandbox" || exit 1
    MACOS_DEFAULTS_SOURCE_DIR="$source_dir" HOME="$sandbox/home" \
      PATH="$stub_bin:$PATH" bash "$@"
  )
}

# ---- apply: system records go through sudo to the resolved path -------------

apply_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.alpha
      key: AlphaKey
      type: bool
      value: true
    - domain: com.example.sys
      key: SysKey
      type: bool
      value: false
      scope: system
    - domain: com.example.lulu
      key: LuLuKey
      type: string
      value: block
      scope: system
      plist_path: /Library/Objective-See/LuLu/preferences.plist
  killall: []
EOF
)"
write_defaults_stub write-only
: >"$defaults_log"
: >"$sudo_log"
run_tool "$apply_src" "$APPLY" || fail 'apply must succeed on a mixed user/system data file'

system_write_line="$(grep -F 'com.example.sys' "$sudo_log" || true)"
[[ -n $system_write_line ]] ||
  fail "apply must route the system record's write through sudo (sudo log: $(cat "$sudo_log"))"
read -r -a system_write_fields <<<"$system_write_line"
[[ ${#system_write_fields[@]} -eq 6 ]] ||
  fail "apply's sudo write must carry exactly 6 arguments (got: $system_write_line)"
[[ ${system_write_fields[0]} == defaults ]] || fail "apply sudo write: field 1 must be 'defaults' (got '${system_write_fields[0]}')"
[[ ${system_write_fields[1]} == write ]] || fail "apply sudo write: field 2 must be 'write' (got '${system_write_fields[1]}')"
[[ ${system_write_fields[2]} == /Library/Preferences/com.example.sys ]] ||
  fail "apply sudo write: field 3 must be the default plist path (got '${system_write_fields[2]}')"
[[ ${system_write_fields[3]} == SysKey ]] || fail "apply sudo write: field 4 must be the key (got '${system_write_fields[3]}')"
[[ ${system_write_fields[4]} == -bool ]] || fail "apply sudo write: field 5 must be the dashed type (got '${system_write_fields[4]}')"
[[ ${system_write_fields[5]} == false ]] || fail "apply sudo write: field 6 must be the value (got '${system_write_fields[5]}')"

assert_file_contains "$sudo_log" '/Library/Objective-See/LuLu/preferences.plist' \
  "apply must write an explicit plist_path record to that path (sudo log: $(cat "$sudo_log"))"
refute_file_contains "$sudo_log" '/Library/Preferences/com.example.lulu' \
  'apply must not compose the default path for a record carrying an explicit plist_path'
assert_file_contains "$defaults_log" 'write com.example.alpha AlphaKey -bool true' \
  "apply must still write the user record (defaults log: $(cat "$defaults_log"))"
refute_file_contains "$sudo_log" 'com.example.alpha' \
  'apply must not route a user-scope write through sudo'

# apply refuses a relative plist_path and never writes it.
apply_relative_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.rel
      key: RelKey
      type: bool
      value: true
      scope: system
      plist_path: Library/Preferences/rel.plist
  killall: []
EOF
)"
: >"$defaults_log"
: >"$sudo_log"
apply_relative_status=0
run_tool "$apply_relative_src" "$APPLY" 2>"$sandbox/apply-relative.err" || apply_relative_status=$?
[[ $apply_relative_status -ne 0 ]] ||
  fail 'apply must refuse a relative plist_path'
assert_file_contains "$sandbox/apply-relative.err" 'absolute path is required' \
  "apply's relative-path refusal must say an absolute path is required (stderr: $(cat "$sandbox/apply-relative.err"))"
refute_file_contains "$defaults_log" 'rel.plist' \
  'apply must not write a relative plist_path record at all'
refute_file_contains "$sudo_log" 'rel.plist' \
  'apply must not sudo-write a relative plist_path record at all'

# apply refuses an unknown scope rather than guessing a write target.
apply_bogus_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.bogus
      key: BogusKey
      type: bool
      value: true
      scope: bogus
  killall: []
EOF
)"
: >"$defaults_log"
apply_bogus_status=0
run_tool "$apply_bogus_src" "$APPLY" 2>"$sandbox/apply-bogus.err" || apply_bogus_status=$?
[[ $apply_bogus_status -ne 0 ]] || fail 'apply must refuse an unknown scope'
refute_file_contains "$defaults_log" 'com.example.bogus' \
  'apply must not write a record whose scope it could not validate'

# ---- drift: three outcomes for a system-scope record -------------------------

drift_system_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.sys
      key: SysKey
      type: bool
      value: true
      scope: system
  killall: []
EOF
)"

# criterion 6: set and matching (the stub reads back 1, the normalized true).
write_defaults_stub read-value
drift_out="$sandbox/drift.out"
drift_err="$sandbox/drift.err"
run_tool "$drift_system_src" "$DRIFT" >"$drift_out" 2>"$drift_err" ||
  fail "drift on a matching system record must exit 0 (stderr: $(cat "$drift_err"))"
[[ ! -s $drift_out ]] ||
  fail "drift on a matching system record must print no rows (got: $(cat "$drift_out"))"
assert_file_contains "$defaults_log" 'read /Library/Preferences/com.example.sys SysKey' \
  "drift must read the system record from its resolved plist path (defaults log: $(cat "$defaults_log"))"

# criterion 5, defaults-failure route: an unknown read failure is indeterminate,
# marked distinctly from <unset>, and NOT counted as drift.
write_defaults_stub read-denied
drift_status=0
run_tool "$drift_system_src" "$DRIFT" >"$drift_out" 2>"$drift_err" || drift_status=$?
[[ $drift_status -eq 0 ]] ||
  fail "an unreadable system record must not count as drift (got exit $drift_status, stderr: $(cat "$drift_err"))"
indeterminate_row="$(grep -F 'com.example.sys' "$drift_out" || true)"
[[ -n $indeterminate_row ]] ||
  fail "an unreadable system record must be reported as its own row, not silently skipped (stdout: $(cat "$drift_out"))"
IFS=$'\t' read -r row_domain row_key row_expected row_actual <<<"$indeterminate_row"
[[ $row_domain == com.example.sys ]] || fail "indeterminate row: field 1 must be the domain (got '$row_domain')"
[[ $row_key == SysKey ]] || fail "indeterminate row: field 2 must be the key (got '$row_key')"
[[ $row_expected == 1 ]] || fail "indeterminate row: field 3 must be the normalized expected value (got '$row_expected')"
[[ $row_actual == '<unreadable>' ]] ||
  fail "indeterminate row: field 4 must be the <unreadable> marker, distinct from <unset> (got '$row_actual')"
refute_file_contains "$drift_out" '<unset>' \
  'an unreadable read must never collapse into <unset>'
assert_file_contains "$drift_err" 'indeterminate' \
  "drift must summarize indeterminate rows on stderr (stderr: $(cat "$drift_err"))"

# criterion 5, unreadable-file route: the plist exists but cannot be read. The
# stub would report a MATCHING value, so a drift that skipped the file check
# would report all-clear here; the correct behavior is distinguishable.
locked_plist="$sandbox/system-plists/locked.plist"
mkdir -p "$(dirname "$locked_plist")"
: >"$locked_plist"
chmod 000 "$locked_plist"
drift_locked_src="$(
  make_source_dir <<EOF
macos:
  defaults:
    - domain: com.example.locked
      key: LockedKey
      type: bool
      value: true
      scope: system
      plist_path: $locked_plist
  killall: []
EOF
)"
write_defaults_stub read-value
drift_status=0
run_tool "$drift_locked_src" "$DRIFT" >"$drift_out" 2>"$drift_err" || drift_status=$?
chmod u+rw "$locked_plist"
[[ $drift_status -eq 0 ]] ||
  fail "an unreadable plist file must not count as drift (got exit $drift_status, stderr: $(cat "$drift_err"))"
assert_file_contains "$drift_out" '<unreadable>' \
  "an existing-but-unreadable plist must be reported indeterminate even when a read would have answered (stdout: $(cat "$drift_out"))"

# A genuinely unset system record IS drift, with the <unset> marker.
write_defaults_stub read-unset
drift_status=0
run_tool "$drift_system_src" "$DRIFT" >"$drift_out" 2>"$drift_err" || drift_status=$?
[[ $drift_status -eq 1 ]] ||
  fail "a genuinely unset system record must count as drift (got exit $drift_status)"
unset_row="$(grep -F 'com.example.sys' "$drift_out" || true)"
IFS=$'\t' read -r row_domain row_key row_expected row_actual <<<"$unset_row"
[[ $row_actual == '<unset>' ]] ||
  fail "an unset system record's row must carry the <unset> marker (got '$row_actual')"

# drift refuses an unknown scope rather than misreading a record.
drift_bogus_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.bogus
      key: BogusKey
      type: bool
      value: true
      scope: bogus
  killall: []
EOF
)"
write_defaults_stub read-value
drift_status=0
run_tool "$drift_bogus_src" "$DRIFT" >"$drift_out" 2>"$drift_err" || drift_status=$?
[[ $drift_status -eq 2 ]] ||
  fail "drift must exit 2 on a record whose scope it cannot validate (got $drift_status)"

# ---- capture: --scope system --------------------------------------------------

# criterion 7. The fixture already tracks the SAME domain/key at user scope, so
# a duplicate check that ignored scope would answer "already tracked" and skip
# the append; the correct behavior appends a second, system-scope record.
capture_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.cap
      key: CapKey
      type: bool
      value: true
  killall: []
EOF
)"
capture_data_file="$capture_src/.chezmoidata/macos_defaults.yaml"
write_defaults_stub capture-bool
: >"$defaults_log"
run_tool "$capture_src" "$CAPTURE" com.example.cap CapKey --scope system ||
  fail 'capture --scope system must append alongside an existing user-scope record'
captured_record="$(yq eval -r \
  '.macos.defaults[] | select(.domain == "com.example.cap" and .key == "CapKey" and ((.scope // "user") == "system")) | [.domain, .key, .type, .value, .scope] | join("|")' \
  "$capture_data_file")"
[[ $captured_record == 'com.example.cap|CapKey|bool|true|system' ]] ||
  fail "capture --scope system must append a record carrying scope: system (got '$captured_record')"
user_record_count="$(yq eval -r \
  '[.macos.defaults[] | select(.domain == "com.example.cap" and .key == "CapKey" and ((.scope // "user") == "user"))] | length' \
  "$capture_data_file")"
[[ $user_record_count -eq 1 ]] ||
  fail "capture --scope system must leave the user-scope record in place (got $user_record_count)"
assert_file_contains "$defaults_log" 'read-type /Library/Preferences/com.example.cap CapKey' \
  "capture --scope system must read the live value from the system plist path (defaults log: $(cat "$defaults_log"))"

# criterion 8: --scope system with --host current is rejected as malformed.
capture_reject_src="$(
  make_source_dir <<'EOF'
macos:
  defaults: []
  killall: []
EOF
)"
capture_reject_data_file="$capture_reject_src/.chezmoidata/macos_defaults.yaml"
cp "$capture_reject_data_file" "$sandbox/capture-reject.before"
capture_status=0
run_tool "$capture_reject_src" "$CAPTURE" com.example.cap CapKey --scope system --host current \
  2>"$sandbox/capture-reject.err" || capture_status=$?
[[ $capture_status -eq 3 ]] ||
  fail "capture --scope system --host current must exit 3, the malformed-args status (got $capture_status)"
assert_file_contains "$sandbox/capture-reject.err" '--scope system' \
  "the rejection must name the conflicting flags (stderr: $(cat "$sandbox/capture-reject.err"))"
cmp -s "$sandbox/capture-reject.before" "$capture_reject_data_file" ||
  fail 'a rejected capture must leave the data file untouched'

# A set-but-empty --scope is rejected, not treated as unset.
capture_status=0
run_tool "$capture_reject_src" "$CAPTURE" com.example.cap CapKey --scope '' \
  2>"$sandbox/capture-empty.err" || capture_status=$?
[[ $capture_status -eq 3 ]] ||
  fail "capture --scope '' must exit 3 (got $capture_status)"

# The single-token form is parsed too, and an unknown scope value is rejected.
capture_status=0
run_tool "$capture_reject_src" "$CAPTURE" com.example.cap CapKey --scope=bogus \
  2>"$sandbox/capture-bogus.err" || capture_status=$?
[[ $capture_status -eq 3 ]] ||
  fail "capture --scope=bogus must exit 3 (got $capture_status)"

printf 'macos-defaults-system-scope: OK (user-only render matches the hardened golden, passes byte-identical arguments to the pre-slice render, and contains no sudo; one sudo -v before any write; explicit absolute plist_path honored, relative rejected everywhere; drift distinguishes value/<unset>/<unreadable> and never counts indeterminate as drift; capture appends scope: system and rejects the --host pairing)\n'
