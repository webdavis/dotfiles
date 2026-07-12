#!/usr/bin/env bash
#
# Fix 4: the uv / npm / volta install loops in run_onchange_before_10 must be
# skipped cleanly when their tool is absent (a `command -v` guard) instead of
# aborting the whole `chezmoi apply` under set -e. A fresh machine, or an apply
# whose PATH has not yet picked up a just-installed tool, must not fail here.
#
# Integration test, two scenarios, both rendering the real chezmoiscript and
# running it with brew stubbed at the boundary:
#   absent  -- no uv/npm/volta on PATH: the script must still exit 0 (clean skip)
#   present -- stub uv/npm/volta on PATH: each loop must run (guard not over-eager)
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl"

fail() {
  printf 'homebrew-before10-ecosystem-guards: FAIL -- %s\n' "$*" >&2
  exit 1
}

command -v chezmoi >/dev/null 2>&1 || {
  printf 'SKIP: chezmoi not on PATH; cannot render before_10\n'
  exit 0
}
[[ -f $SCRIPT ]] || fail "missing template: $SCRIPT"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Render the darwin-only script (SKIP_SYSTEM_PACKAGES unset so the full body
# renders). Empty render == non-darwin host: skip.
rendered="$work/rendered.sh"
render_home="$(mktemp -d)"
HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$SCRIPT" >"$rendered" || fail "chezmoi failed to render $SCRIPT"
rm -rf "$render_home"
if [[ ! -s $rendered ]]; then
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi

# A brew stub that exits 0 for everything except `list` (so the multiplexer
# probe reports absent and the cleanup runs). $brew_prefix/bin/brew resolves
# here via HOMEBREW_PREFIX.
make_brew() {
  local dir="$1"
  mkdir -p "$dir/bin"
  cat >"$dir/bin/brew" <<'MOCK'
#!/usr/bin/env bash
[[ ${1:-} == list ]] && exit 1
exit 0
MOCK
  chmod +x "$dir/bin/brew"
}

# --- Scenario 1: tools ABSENT -> must exit 0 (clean skip) ---
absent="$work/absent"
make_brew "$absent"
rc=0
env -i HOME="$work" PATH="$absent/bin:/usr/bin:/bin" HOMEBREW_PREFIX="$absent" \
  bash "$rendered" >"$work/absent.out" 2>&1 || rc=$?
if [[ $rc -ne 0 ]]; then
  printf 'rendered output (absent):\n' >&2
  cat "$work/absent.out" >&2
  fail "script aborted (rc=$rc) with uv/npm/volta absent; loops are not guarded"
fi

# --- Scenario 2: tools PRESENT -> each loop must run (guard not over-eager) ---
present="$work/present"
make_brew "$present"
for tool in uv npm volta; do
  log="$work/$tool.invoked"
  cat >"$present/bin/$tool" <<MOCK
#!/usr/bin/env bash
printf '%s\n' "\$*" >>"$log"
exit 0
MOCK
  chmod +x "$present/bin/$tool"
done
rc=0
env -i HOME="$work" PATH="$present/bin:/usr/bin:/bin" HOMEBREW_PREFIX="$present" \
  bash "$rendered" >"$work/present.out" 2>&1 || rc=$?
if [[ $rc -ne 0 ]]; then
  printf 'rendered output (present):\n' >&2
  cat "$work/present.out" >&2
  fail "script aborted (rc=$rc) with uv/npm/volta present"
fi
for tool in uv npm volta; do
  [[ -s "$work/$tool.invoked" ]] || fail "$tool loop did not run when $tool was present (guard over-eager)"
done

printf 'homebrew-before10-ecosystem-guards: OK (absent tools skip cleanly; present tools run)\n'
