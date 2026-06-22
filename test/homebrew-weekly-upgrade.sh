#!/usr/bin/env bash
#
# Verifies homebrew-weekly-upgrade.sh is resilient: a failing step is logged but
# does NOT abort the run, and every later step (including cleanup) still runs.
# Uses mock brew/mas (no real upgrade), so it is safe to run anywhere.
set -uo pipefail

helper="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/dot_local/bin/executable_homebrew-weekly-upgrade.sh"

if [[ ! -x $helper ]]; then
  echo "homebrew-weekly-upgrade: FAIL -- helper not found/executable: $helper" >&2
  exit 1
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# Mock brew: succeed on everything EXCEPT `upgrade`, which fails (exit 1) -- to
# prove the run continues past a failed step.
cat >"$tmp/brew" <<'MOCK'
#!/usr/bin/env bash
echo "mock brew $*"
[[ ${1:-} == upgrade ]] && exit 1
exit 0
MOCK
# Mock mas: succeed on everything.
cat >"$tmp/mas" <<'MOCK'
#!/usr/bin/env bash
echo "mock mas $*"
exit 0
MOCK
chmod +x "$tmp/brew" "$tmp/mas"

out="$(HOMEBREW_WEEKLY_BREW="$tmp/brew" HOMEBREW_WEEKLY_MAS="$tmp/mas" bash "$helper" 2>&1)"

fail=0
for marker in "== brew update ==" "== brew outdated ==" "== mas outdated ==" \
  "== brew upgrade ==" "== mas upgrade ==" "== brew cleanup ==" "=== done"; do
  if ! grep -qF "$marker" <<<"$out"; then
    echo "homebrew-weekly-upgrade: FAIL -- missing section: $marker" >&2
    fail=1
  fi
done
grep -qF "FAILED" <<<"$out" || {
  echo "homebrew-weekly-upgrade: FAIL -- failed step not reported" >&2
  fail=1
}

if [[ $fail -ne 0 ]]; then
  echo "--- helper output ---" >&2
  echo "$out" >&2
  exit 1
fi
echo "homebrew-weekly-upgrade: OK -- resilient (continued past failure; all sections + cleanup ran)"
