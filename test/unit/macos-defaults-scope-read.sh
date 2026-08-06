#!/usr/bin/env bash
# macos-defaults-scope-read.sh -- unit coverage for the scope-aware helpers in
# macos-defaults-lib.sh: resolve_system_plist_path, validate_record_scope,
# system_defaults_read_actual, and system_defaults_write.
#
# The properties pinned here, each a fail-closed rule a consumer depends on:
#
#   - resolve_system_plist_path defaults an empty declared path to
#     /Library/Preferences/<domain>, passes an absolute path through verbatim,
#     and REJECTS a relative path rather than resolving it against whatever
#     directory the caller happens to be standing in.
#   - resolve_system_plist_path enforces the domain rule for EVERY system-scope
#     record, not only for the ones that omit plist_path. The template rejects a
#     traversal domain unconditionally, so a library that only checked it on the
#     default-path branch was the MORE PERMISSIVE of the two consumers and the
#     same YAML rendered one way and applied another.
#   - Degenerate targets are rejected rather than resolved: a domain that is
#     empty or nothing but dots, and a plist_path of exactly "/". None of them
#     escape a directory, but none of them answer "does this resolve where I
#     intend to write" either.
#   - validate_record_scope accepts only user/system, rejects the set-but-empty
#     scope "" (yq already defaulted an ABSENT field to "user", so "" can only
#     mean an explicitly empty field), and rejects the meaningless pairs
#     scope=system+host and scope=user+plist_path.
#   - system_defaults_read_actual distinguishes THREE outcomes: the value,
#     "<unset>" (only when defaults itself says the pair does not exist), and
#     "<unreadable>" for every other failure AND for a plist file that exists
#     but cannot be read. An unknown failure must never collapse into
#     "<unset>": that would report drift on a value nobody actually read.
#   - The library survives being sourced twice under `set -euo pipefail`. Its
#     read-outcome constants were `readonly`, so a second `source` failed the
#     assignment, and under `set -e` that failure killed the CALLER outright.
#
# Pure stubs: no real chezmoi, git, or yq. The only binary the functions under
# test invoke is `defaults` (and `sudo`), both stubbed per case.
set -euo pipefail

# Scrubbed at SCRIPT scope. Git exports GIT_DIR to every hook it runs and this
# suite runs from the pre-commit hook; the library's own override may be exported
# on a developer machine.
unset MACOS_DEFAULTS_SOURCE_DIR GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIB="$REPO_ROOT/dot_local/libexec/macos-defaults/helpers/defaults-records.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -f $LIB ]] || fail "missing lib: $LIB"

work="$(cd "$(mktemp -d)" && pwd -P)"
trap 'chmod -R u+rwX "$work" 2>/dev/null; rm -rf "$work"' EXIT

stub_bin="$work/bin"
mkdir -p "$stub_bin"

# call_function <function> [args...] -- run one library function with the stubs
# on PATH. Prints its stdout, writes its stderr to "$work/err", returns its
# status.
call_function() { # <function> [args...]
  PATH="$stub_bin:$PATH" LIB="$LIB" bash -c 'source "$LIB"; "$@"' _ "$@" 2>"$work/err"
}

# ---- resolve_system_plist_path ----------------------------------------------

# case 1 (control): an empty declared path defaults to /Library/Preferences.
status=0
output="$(call_function resolve_system_plist_path com.example.sys '')" || status=$?
[[ $status -eq 0 ]] ||
  fail "default path: resolve_system_plist_path must succeed on an empty declared path (got $status, stderr: $(cat "$work/err"))"
[[ $output == '/Library/Preferences/com.example.sys' ]] ||
  fail "default path: an empty declared path must resolve to /Library/Preferences/<domain> (got '$output')"

# case 2: an explicit absolute path passes through verbatim.
status=0
output="$(call_function resolve_system_plist_path com.example.lulu /Library/Objective-See/LuLu/preferences.plist)" || status=$?
[[ $status -eq 0 ]] ||
  fail "absolute path: resolve_system_plist_path must accept an absolute path (got $status, stderr: $(cat "$work/err"))"
[[ $output == '/Library/Objective-See/LuLu/preferences.plist' ]] ||
  fail "absolute path: an absolute declared path must pass through verbatim (got '$output')"

# case 3: a relative path is rejected, never resolved against the working dir.
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

# case 4: a dot-relative path is a relative path too.
status=0
output="$(call_function resolve_system_plist_path com.example.rel './prefs.plist')" || status=$?
[[ $status -ne 0 ]] ||
  fail "dot-relative path: resolve_system_plist_path must reject ./prefs.plist (got 0, stdout: '$output')"

# reject_resolved_path <label> <domain> <plist_path> -- assert the pair is
# refused with a nonzero status AND an empty stdout. Empty stdout matters as much
# as the status: every caller reads the resolved path from stdout, so a refusal
# that still printed something would hand a caller the very target it rejected.
reject_resolved_path() { # <label> <domain> <plist_path>
  local label="$1" domain="$2" declared_path="$3"
  local reject_status=0 reject_output
  reject_output="$(call_function resolve_system_plist_path "$domain" "$declared_path")" || reject_status=$?
  [[ $reject_status -ne 0 ]] ||
    fail "$label: resolve_system_plist_path must reject domain '$domain' plist_path '$declared_path' (got 0, stdout: '$reject_output')"
  [[ -z $reject_output ]] ||
    fail "$label: a rejected pair must print nothing on stdout (got '$reject_output')"
}

# case 4a: a traversal domain is rejected on the DEFAULT-path branch. Without the
# slash rule, /Library/Preferences/../../tmp/owned is /tmp/owned, written as root.
reject_resolved_path 'traversal domain, default path' '../../tmp/owned' ''

# case 4b: the same traversal domain is rejected when the record ALSO declares a
# plist_path. The template rejects this record outright, so a library that
# accepted it made the two consumers disagree about the same YAML.
reject_resolved_path 'traversal domain, declared path' '../../tmp/owned' \
  '/Library/Preferences/com.example.sys.plist'

# case 4c: a declared plist_path that walks back out is rejected in every
# spelling, including the ones a bare "starts with /" check waves through.
reject_resolved_path 'traversal path, interior' com.example.sys '/Library/Preferences/../../etc/x'
reject_resolved_path 'traversal path, bare root parent' com.example.sys '/..'
reject_resolved_path 'traversal path, trailing parent' com.example.sys '/Library/Preferences/..'
reject_resolved_path 'traversal path, doubled slashes' com.example.sys '//Library/..//etc/x'

# case 4d: degenerate targets. None of these escape a directory (`defaults`
# appends .plist), so they are hygiene rather than traversal, but none of them
# name a file anybody meant to write either.
reject_resolved_path 'empty domain' '' ''
reject_resolved_path 'dot domain' '.' ''
reject_resolved_path 'dot-dot domain' '..' ''
reject_resolved_path 'root plist_path' com.example.sys '/'

# case 4e (control for 4a-4d): the real path a later slice depends on stays
# ACCEPTED. Without this row every rejection above is satisfied by a function
# that refuses everything.
status=0
output="$(call_function resolve_system_plist_path com.example.lulu '/Library/Objective-See/LuLu/preferences.plist')" || status=$?
[[ $status -eq 0 && $output == '/Library/Objective-See/LuLu/preferences.plist' ]] ||
  fail "LuLu path: the tracked Objective-See path must stay accepted verbatim (got status $status, output '$output')"

# ---- validate_record_scope ---------------------------------------------------

# case 5 (control): user with no host and no path is valid.
status=0
output="$(call_function validate_record_scope user '' '')" || status=$?
[[ $status -eq 0 && $output == user ]] ||
  fail "scope user: must validate and print 'user' (got status $status, output '$output')"

# case 6: system with no host and no path is valid.
status=0
output="$(call_function validate_record_scope system '' '')" || status=$?
[[ $status -eq 0 && $output == system ]] ||
  fail "scope system: must validate and print 'system' (got status $status, output '$output')"

# case 7: user with a host stays valid (the pre-slice pairing, unchanged).
status=0
output="$(call_function validate_record_scope user current '')" || status=$?
[[ $status -eq 0 && $output == user ]] ||
  fail "scope user + host: the existing pairing must stay valid (got status $status, output '$output')"

# case 8: an unknown scope is rejected.
status=0
output="$(call_function validate_record_scope bogus '' '')" || status=$?
[[ $status -ne 0 ]] ||
  fail "unknown scope: 'bogus' must be rejected (got 0, output '$output')"
grep -qF 'unknown scope' "$work/err" ||
  fail "unknown scope: the rejection must say the scope is unknown (stderr: $(cat "$work/err"))"
[[ -z $output ]] ||
  fail "unknown scope: a rejected scope must print nothing on stdout (got '$output')"

# case 9: a set-but-empty scope is rejected, not treated as absent. yq defaults
# an ABSENT scope to "user" before this function ever sees it, so "" here can
# only mean an explicitly empty field in the record.
status=0
output="$(call_function validate_record_scope '' '' '')" || status=$?
[[ $status -ne 0 ]] ||
  fail "empty scope: a set-but-empty scope must be rejected (got 0, output '$output')"

# case 10: system + host is a meaningless pair (ByHost storage is per-user).
status=0
output="$(call_function validate_record_scope system current '')" || status=$?
[[ $status -ne 0 ]] ||
  fail "system + host: the pair must be rejected (got 0, output '$output')"
grep -qF 'per-user' "$work/err" ||
  fail "system + host: the rejection must explain ByHost storage is per-user (stderr: $(cat "$work/err"))"

# case 11: user + plist_path is rejected; the path is only honored on system
# records, and silently ignoring it would write the user domain instead.
status=0
output="$(call_function validate_record_scope user '' /Library/Preferences/x.plist)" || status=$?
[[ $status -ne 0 ]] ||
  fail "user + plist_path: the pair must be rejected (got 0, output '$output')"
grep -qF 'plist_path' "$work/err" ||
  fail "user + plist_path: the rejection must name plist_path (stderr: $(cat "$work/err"))"

# ---- system_defaults_read_actual ----------------------------------------------

# write_defaults_read_stub <ok|unset|denied> -- how `defaults read` behaves.
write_defaults_read_stub() { # <ok|unset|denied>
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

# case 12 (control): a successful read prints the value.
write_defaults_read_stub ok
status=0
output="$(call_function system_defaults_read_actual /Library/Preferences/com.example.sys SysKey)" || status=$?
[[ $status -eq 0 && $output == 1 ]] ||
  fail "read ok: a successful read must print the value (got status $status, output '$output')"

# case 13: a does-not-exist failure is the ONE failure that means unset.
write_defaults_read_stub unset
status=0
output="$(call_function system_defaults_read_actual /Library/Preferences/com.example.sys SysKey)" || status=$?
[[ $status -eq 1 ]] ||
  fail "read unset: a does-not-exist failure must report the unset STATUS 1 (got $status, stderr: $(cat "$work/err"))"
[[ -z $output ]] ||
  fail "read unset: the unset outcome must print no value, so no value can impersonate it (got '$output')"

# case 14: any OTHER failure is indeterminate, with a marker DISTINCT from
# <unset>. Collapsing it into <unset> would report drift on an unread value.
write_defaults_read_stub denied
status=0
output="$(call_function system_defaults_read_actual /Library/Preferences/com.example.sys SysKey)" || status=$?
[[ $status -eq 2 ]] ||
  fail "read denied: an unknown failure must report the unreadable STATUS 2, never unset (got $status, stderr: $(cat "$work/err"))"

# case 15: a plist file that exists but cannot be read is indeterminate BEFORE
# defaults is consulted. The stub would report a matching value, so a read that
# skipped the file check would report all-clear here, which is exactly the
# fail-open this case makes distinguishable.
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

# case 15a: a failed mktemp refuses toward indeterminate AND says why. Status
# alone cannot pin this: with the guard deleted the empty filename becomes an
# ambiguous redirect, which ALSO ends at status 2, so the deliberate refusal and
# the accident are indistinguishable by status. The explicit message is what
# separates them.
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

# ---- re-sourcing the library --------------------------------------------------

# case 16: sourcing the library twice under `set -euo pipefail` must survive.
# `readonly` constants make the second source's assignment fail, and under
# `set -e` that failure kills the CALLER, not just the source. Latent while no
# caller sources twice, and a trap for the first one that does.
status=0
double_source_output="$(bash -c 'set -euo pipefail; source "$1"; source "$1"; printf "SURVIVED\n"' _ "$LIB" 2>"$work/err")" || status=$?
[[ $status -eq 0 ]] ||
  fail "double source: sourcing the library twice under set -euo pipefail must succeed (got $status, stderr: $(cat "$work/err"))"
[[ $double_source_output == SURVIVED ]] ||
  fail "double source: the caller must run past the second source (got '$double_source_output')"

# case 17: the constants must still carry their documented values after a
# re-source. A guard that skipped the assignment on an already-set value would
# satisfy case 16 while leaving the caller reading whatever status codes the
# environment handed it.
status=0
constants_output="$(SYSTEM_READ_OK=9 SYSTEM_READ_UNSET=9 SYSTEM_READ_UNREADABLE=9 \
  bash -c 'set -euo pipefail; source "$1"; source "$1"; printf "%s %s %s\n" "$SYSTEM_READ_OK" "$SYSTEM_READ_UNSET" "$SYSTEM_READ_UNREADABLE"' \
  _ "$LIB" 2>"$work/err")" || status=$?
[[ $status -eq 0 && $constants_output == '0 1 2' ]] ||
  fail "double source: the read-outcome constants must be 0 1 2 even when the environment presets them (got status $status, output '$constants_output')"

# ---- require_system_plist_path_permitted ------------------------------------

# case 18: the WRITE-path allowlist. Rendering already refuses an
# out-of-allowlist plist_path, but `just defaults-apply` reads the YAML
# directly, so without this gate a record the render would refuse was still
# handed to `sudo defaults write` (verified against /etc/example.evil.plist
# before the fix). Absolute and parent-free is not enough: membership in the
# deliberately granted directories is its own rule.
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

# Positive controls: the default construction and the tracked Objective-See
# path both pass, so the refusals above were not bought with a blanket deny.
for permitted_path in /Library/Preferences/com.example.sys /Library/Objective-See/LuLu/preferences.plist; do
  status=0
  call_function require_system_plist_path_permitted "$permitted_path" >/dev/null || status=$?
  [[ $status -eq 0 ]] ||
    fail "allowlist: $permitted_path must be permitted (got $status, stderr: $(cat "$work/err"))"
done

# ---- system_defaults_write -----------------------------------------------------

# case 16: the write goes through sudo with the exact argument shape
# `defaults write <plist-path> <key> -<type> <value>`, then repairs the
# written file to root:wheel 0644 IN THE SAME CALL. `defaults write` recreates
# the target as a root-owned 0600 binary plist (verified on a copy,
# 2026-07-27), and a 0600 plist reads back SYSTEM_READ_UNREADABLE for the
# unprivileged drift checker forever after, so an unrepaired write defeats the
# very drift gate that verifies it. The sudo stub records one line per CALL
# and does not execute anything.
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

# case 16b: a declared path already ending in .plist is repaired AS ITSELF,
# never with a second .plist appended.
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

# case 16c: a FAILED write that left no file behind re-raises the write's
# status and skips the repair rather than failing on the missing path.
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

# case 16d: a FAILED write whose target file EXISTS still repairs it before
# re-raising the failure: a write can die after replacing the file, and under
# set -e the caller ends at this record, so a trailing cleanup would never
# run. Per-write repair is the property that survives that.
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
