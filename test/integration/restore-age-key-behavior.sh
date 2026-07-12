#!/usr/bin/env bash
# restore-age-key-behavior: drives the RENDERED age-key restore chezmoiscript through its decision paths
# with stubbed keepassxc-cli and age-keygen, in a throwaway sandbox. Verifies the F3 hardening:
#   - a symlink at the key path is rejected loudly and never written through;
#   - a WRONG existing key (recipient mismatch) warns and is NOT replaced;
#   - a CORRECT existing key is accepted silently and forced to mode 600;
#   - an ABSENT key is restored atomically from the (stubbed) KeePassXC value, then mode 600;
#   - a garbage KeePassXC value (no age marker) is refused and the key stays absent.
#
# The rendered script hardcodes the real config paths; the test rewrites only those three assignment
# lines to sandbox paths, then runs the real body. No real key, database, or secret is ever touched, and
# no plaintext is emitted. Skips cleanly when chezmoi is absent (CI without chezmoi).
set -uo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root" || exit 1
fail() {
  echo "restore-age-key-behavior: FAIL -- $1" >&2
  exit 1
}

command -v chezmoi >/dev/null 2>&1 || {
  echo "restore-age-key-behavior: skipped (no chezmoi to render the template)"
  exit 0
}

tmpl=""
for f in .chezmoiscripts/*restore-age-key*.sh.tmpl; do
  [[ -f $f ]] && tmpl="$f"
done
[[ -n $tmpl ]] || fail "no *restore-age-key*.sh.tmpl found under .chezmoiscripts/"

sandbox="$(mktemp -d)"
trap 'rm -rf "$sandbox"' EXIT

# Render the real body once; guard that it is secret-free before we ever execute it.
CI=1 chezmoi execute-template --no-tty <"$tmpl" >"$sandbox/rendered.sh" 2>/dev/null || fail "render failed"
if grep -qE 'AGE-SECRET-KEY-''(PQ-)?1[A-Z0-9]{40,}' "$sandbox/rendered.sh"; then
  fail "rendered script carries key material -- refusing to run it"
fi

expected_recipient="age1expectedrecipientvalueforthesandboxtestonly000000000000"
key_dir="$sandbox/cfg"
key_file="$key_dir/key.txt"
db_path="$sandbox/fake.kdbx" # never read; keepassxc-cli is stubbed

# Rewrite ONLY the injected config constants to sandbox values; the logic is untouched.
sed -E \
  -e "s#^key_file=.*#key_file=\"$key_file\"#" \
  -e "s#^expected_recipient=.*#expected_recipient=\"$expected_recipient\"#" \
  -e "s#^keepass_db=.*#keepass_db=\"$db_path\"#" \
  "$sandbox/rendered.sh" >"$sandbox/script.sh"
chmod +x "$sandbox/script.sh"

# Stubs. keepassxc-cli emits whatever KP_OUTPUT holds (or fails if KP_FAIL=1). age-keygen -y emits
# DERIVED (a stub public recipient); `age-keygen -o` is unused here. Both honor an absent-tool toggle.
mkdir -p "$sandbox/bin"
cat >"$sandbox/bin/keepassxc-cli" <<'STUB'
#!/usr/bin/env bash
[[ ${KP_FAIL:-0} == 1 ]] && exit 1
printf '%s\n' "${KP_OUTPUT:-}"
STUB
cat >"$sandbox/bin/age-keygen" <<'STUB'
#!/usr/bin/env bash
# Only the `-y <file>` derivation path is exercised; emit the stub recipient.
printf '%s\n' "${DERIVED:-}"
STUB
chmod +x "$sandbox/bin/keepassxc-cli" "$sandbox/bin/age-keygen"

run() { # run NAME=VALUE...  : sets stub env, prepends the stub bin to PATH, runs the script
  env PATH="$sandbox/bin:$PATH" "$@" bash "$sandbox/script.sh"
}

# GNU form first: GNU stat treats -f as filesystem-status and SUCCEEDS with
# multi-line junk under nix coreutils (CI), so a BSD-first chain never falls
# through. BSD stat rejects -c outright, so GNU-first fails cleanly into it.
perms() { stat -c '%a' "$1" 2>/dev/null || stat -f '%Lp' "$1"; }

reset_key_dir() {
  rm -rf "$key_dir"
  mkdir -p "$key_dir"
}

marker='AGE-SECRET-KEY-''1ABCDEFGHIJKLMNOPQRSTUVWXYZ234567ABCDEFGHIJKLMNOPQR' # a well-formed-looking stub

# 1. Symlink at the key path: rejected, left as a symlink, never written through.
reset_key_dir
ln -s /dev/null "$key_file"
out="$(run DERIVED="$expected_recipient" KP_OUTPUT="$marker" 2>&1)" || fail "symlink case exited nonzero"
grep -q 'symlink' <<<"$out" || fail "symlink case did not warn about a symlink"
[[ -L $key_file ]] || fail "symlink case replaced the symlink (must leave it untouched)"

# 2. Wrong existing key: derivation mismatch -> warn, do NOT replace.
reset_key_dir
printf '%s\n' "$marker" >"$key_file"
before="$(cat "$key_file")"
out="$(run DERIVED="age1wrongrecipientdoesnotmatchconfig0000000000000000000000" 2>&1)" || fail "wrong-key case exited nonzero"
grep -qi 'not overwriting' <<<"$out" || fail "wrong-key case did not warn/refuse"
[[ "$(cat "$key_file")" == "$before" ]] || fail "wrong-key case overwrote the existing key"

# 3. Correct existing key: accepted silently, forced to mode 600.
reset_key_dir
printf '%s\n' "$marker" >"$key_file"
chmod 644 "$key_file"
out="$(run DERIVED="$expected_recipient" 2>&1)" || fail "correct-key case exited nonzero"
[[ "$(perms "$key_file")" == 600 ]] || fail "correct-key case did not enforce mode 600 (got $(perms "$key_file"))"

# 4. Absent key: restored atomically from the stubbed value, then mode 600.
reset_key_dir
out="$(run DERIVED="$expected_recipient" KP_OUTPUT="$marker" 2>&1)" || fail "absent-key case exited nonzero"
[[ -f $key_file ]] || fail "absent-key case did not create the key file"
[[ "$(perms "$key_file")" == 600 ]] || fail "restored key is not mode 600 (got $(perms "$key_file"))"
grep -qF "$marker" "$key_file" || fail "restored key content does not match the KeePassXC value"

# 5. Absent key, garbage value (no age marker): refused, key stays absent.
reset_key_dir
out="$(run DERIVED="$expected_recipient" KP_OUTPUT="not-a-key-just-noise" 2>&1)" || fail "garbage case exited nonzero"
grep -qi 'not an age identity' <<<"$out" || fail "garbage case did not refuse the non-key value"
[[ ! -e $key_file ]] || fail "garbage case installed a non-key value"

echo "restore-age-key-behavior: OK (symlink/wrong/correct/absent/garbage paths all hold)"
