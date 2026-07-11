#!/usr/bin/env bash
# rendered-template-shellcheck-wrapper.sh drives the per-file body of the treefmt
# rendered-template lint formatter (factored into
# scripts/lib-shellcheck-rendered-template.sh) with a stubbed chezmoi and lint
# tool, proving the blank-render skip semantic:
#   (a) a template that renders whitespace-only -> skip, exit 0, shellcheck NOT run
#   (b) a template that renders a real script    -> shellcheck runs, exit follows it
#   (c) a chezmoi render failure                 -> fatal (non-zero), no skip
#   (d) a rendered script that fails shellcheck   -> fatal (non-zero)
set -euo pipefail

REPO_ROOT="$(
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." || exit 1
  pwd
)"
LIB="$REPO_ROOT/scripts/lib-shellcheck-rendered-template.sh"

[[ -f $LIB ]] || {
  printf 'FAIL: missing %s\n' "$LIB" >&2
  exit 1
}

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# ---- stub chezmoi + shellcheck on PATH ------------------------------------
STUBS="$WORK/bin"
mkdir -p "$STUBS"

# chezmoi stub: pass the template body through as the "render" (so the fixture
# file's own content is what shellcheck sees), unless STUB_CHEZMOI_FAIL is set.
cat >"$STUBS/chezmoi" <<'EOF'
#!/usr/bin/env bash
if [[ -n ${STUB_CHEZMOI_FAIL:-} ]]; then
  echo "stub chezmoi: render failure" >&2
  exit 1
fi
cat
EOF

# lint-tool stub: record that it ran, drain stdin, exit per STUB_SHELLCHECK_EXIT.
cat >"$STUBS/shellcheck" <<'EOF'
#!/usr/bin/env bash
: >"$SHELLCHECK_MARKER"
cat >/dev/null
exit "${STUB_SHELLCHECK_EXIT:-0}"
EOF

chmod +x "$STUBS/chezmoi" "$STUBS/shellcheck"
PATH="$STUBS:$PATH"
export PATH

SHELLCHECK_MARKER="$WORK/shellcheck-ran"
export SHELLCHECK_MARKER

# shellcheck source=/dev/null
. "$LIB"

fails=0
check() { # <name> <expected-rc> <expected-marker: yes|no|any>
  local name="$1" want_rc="$2" want_marker="$3" got_rc=0
  rm -f "$SHELLCHECK_MARKER"
  if render_and_shellcheck_one "$TEMPLATE"; then got_rc=0; else got_rc=$?; fi
  if [[ $want_rc == zero && $got_rc -ne 0 ]]; then
    printf 'FAIL: %s: expected exit 0, got %d\n' "$name" "$got_rc" >&2
    fails=1
  fi
  if [[ $want_rc == nonzero && $got_rc -eq 0 ]]; then
    printf 'FAIL: %s: expected non-zero exit, got 0\n' "$name" >&2
    fails=1
  fi
  if [[ $want_marker == yes && ! -e $SHELLCHECK_MARKER ]]; then
    printf 'FAIL: %s: expected shellcheck to run, it did not\n' "$name" >&2
    fails=1
  fi
  if [[ $want_marker == no && -e $SHELLCHECK_MARKER ]]; then
    printf 'FAIL: %s: expected shellcheck NOT to run, it did\n' "$name" >&2
    fails=1
  fi
}

# (a) whitespace-only render -> skip, exit 0, shellcheck not run.
TEMPLATE="$WORK/blank.tmpl"
printf '   \n\t\n' >"$TEMPLATE"
check "blank render skips" zero no

# (b) real script render -> shellcheck runs, exit 0.
TEMPLATE="$WORK/real.tmpl"
printf '#!/bin/bash\necho hi\n' >"$TEMPLATE"
check "real render runs shellcheck" zero yes

# (c) chezmoi render failure -> fatal.
TEMPLATE="$WORK/real.tmpl"
export STUB_CHEZMOI_FAIL=1
check "render failure is fatal" nonzero any
unset STUB_CHEZMOI_FAIL

# (d) rendered script fails shellcheck -> fatal.
TEMPLATE="$WORK/real.tmpl"
export STUB_SHELLCHECK_EXIT=1
check "shellcheck failure is fatal" nonzero yes
unset STUB_SHELLCHECK_EXIT

if [[ $fails -ne 0 ]]; then
  printf 'rendered-template-shellcheck-wrapper: FAILURES above\n' >&2
  exit 1
fi
printf 'rendered-template-shellcheck-wrapper: OK\n'
