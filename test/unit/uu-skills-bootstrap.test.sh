#!/usr/bin/env bash
# The bootstrap wrapper runs only owned stand-ins under each case's private HOME.

# shellcheck disable=SC2016  # Several redirects below rewrite a rendered script
# so it expands $HOME at RUN time; the single quotes keeping $HOME literal are
# the point, not an oversight.
function set_up() {
  SKILLS_FIXTURE="$(mktemp -d)"
  SKILLS_REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  mkdir -p "$SKILLS_FIXTURE/.cargo/bin" \
    "$SKILLS_FIXTURE/.local/state/skills" "$SKILLS_FIXTURE/c" "$SKILLS_FIXTURE/d" \
    "$SKILLS_FIXTURE/s" "$SKILLS_FIXTURE/k" "$SKILLS_FIXTURE/r" "$SKILLS_FIXTURE/l" "$SKILLS_FIXTURE/t"
  # RENDERED, not comment-stripped. Until the install location moved, the only
  # Go actions in this script sat in comment lines, so deleting those lines left
  # runnable bash. The two binary paths now come from .chezmoidata/rust_tools.yaml,
  # and a stripped copy carries a bare `{{ ... }}` into executable position, which
  # exits 127 in every case. Render properly, then point both paths back at this
  # case's own HOME, because the render bakes an absolute path off the real one.
  CI=1 HOME="$SKILLS_FIXTURE" chezmoi --source "$SKILLS_REPO" execute-template --no-tty \
    <"$SKILLS_REPO/.chezmoiscripts/run_onchange_after_64-update-skills-first-install.sh.tmpl" \
    >"$SKILLS_FIXTURE/subject.sh" 2>/dev/null
  sed -i '' -e 's|^UPDATER=.*|UPDATER="$HOME/.cargo/bin/uu"|' \
    -e 's|^ENGINE=.*|ENGINE="$HOME/.cargo/bin/pns"|' "$SKILLS_FIXTURE/subject.sh"
  grep -q '^UPDATER="\$HOME/.cargo/bin/uu"$' "$SKILLS_FIXTURE/subject.sh"
  grep -q '^ENGINE="\$HOME/.cargo/bin/pns"$' "$SKILLS_FIXTURE/subject.sh"
  cat >"$SKILLS_FIXTURE/.cargo/bin/uu" <<'STUB'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$@" >"$HOME/argv"
cat "$HOME/message" >&2
exit "$(cat "$HOME/exit")"
STUB
  cat >"$SKILLS_FIXTURE/.cargo/bin/pns" <<'STUB'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$@" >"$HOME/alarm"
STUB
  chmod 755 "$SKILLS_FIXTURE/.cargo/bin/uu" "$SKILLS_FIXTURE/.cargo/bin/pns"
  : >"$SKILLS_FIXTURE/message"
}

skills_invoke() {
  env -i HOME="$SKILLS_FIXTURE" PATH=/usr/bin:/bin \
    XDG_CONFIG_HOME="$SKILLS_FIXTURE/c" XDG_DATA_HOME="$SKILLS_FIXTURE/d" \
    XDG_STATE_HOME="$SKILLS_FIXTURE/s" XDG_CACHE_HOME="$SKILLS_FIXTURE/k" \
    XDG_RUNTIME_DIR="$SKILLS_FIXTURE/r" XDG_CONFIG_DIRS="$SKILLS_FIXTURE/c" \
    XDG_DATA_DIRS="$SKILLS_FIXTURE/d" CLAUDE_CONFIG_DIR="$SKILLS_FIXTURE/l" \
    TMPDIR="$SKILLS_FIXTURE/t" TMP="$SKILLS_FIXTURE/t" TEMP="$SKILLS_FIXTURE/t" \
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null \
    /bin/bash "$SKILLS_FIXTURE/subject.sh" >"$SKILLS_FIXTURE/stdout" 2>"$SKILLS_FIXTURE/stderr"
}

function test_a_successful_skills_bootstrap_clears_the_pending_marker() {
  printf '0\n' >"$SKILLS_FIXTURE/exit"
  printf '2\n' >"$SKILLS_FIXTURE/.local/state/skills/first-install-pending"
  skills_invoke
  assert_same 0 "$?"
  assert_same $'bootstrap\nskills' "$(cat "$SKILLS_FIXTURE/argv" 2>/dev/null)"
  assert_file_not_exists "$SKILLS_FIXTURE/.local/state/skills/first-install-pending"
  assert_file_not_exists "$SKILLS_FIXTURE/alarm"
}

function test_a_failed_skills_bootstrap_advances_the_retry_marker_without_aborting_apply() {
  printf '1\n' >"$SKILLS_FIXTURE/exit"
  printf '08\n' >"$SKILLS_FIXTURE/.local/state/skills/first-install-pending"
  printf 'owned bootstrap failure\n' >"$SKILLS_FIXTURE/message"
  skills_invoke
  assert_same 0 "$?"
  assert_same '9' "$(cat "$SKILLS_FIXTURE/.local/state/skills/first-install-pending")"
  assert_same $'bootstrap\nskills' "$(cat "$SKILLS_FIXTURE/argv" 2>/dev/null)"
  assert_contains 'owned bootstrap failure' "$(cat "$SKILLS_FIXTURE/stderr")"
  assert_contains 'bootstrap skills' "$(cat "$SKILLS_FIXTURE/alarm" 2>/dev/null)"
}

function test_a_contended_skills_bootstrap_retains_work_for_the_next_apply() {
  printf '1\n' >"$SKILLS_FIXTURE/exit"
  printf 'uu: another run holds the lock\n' >"$SKILLS_FIXTURE/message"
  skills_invoke
  assert_same 0 "$?"
  assert_same '1' "$(cat "$SKILLS_FIXTURE/.local/state/skills/first-install-pending" 2>/dev/null)"
  assert_same $'bootstrap\nskills' "$(cat "$SKILLS_FIXTURE/argv" 2>/dev/null)"
  assert_contains 'another run holds the lock' "$(cat "$SKILLS_FIXTURE/stderr")"
  assert_contains 'retry on the next apply' "$(cat "$SKILLS_FIXTURE/stderr")"
}
