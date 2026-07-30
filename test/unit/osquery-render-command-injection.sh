#!/usr/bin/env bash
#
# render_page builds remediation next-step commands (codesign -dv <path>,
# cat <path>, sudo cat <path>, shasum -a 256 <path>) from the attacker-controlled
# finding path. A path must NEVER be interpolated into a command as raw text: a
# crafted path like /tmp/x"; touch /tmp/PROOF; # would, if the operator copies the
# suggested command, break out of the quotes and run the injected clause. Every
# rendered command operand that is a path must be a SINGLE safe shell token
# (jq @sh single-quoting, plus -- before the operand).
#
# Unit test (hostile render): for each command-building arm, render a finding whose
# ep carries a shell-injection payload, extract the emitted command, and EXECUTE it
# under stub tools (codesign/cat/shasum record their argv; touch drops a PROOF
# marker if the injection runs). Assert the injected clause never executes and the
# tool receives the whole path as one argument. bash -n alone does NOT catch this
# (the injected form is valid bash), so the proof is actual execution.
#
# This test deals in LITERAL shell-injection payloads and stub-script bodies, so
# `$(...)` / `$@` inside single quotes is deliberate (they must NOT expand here).
# shellcheck disable=SC2016
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HELPER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/render-page.sh"

fail() {
  printf 'osquery-render-command-injection: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -f $HELPER ]] || fail "missing helper: $HELPER"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin"
# Stub tools: codesign/cat/shasum record every argv line; sudo drops itself and
# execs the rest; touch drops a PROOF marker so an injected `touch` is detectable.
for t in codesign cat shasum; do
  printf '#!/usr/bin/env bash\nfor a in "$@"; do printf "%%s\\n" "$a" >>"%s/argv"; done\nexit 0\n' "$work" >"$work/bin/$t"
done
printf '#!/usr/bin/env bash\nexec "$@"\n' >"$work/bin/sudo"
printf '#!/usr/bin/env bash\n: >"%s/PROOF"\n' "$work" >"$work/bin/touch"
chmod +x "$work/bin/"*

# render_command <finding-json>: render the finding and print the emitted next-step
# command (the text inside the backticks on the Inspect/Review/Compare line).
render_command() {
  local finding="$1" pbody
  pbody="$(printf '%s\n' "$finding" | bash -c "source '$HELPER'; render_page" | jq -r '.pbody')"
  printf '%s\n' "$pbody" | grep -E '\*\*(Inspect|Review|Compare)' | sed -n 's/.*`\(.*\)`.*/\1/p' | head -1
}

# assert_safe <label> <finding> <payload-substring>: the emitted command runs the
# tool with the FULL path (payload intact) as one argument and never injects.
assert_safe() {
  local label="$1" finding="$2" needle="$3" cmd
  cmd="$(render_command "$finding")"
  [[ -n $cmd ]] || fail "$label: no next-step command was rendered"
  : >"$work/argv"
  rm -f "$work/PROOF"
  PATH="$work/bin:/usr/bin:/bin" bash -c "$cmd" >/dev/null 2>&1 || true
  [[ ! -f "$work/PROOF" ]] || fail "$label: COMMAND INJECTION - the crafted path executed 'touch'. command: $cmd"
  grep -qF -- "$needle" "$work/argv" ||
    fail "$label: the tool did not receive the whole path as one argument (argv: $(tr '\n' '|' <"$work/argv")). command: $cmd"
}

PAYLOAD='/tmp/x"; touch /tmp/PROOF; #'
SUBST_PAYLOAD='/tmp/$(touch /tmp/PROOF)'

# f <q> <ep> <extra-cols-json>: build a CRIT finding with jq (correct escaping).
f() { jq -cn --arg q "$1" --arg ep "$2" --argjson cols "$3" '{q:$q,act:"added",sev:"CRIT",cols:($cols + {}),ep:$ep}'; }

# suid_bin_unexpected -> codesign -dv <path>
assert_safe "suid (codesign) quote-injection" \
  "$(f suid_bin_unexpected "$PAYLOAD" "$(jq -cn --arg p "$PAYLOAD" '{path:$p,username:"root"}')")" \
  'touch /tmp/PROOF; #'

# suid with a command-substitution payload
assert_safe "suid (codesign) command-substitution" \
  "$(f suid_bin_unexpected "$SUBST_PAYLOAD" "$(jq -cn --arg p "$SUBST_PAYLOAD" '{path:$p,username:"root"}')")" \
  '$(touch /tmp/PROOF)'

# persistence_launchd -> cat <path>
assert_safe "persistence (cat) quote-injection" \
  "$(f persistence_launchd "$PAYLOAD" '{"label":"com.x","program":"/bin/sh"}')" \
  'touch /tmp/PROOF; #'

# file_events pipeline_integrity -> shasum -a 256 <path>
assert_safe "file_events pipeline (shasum) quote-injection" \
  "$(f file_events_recent "$PAYLOAD" '{"category":"pipeline_integrity","target_path":"/x/osquery-alerter.sh"}')" \
  'touch /tmp/PROOF; #'

# file_events non-pipeline (ssh) -> sudo cat <path>
assert_safe "file_events ssh (sudo cat) quote-injection" \
  "$(f file_events_recent "$PAYLOAD" '{"category":"ssh","target_path":"/x/authorized_keys"}')" \
  'touch /tmp/PROOF; #'

# es_launchd_writes -> codesign -dv <path>
assert_safe "es_launchd_writes (codesign) quote-injection" \
  "$(f es_launchd_writes "$PAYLOAD" '{"path":"/proc/x"}')" \
  'touch /tmp/PROOF; #'

printf 'osquery-render-command-injection: OK (codesign/cat/sudo-cat/shasum next-steps shell-escape the path; quote and command-substitution payloads never execute)\n'
