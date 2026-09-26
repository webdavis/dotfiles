#!/usr/bin/env bash
set -euo pipefail

unset MACOS_DEFAULTS_SOURCE_DIR GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIB="$REPO_ROOT/scripts/macos-defaults/helpers/defaults-records.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -f $LIB ]] || fail "missing lib: $LIB"

work="$(cd "$(mktemp -d)" && pwd -P)"
trap 'chmod -R u+rwX "$work" 2>/dev/null; rm -rf "$work"' EXIT

stub_bin="$work/bin"
mkdir -p "$stub_bin"

call_function() {
  PATH="$stub_bin:$PATH" LIB="$LIB" bash -c 'source "$LIB"; "$@"' _ "$@" 2>"$work/err"
}

status=0
output="$(call_function resolve_system_plist_path com.example.sys '')" || status=$?
[[ $status -eq 0 ]] ||
  fail "default path: resolve_system_plist_path must succeed on an empty declared path (got $status, stderr: $(cat "$work/err"))"
[[ $output == '/Library/Preferences/com.example.sys' ]] ||
  fail "default path: an empty declared path must resolve to /Library/Preferences/<domain> (got '$output')"

status=0
output="$(call_function resolve_system_plist_path com.example.lulu /Library/Objective-See/LuLu/preferences.plist)" || status=$?
[[ $status -eq 0 ]] ||
  fail "absolute path: resolve_system_plist_path must accept an absolute path (got $status, stderr: $(cat "$work/err"))"
[[ $output == '/Library/Objective-See/LuLu/preferences.plist' ]] ||
  fail "absolute path: an absolute declared path must pass through verbatim (got '$output')"

status=0
output="$(call_function resolve_system_plist_path com.example.rel 'Library/Preferences/rel.plist')" || status=$?
[[ $status -ne 0 ]] ||
  fail "relative path: resolve_system_plist_path must reject a relative path (got 0, stdout: '$output')"
grep -qF 'absolute path is required' "$work/err" ||
  fail "relative path: the rejection must say an absolute path is required (stderr: $(cat "$work/err"))"
grep -qF 'Library/Preferences/rel.plist' "$work/err" ||
  fail "relative path: the rejection must name the offending path (stderr: $(cat "$work/err"))"
[[ -z $output ]] ||
  fail "relative path: a rejected path must print nothing on stdout (got '$output')"

status=0
output="$(call_function resolve_system_plist_path com.example.rel './prefs.plist')" || status=$?
[[ $status -ne 0 ]] ||
  fail "dot-relative path: resolve_system_plist_path must reject ./prefs.plist (got 0, stdout: '$output')"

reject_resolved_path() {
  local label="$1" domain="$2" declared_path="$3"
  local reject_status=0 reject_output
  reject_output="$(call_function resolve_system_plist_path "$domain" "$declared_path")" || reject_status=$?
  [[ $reject_status -ne 0 ]] ||
    fail "$label: resolve_system_plist_path must reject domain '$domain' plist_path '$declared_path' (got 0, stdout: '$reject_output')"
  [[ -z $reject_output ]] ||
    fail "$label: a rejected pair must print nothing on stdout (got '$reject_output')"
}

reject_resolved_path 'traversal domain, default path' '../../tmp/owned' ''

reject_resolved_path 'traversal domain, declared path' '../../tmp/owned' \
  '/Library/Preferences/com.example.sys.plist'

reject_resolved_path 'traversal path, interior' com.example.sys '/Library/Preferences/../../etc/x'
reject_resolved_path 'traversal path, bare root parent' com.example.sys '/..'
reject_resolved_path 'traversal path, trailing parent' com.example.sys '/Library/Preferences/..'
reject_resolved_path 'traversal path, doubled slashes' com.example.sys '//Library/..//etc/x'

reject_resolved_path 'empty domain' '' ''
reject_resolved_path 'dot domain' '.' ''
reject_resolved_path 'dot-dot domain' '..' ''
reject_resolved_path 'root plist_path' com.example.sys '/'

status=0
output="$(call_function resolve_system_plist_path com.example.lulu '/Library/Objective-See/LuLu/preferences.plist')" || status=$?
[[ $status -eq 0 && $output == '/Library/Objective-See/LuLu/preferences.plist' ]] ||
  fail "LuLu path: the tracked Objective-See path must stay accepted verbatim (got status $status, output '$output')"

status=0
output="$(call_function validate_record_scope user '' '')" || status=$?
[[ $status -eq 0 && $output == user ]] ||
  fail "scope user: must validate and print 'user' (got status $status, output '$output')"

status=0
output="$(call_function validate_record_scope system '' '')" || status=$?
[[ $status -eq 0 && $output == system ]] ||
  fail "scope system: must validate and print 'system' (got status $status, output '$output')"

status=0
output="$(call_function validate_record_scope user current '')" || status=$?
[[ $status -eq 0 && $output == user ]] ||
  fail "scope user + host: the existing pairing must stay valid (got status $status, output '$output')"

status=0
output="$(call_function validate_record_scope bogus '' '')" || status=$?
[[ $status -ne 0 ]] ||
  fail "unknown scope: 'bogus' must be rejected (got 0, output '$output')"
grep -qF 'unknown scope' "$work/err" ||
  fail "unknown scope: the rejection must say the scope is unknown (stderr: $(cat "$work/err"))"
[[ -z $output ]] ||
  fail "unknown scope: a rejected scope must print nothing on stdout (got '$output')"

status=0
output="$(call_function validate_record_scope '' '' '')" || status=$?
[[ $status -ne 0 ]] ||
  fail "empty scope: a set-but-empty scope must be rejected (got 0, output '$output')"

status=0
output="$(call_function validate_record_scope system current '')" || status=$?
[[ $status -ne 0 ]] ||
  fail "system + host: the pair must be rejected (got 0, output '$output')"
grep -qF 'per-user' "$work/err" ||
  fail "system + host: the rejection must explain ByHost storage is per-user (stderr: $(cat "$work/err"))"

status=0
output="$(call_function validate_record_scope user '' /Library/Preferences/x.plist)" || status=$?
[[ $status -ne 0 ]] ||
  fail "user + plist_path: the pair must be rejected (got 0, output '$output')"
grep -qF 'plist_path' "$work/err" ||
  fail "user + plist_path: the rejection must name plist_path (stderr: $(cat "$work/err"))"

write_defaults_read_stub() {
  case "$1" in
    ok)
      cat >"$stub_bin/defaults" <<'EOF'
#!/bin/bash
printf '1\n'
exit 0
EOF
      ;;
    unset)
      cat >"$stub_bin/defaults" <<'EOF'
#!/bin/bash
printf 'The domain/default pair of (%s, %s) does not exist\n' "$2" "$3" >&2
exit 1
EOF
      ;;
    denied)
      cat >"$stub_bin/defaults" <<'EOF'
#!/bin/bash
printf 'Operation not permitted\n' >&2
exit 1
EOF
      ;;
  esac
  chmod +x "$stub_bin/defaults"
}

write_defaults_read_stub ok
status=0
output="$(call_function system_defaults_read_actual /Library/Preferences/com.example.sys SysKey)" || status=$?
[[ $status -eq 0 && $output == 1 ]] ||
  fail "read ok: a successful read must print the value (got status $status, output '$output')"

write_defaults_read_stub unset
status=0
output="$(call_function system_defaults_read_actual /Library/Preferences/com.example.sys SysKey)" || status=$?
[[ $status -eq 1 ]] ||
  fail "read unset: a does-not-exist failure must report the unset STATUS 1 (got $status, stderr: $(cat "$work/err"))"
[[ -z $output ]] ||
  fail "read unset: the unset outcome must print no value, so no value can impersonate it (got '$output')"

write_defaults_read_stub denied
status=0
output="$(call_function system_defaults_read_actual /Library/Preferences/com.example.sys SysKey)" || status=$?
[[ $status -eq 2 ]] ||
  fail "read denied: an unknown failure must report the unreadable STATUS 2, never unset (got $status, stderr: $(cat "$work/err"))"

write_defaults_read_stub ok
locked_plist="$work/locked.plist"
: >"$locked_plist"
chmod 000 "$locked_plist"
status=0
output="$(call_function system_defaults_read_actual "$work/locked" SysKey)" || status=$?
[[ $status -eq 2 ]] ||
  fail "read locked (.plist candidate): an existing unreadable <path>.plist must report status 2 (got $status)"
status=0
output="$(call_function system_defaults_read_actual "$locked_plist" SysKey)" || status=$?
[[ $status -eq 2 ]] ||
  fail "read locked (exact path): an existing unreadable plist must report status 2 (got $status)"
chmod u+rw "$locked_plist"

write_defaults_read_stub ok
printf '#!/bin/bash\nexit 1\n' >"$stub_bin/mktemp"
chmod +x "$stub_bin/mktemp"
status=0
output="$(call_function system_defaults_read_actual "$work/no-such-plist" SysKey)" || status=$?
[[ $status -eq 2 ]] ||
  fail "read mktemp failure: a failed mktemp must report the unreadable status 2 (got $status, stderr: $(cat "$work/err"))"
grep -qF 'cannot classify' "$work/err" ||
  fail "read mktemp failure: the refusal must name its reason, not fall through silently (stderr: $(cat "$work/err"))"
if grep -qF 'ambiguous redirect' "$work/err"; then
  fail "read mktemp failure: the temp file must never become an ambiguous redirect (stderr: $(cat "$work/err"))"
fi
[[ -z $output ]] ||
  fail "read mktemp failure: an indeterminate read must print no value (got '$output')"
rm -f "$stub_bin/mktemp"

status=0
double_source_output="$(bash -c 'set -euo pipefail; source "$1"; source "$1"; printf "SURVIVED\n"' _ "$LIB" 2>"$work/err")" || status=$?
[[ $status -eq 0 ]] ||
  fail "double source: sourcing the library twice under set -euo pipefail must succeed (got $status, stderr: $(cat "$work/err"))"
[[ $double_source_output == SURVIVED ]] ||
  fail "double source: the caller must run past the second source (got '$double_source_output')"

status=0
constants_output="$(SYSTEM_READ_OK=9 SYSTEM_READ_UNSET=9 SYSTEM_READ_UNREADABLE=9 \
  bash -c 'set -euo pipefail; source "$1"; source "$1"; printf "%s %s %s\n" "$SYSTEM_READ_OK" "$SYSTEM_READ_UNSET" "$SYSTEM_READ_UNREADABLE"' \
  _ "$LIB" 2>"$work/err")" || status=$?
[[ $status -eq 0 && $constants_output == '0 1 2' ]] ||
  fail "double source: the read-outcome constants must be 0 1 2 even when the environment presets them (got status $status, output '$constants_output')"

for evil_path in /etc/example.evil.plist /Library/LaunchDaemons/com.example.evil.plist; do
  status=0
  output="$(call_function require_system_plist_path_permitted "$evil_path")" || status=$?
  [[ $status -ne 0 ]] ||
    fail "allowlist: $evil_path must be refused as a write target (got 0)"
  grep -qF 'permitted plist director' "$work/err" ||
    fail "allowlist: the refusal must name the containment rule (stderr: $(cat "$work/err"))"
  grep -qF "$evil_path" "$work/err" ||
    fail "allowlist: the refusal must name the offending path (stderr: $(cat "$work/err"))"
done

for permitted_path in /Library/Preferences/com.example.sys /Library/Objective-See/LuLu/preferences.plist; do
  status=0
  call_function require_system_plist_path_permitted "$permitted_path" >/dev/null || status=$?
  [[ $status -eq 0 ]] ||
    fail "allowlist: $permitted_path must be permitted (got $status, stderr: $(cat "$work/err"))"
done

sudo_log="$work/sudo.log"
cat >"$stub_bin/sudo" <<EOF
#!/bin/bash
printf '%s\n' "\$*" >>"$sudo_log"
exit 0
EOF
chmod +x "$stub_bin/sudo"
: >"$sudo_log"
status=0
call_function system_defaults_write /Library/Preferences/com.example.sys SysKey bool false >/dev/null || status=$?
[[ $status -eq 0 ]] ||
  fail "write: system_defaults_write must succeed when sudo succeeds (got $status, stderr: $(cat "$work/err"))"
mapfile -t sudo_calls <"$sudo_log"
[[ ${#sudo_calls[@]} -eq 3 ]] ||
  fail "write: one write must make exactly 3 sudo calls, the write plus its two-step repair (got ${#sudo_calls[@]}: $(cat "$sudo_log"))"
[[ ${sudo_calls[0]} == 'defaults write /Library/Preferences/com.example.sys SysKey -bool false' ]] ||
  fail "write: call 1 must be the exact defaults write (got '${sudo_calls[0]}')"
[[ ${sudo_calls[1]} == 'chown root:wheel /Library/Preferences/com.example.sys.plist' ]] ||
  fail "write: call 2 must repair ownership on the file defaults actually writes, the .plist beside the extensionless path (got '${sudo_calls[1]}')"
[[ ${sudo_calls[2]} == 'chmod 644 /Library/Preferences/com.example.sys.plist' ]] ||
  fail "write: call 3 must repair the mode to 0644 so the unprivileged drift reader can read the value back (got '${sudo_calls[2]}')"

: >"$sudo_log"
status=0
call_function system_defaults_write /Library/Objective-See/LuLu/preferences.plist LuLuKey bool true >/dev/null || status=$?
[[ $status -eq 0 ]] ||
  fail "write .plist: system_defaults_write must succeed (got $status, stderr: $(cat "$work/err"))"
grep -qxF 'chmod 644 /Library/Objective-See/LuLu/preferences.plist' "$sudo_log" ||
  fail "write .plist: the repair must target the declared .plist file itself (log: $(cat "$sudo_log"))"
if grep -qF 'preferences.plist.plist' "$sudo_log"; then
  fail "write .plist: the repair must not append a second .plist (log: $(cat "$sudo_log"))"
fi

cat >"$stub_bin/sudo" <<EOF
#!/bin/bash
printf '%s\n' "\$*" >>"$sudo_log"
if [[ \$1 == defaults ]]; then exit 7; fi
exit 0
EOF
chmod +x "$stub_bin/sudo"
: >"$sudo_log"
status=0
call_function system_defaults_write /Library/Preferences/com.example.absent AbsentKey bool true >/dev/null || status=$?
[[ $status -eq 7 ]] ||
  fail "write failure: the write's own exit status must be re-raised (got $status)"
if grep -qE '^(chown|chmod) ' "$sudo_log"; then
  fail "write failure: no file was left behind, so no repair call may run (log: $(cat "$sudo_log"))"
fi

existing_target="$work/failed-write.plist"
: >"$existing_target"
: >"$sudo_log"
status=0
call_function system_defaults_write "$existing_target" FailKey bool true >/dev/null || status=$?
[[ $status -eq 7 ]] ||
  fail "write failure with file: the write's exit status must be re-raised (got $status)"
grep -qxF "chown root:wheel $existing_target" "$sudo_log" ||
  fail "write failure with file: ownership must still be repaired on the failure path (log: $(cat "$sudo_log"))"
grep -qxF "chmod 644 $existing_target" "$sudo_log" ||
  fail "write failure with file: the mode must still be repaired on the failure path (log: $(cat "$sudo_log"))"

printf 'macos-defaults-scope-read: OK (plist path defaults, passes absolute, rejects relative, traversal and degenerate targets on BOTH branches while the tracked Objective-See path still resolves; re-sourcing under set -euo pipefail is a no-op that still fixes the constants; scope enum and pairings validated fail-closed; the system read distinguishes value/unset/unreadable by STATUS, so no live value can impersonate an outcome and never collapses unknown failures; the system write goes through sudo with the exact argument shape and repairs root:wheel 0644 per write, success and failure alike)\n'
