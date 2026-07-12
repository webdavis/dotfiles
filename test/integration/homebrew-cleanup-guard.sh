#!/usr/bin/env bash
#
# Fix 3 (forced cleanup must not trust an empty/partial Brewfile): before_10 runs
# `brew bundle cleanup --force` against the rendered Brewfile. Under the two-world
# model main's manifest is legitimately partial until later slices land, so an
# unguarded forced cleanup would mass-uninstall at D1. The guard runs the
# non-forced `brew bundle cleanup` preview FIRST, counts would-be removals, and
# REFUSES the forced cleanup (loud warning naming the packages; install results
# stand) when the count exceeds a threshold (default 5), unless
# HOMEBREW_BUNDLE_ALLOW_BULK_CLEANUP=1 authorizes it.
#
# Integration test: render the real before_10 and run it with brew stubbed at the
# boundary; the stub's non-forced `bundle cleanup` prints a controllable
# would-remove block, and forced `bundle cleanup --force` is recorded so a test
# can prove whether it ran.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl"

fail() {
  printf 'homebrew-cleanup-guard: FAIL -- %s\n' "$*" >&2
  exit 1
}

command -v chezmoi >/dev/null 2>&1 || {
  printf 'SKIP: chezmoi not on PATH; cannot render before_10\n'
  exit 0
}
[[ -f $SCRIPT ]] || fail "missing template: $SCRIPT"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

rendered="$work/rendered.sh"
render_home="$(mktemp -d)"
HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$SCRIPT" >"$rendered" || fail "chezmoi failed to render $SCRIPT"
rm -rf "$render_home"
if [[ ! -s $rendered ]]; then
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi

# Preview blocks mimic Homebrew 6.x's non-forced `brew bundle cleanup` dry-run
# output ("Would uninstall formulae:" / "Would untap:" + columnar names +
# footer). BULK has 8 items (> threshold 5); SMALL has 3 (<= 5).
bulk_preview="$work/bulk-preview.txt"
cat >"$bulk_preview" <<'EOF'
Would uninstall formulae:
codex codex-app lulu oversight paseo foo bar
Would untap:
rjyo/moshi
Run `brew bundle cleanup --force` to make these changes.
EOF
small_preview="$work/small-preview.txt"
cat >"$small_preview" <<'EOF'
Would uninstall formulae:
codex paseo lulu
Run `brew bundle cleanup --force` to make these changes.
EOF

# Build a stub prefix. The brew stub: `tap` (no arg) lists nothing (teardown
# skips); `list` exits 1 (multiplexer absent -> cleanup path runs); `bundle
# cleanup --force` is RECORDED in $FORCE_LOG; the non-forced `bundle cleanup`
# preview prints $PREVIEW_FILE and exits 1 (removals present); plain `bundle`
# install exits 0. uv/npm/volta/mas stubs exit 0.
make_prefix() {
  local prefix="$1"
  mkdir -p "$prefix/bin"
  cat >"$prefix/bin/brew" <<'MOCK'
#!/usr/bin/env bash
case "${1:-}" in
  tap) exit 0 ;;
  list) exit 1 ;;
  autoupdate) exit 0 ;;
  bundle)
    if [[ ${2:-} == cleanup ]]; then
      for a in "$@"; do
        if [[ $a == --force ]]; then
          printf '%s\n' "$*" >>"$FORCE_LOG"
          exit 0
        fi
      done
      [[ -n ${PREVIEW_FILE:-} && -f ${PREVIEW_FILE:-} ]] && cat "$PREVIEW_FILE"
      exit 1
    fi
    exit 0 ;;
esac
exit 0
MOCK
  for tool in uv npm volta mas; do
    printf '#!/usr/bin/env bash\nexit 0\n' >"$prefix/bin/$tool"
  done
  chmod +x "$prefix/bin/"*
}

# run_case <name> <preview-file> <allow-bulk 0|1> -> sets FORCE_LOG, WARN
run_case() {
  local name="$1" preview="$2" allow="$3"
  local prefix="$work/$name/prefix"
  FORCE_LOG="$work/$name/force.log"
  WARN="$work/$name/stderr"
  local case_home="$work/$name/home"
  mkdir -p "$prefix" "$case_home"
  make_prefix "$prefix"
  : >"$FORCE_LOG"
  local -a env_extra=()
  [[ $allow == 1 ]] && env_extra+=("HOMEBREW_BUNDLE_ALLOW_BULK_CLEANUP=1")
  env -i HOME="$case_home" PATH="$prefix/bin:/usr/bin:/bin" HOMEBREW_PREFIX="$prefix" \
    PREVIEW_FILE="$preview" FORCE_LOG="$FORCE_LOG" "${env_extra[@]}" \
    bash "$rendered" >"$work/$name/stdout" 2>"$WARN" ||
    fail "$name: rendered before_10 exited non-zero (stderr: $(cat "$WARN"))"
}

forced_ran() { [[ -s $FORCE_LOG ]]; }

# --- partial manifest (8 > 5): forced cleanup REFUSED, warning names packages --
run_case partial "$bulk_preview" 0
forced_ran && {
  printf 'stderr:\n' >&2
  cat "$WARN" >&2
  fail "partial: forced cleanup ran though 8 removals exceed the threshold"
}
grep -qiE 'refus' "$WARN" || fail "partial: no refusal warning printed (stderr: $(cat "$WARN"))"
grep -qF 'codex' "$WARN" || fail "partial: the refusal warning does not name the would-be removals (stderr: $(cat "$WARN"))"

# --- small delta (3 <= 5): forced cleanup PROCEEDS ----------------------------
run_case small "$small_preview" 0
forced_ran || fail "small: forced cleanup was withheld though only 3 removals (<= threshold)"

# --- override: 8 removals but authorized -> forced cleanup PROCEEDS ------------
run_case override "$bulk_preview" 1
forced_ran || fail "override: forced cleanup was withheld though HOMEBREW_BUNDLE_ALLOW_BULK_CLEANUP=1"

printf 'homebrew-cleanup-guard: OK (bulk refused + named; small proceeds; override proceeds)\n'
