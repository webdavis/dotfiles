#!/usr/bin/env bash
# homebrew-tap-trust-verified.sh: the tap-trust pass in
# run_onchange_before_10-system-packages.sh.tmpl must VERIFY that trust
# persisted, and refuse the apply when it did not.
#
# WHY THIS EXISTS. Measured on dresden 2026-08-05, during the D1 cutover. The
# pass used to be fire-and-forget, one `brew trust --tap X 2>/dev/null || true`
# per declared tap. It printed success for all sixteen taps while
# `brew trust --json v1` reported every list empty, and the apply then died two
# hundred lines later inside `brew cleanup` with "Refusing to load cask
# mediosz/tap/swipeaerospace from untrusted tap mediosz/tap". Suppressing stderr
# and forcing exit 0 is what turned a precise trust failure into a misleading
# cleanup failure, and it cost a full apply cycle to trace.
#
# WHAT THIS PINS is that the guard can FAIL. A verification that only ever
# passes is the same defect wearing a different shape, so the cases below feed
# it a store that is complete, one short, and empty, and require the last two to
# refuse and to name how many taps are missing.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -r $TEMPLATE ]] || fail "$TEMPLATE is missing"

# Render with a throwaway HOME, the same way treefmt's rendered-template
# formatter does, so the test does not depend on this machine's real state. The
# directory must EXIST: chezmoi's read-source-state pre hook chdirs into HOME
# before running, and fails the render outright when it cannot.
mkdir -p "$work/home"
rendered="$work/rendered.sh"
HOME="$work/home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$TEMPLATE" >"$rendered" 2>"$work/render.err" ||
  fail "template render failed:"$'\n'"$(cat "$work/render.err")"

# The declaration and the guard, lifted out so they can run without the rest of
# the script (which would invoke brew bundle against the live machine).
fn="$work/fn.sh"
awk '/^trusted_taps=\(/,/^\)/' "$rendered" >"$fn"
awk '/^assert_taps_trusted\(\) \{/,/^\}/' "$rendered" >>"$fn"

grep -q '^trusted_taps=(' "$fn" ||
  fail "the rendered script no longer declares a trusted_taps array; the guard cannot be verifying a declared set"
grep -q '^assert_taps_trusted() {' "$fn" ||
  fail "the rendered script no longer defines assert_taps_trusted; the trust pass is unverified again"

declared="$(sed -n '/^trusted_taps=(/,/^)/p' "$fn" | grep -c '"')"
[[ $declared -gt 0 ]] || fail "trusted_taps rendered empty, so every case below would pass vacuously"

# A stub brew whose tap-info reports Trusted only for the taps named in $1.
stub_brew() {
  cat >"$work/brew" <<STUB
#!/usr/bin/env bash
case "\$1" in
  tap-info)
    for t in $1; do
      [[ \$2 == "\$t" ]] && { printf '%s: Installed\nTrusted\n' "\$2"; exit 0; }
    done
    printf '%s: Installed\nUntrusted\n' "\$2"; exit 0 ;;
  *) exit 0 ;;
esac
STUB
  chmod +x "$work/brew"
}

# Run the guard against the stub. Prints the guard's exit status.
run_guard() {
  local rc=0
  (
    set -euo pipefail
    # shellcheck disable=SC2034  # read by the sourced guard, not by this scope
    brew_bin="$work/brew"
    # shellcheck source=/dev/null
    . "$fn"
    assert_taps_trusted
  ) >"$work/out" 2>&1 || rc=$?
  printf '%s' "$rc"
}

all_taps="$(sed -n '/^trusted_taps=(/,/^)/p' "$fn" | tr -d '"' | grep -vE '^trusted_taps=\(|^\)' | tr '\n' ' ')"

# 1. Every declared tap trusted: the guard passes and says nothing.
stub_brew "$all_taps"
[[ "$(run_guard)" == "0" ]] ||
  fail "the guard refused a fully-trusted store:"$'\n'"$(cat "$work/out")"

# 2. ONE tap short: the guard must refuse. This is the mutation that matters,
#    because it is the shape the real failure took (a store that looked fine).
one_short="${all_taps#* }"
stub_brew "$one_short"
[[ "$(run_guard)" != "0" ]] ||
  fail "the guard PASSED with a tap missing from the trust store, so it verifies nothing"
grep -q 'did not persist for 1 of' "$work/out" ||
  fail "the refusal does not report how many taps are missing:"$'\n'"$(cat "$work/out")"

# 3. Empty store: the exact condition measured on 2026-08-05.
stub_brew ""
[[ "$(run_guard)" != "0" ]] ||
  fail "the guard PASSED against a completely empty trust store, which is the live bug it exists to catch"
grep -q "did not persist for $declared of $declared" "$work/out" ||
  fail "an empty store should report every declared tap missing:"$'\n'"$(cat "$work/out")"

# 4. The guard is actually wired into the cleanup path, not just defined. The
#    original bug was trust going stale between the top of the script and the
#    cleanup, so a definition nobody calls there would not have fixed it.
# shellcheck disable=SC2016  # $brewfile is a literal in the rendered script, not an expansion here
cleanup_line="$(grep -n 'brew_bundle_cleanup_guarded "\$brewfile"' "$rendered" | head -1 | cut -d: -f1)"
[[ -n $cleanup_line ]] || fail "could not find the cleanup call in the rendered script"
bundle_line="$(grep -n 'bundle --file=' "$rendered" | head -1 | cut -d: -f1)"
[[ -n $bundle_line ]] || fail "could not find the brew bundle call in the rendered script"
# The window is deliberately AFTER the bundle. Anchoring only on "before the
# cleanup" is satisfied by the first call at the top of the script, so it passes
# whether or not the re-assert exists. Verified by mutation on 2026-08-05: with
# the pre-cleanup call deleted, the looser form still reported OK.
reassert="$(awk -v s="$bundle_line" -v e="$cleanup_line" \
  'NR>s && NR<e && $0 ~ /^[[:space:]]*assert_taps_trusted[[:space:]]*$/ {n++} END{print n+0}' "$rendered")"
[[ $reassert -ge 1 ]] ||
  fail "assert_taps_trusted is not re-called between the bundle and brew_bundle_cleanup_guarded; trust established at the top of the script is not trust the cleanup sees"

# 5. The Brewfile declares trust. This is the ROOT-CAUSE fix, found 2026-08-05
#    via the stage checkpoints: `brew bundle cleanup --force` resets Homebrew's
#    global trust store to the Brewfile's `trusted:` declarations (documented in
#    Homebrew's bundle/subcommand/cleanup.rb), so a Brewfile with none reset the
#    store to EMPTY and the trailing `brew cleanup` then refused every
#    third-party cask. Every declared trusted tap must carry `, trusted: true`
#    on its Brewfile line or the wipe returns.
brewfile_trusted="$(grep -c '^tap ".*", trusted: true$' "$rendered")"
[[ $brewfile_trusted -eq $declared ]] ||
  fail "the Brewfile declares trusted: true on $brewfile_trusted tap(s) but $declared taps are in trusted_taps; brew bundle cleanup --force resets the trust store to the Brewfile's declarations, so any gap reintroduces the wipe"

printf 'homebrew-tap-trust-verified: OK (%d taps declared; guard passes complete, refuses one-short and empty, and runs before the cleanup)\n' "$declared"
