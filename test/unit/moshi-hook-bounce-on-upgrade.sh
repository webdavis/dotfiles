#!/usr/bin/env bash
# moshi-hook-bounce-on-upgrade.sh -- the daemon must be bounced when it is
# running a replaced binary, and left alone otherwise.
#
# The bounce path is the whole reason the script exists and it only fires after
# an upgrade, so it is the branch most likely to ship broken and never be
# noticed. Both directions are pinned here, plus the three refusals, against
# stubbed pgrep/lsof/stat/readlink/launchctl. No real daemon is involved and no
# launchctl call reaches the live service.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_after_46-bounce-moshi-hook-on-upgrade.sh.tmpl"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -f $TEMPLATE ]] || fail "template missing at $TEMPLATE"

sandbox="$(mktemp -d)"
[[ -n $sandbox && -d $sandbox ]] || fail 'could not create the sandbox'
trap 'rm -rf "$sandbox"' EXIT

rendered="$sandbox/bounce.sh"
# --source pins the render to THIS checkout. The script includeTemplate's the
# print library, and a bare `chezmoi execute-template` resolves includes against
# whatever source dir the ambient config names, which is another tree here and
# nothing at all in CI. The render error is reported rather than swallowed,
# because a hidden one reads as "the template is broken".
mkdir -p "$sandbox/render-home" # chezmoi's read-source-state pre hook chdirs into HOME
HOME="$sandbox/render-home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$TEMPLATE" >"$rendered" 2>"$sandbox/render.err" ||
  fail "chezmoi could not render the template:"$'\n'"$(cat "$sandbox/render.err")"

mkdir -p "$sandbox/bin"
kickstart_log="$sandbox/kickstart.log"

# run_case <running-inode> <disk-inode>: drive the script with stubs and record
# whether it kickstarted. The binary path the script probes is faked by putting
# a stub `readlink` and `stat` first on PATH.
run_case() {
  local running="$1" disk="$2"
  : >"$kickstart_log"

  cat >"$sandbox/bin/pgrep" <<'STUB'
#!/bin/bash
echo 4242
STUB
  cat >"$sandbox/bin/lsof" <<STUB
#!/bin/bash
# column 4 is the fd type, the inode is the second-to-last field
printf 'moshi-hook 4242 stephen txt REG 1,2 999 %s /some/path\n' '$running'
STUB
  cat >"$sandbox/bin/readlink" <<STUB
#!/bin/bash
printf '%s\n' '$sandbox/fake-binary'
STUB
  cat >"$sandbox/bin/stat" <<STUB
#!/bin/bash
printf '%s\n' '$disk'
STUB
  cat >"$sandbox/bin/launchctl" <<STUB
#!/bin/bash
printf '%s\n' "\$*" >>'$kickstart_log'
STUB
  chmod +x "$sandbox/bin"/*
  : >"$sandbox/fake-binary"
  chmod +x "$sandbox/fake-binary"

  # MOSHI_HOOK_BIN is a seam, and setting it is what keeps this test honest on
  # a machine where moshi-hook is NOT installed. Without it the script exits at
  # its own executable check, the bounce path is never reached, and this suite
  # passes on the developer machine while failing on a clean CI runner. That is
  # exactly what happened on the first attempt.
  #
  # MOSHI_EXTENSION_ROOT is the same kind of seam for the script's OTHER phase,
  # which rewrites the generated pi and omp extension files. Unset, it defaults
  # to the real $HOME, and running this test would edit the operator's live
  # files. It points at an empty sandbox here, so that phase finds nothing.
  PATH="$sandbox/bin:$PATH" MOSHI_HOOK_BIN="$sandbox/fake-binary" \
    MOSHI_EXTENSION_ROOT="$sandbox/home" PNS_MOSHI_GATE="$sandbox/no-such-gate.sh" \
    bash "$rendered" >"$sandbox/out" 2>&1 || true
}

kickstarted() { [[ -s $kickstart_log ]]; }

# --- 1: inodes DIFFER, so the daemon runs a replaced binary: BOUNCE ----------

run_case 111 222
kickstarted || fail '1: a replaced binary must bounce the LaunchAgent'
grep -q 'kickstart -k' "$kickstart_log" ||
  fail "1: the bounce must use kickstart -k (log: $(cat "$kickstart_log"))"
grep -q 'homebrew.mxcl.moshi-hook' "$kickstart_log" ||
  fail "1: the bounce must target the moshi-hook service (log: $(cat "$kickstart_log"))"
grep -qi 'replaced binary' "$sandbox/out" ||
  fail "1: the bounce must say why (out: $(cat "$sandbox/out"))"

# --- 2: inodes MATCH, the daemon is current: LEAVE IT ALONE ------------------

run_case 333 333
if kickstarted; then
  fail "2: a current daemon must NOT be bounced (log: $(cat "$kickstart_log"))"
fi

# --- 3: a non-numeric inode must not be read as a mismatch ------------------
# A surprising lsof or stat format would otherwise compare unequal and bounce a
# healthy daemon on every apply.

run_case 'not-an-inode' 333
if kickstarted; then
  fail '3: an unparseable running inode must not trigger a bounce'
fi
run_case 333 'not-an-inode'
if kickstarted; then
  fail '3: an unparseable disk inode must not trigger a bounce'
fi

# --- 4: an lsof that reports nothing must refuse loudly, not bounce ---------

cat >"$sandbox/bin/lsof" <<'STUB'
#!/bin/bash
exit 1
STUB
chmod +x "$sandbox/bin/lsof"
: >"$kickstart_log"
PATH="$sandbox/bin:$PATH" MOSHI_HOOK_BIN="$sandbox/fake-binary" \
  MOSHI_EXTENSION_ROOT="$sandbox/home" PNS_MOSHI_GATE="$sandbox/no-such-gate.sh" \
  bash "$rendered" >"$sandbox/out" 2>&1 || true
if kickstarted; then
  fail '4: an unreadable running image must not trigger a bounce'
fi
grep -qi 'could not read the running image' "$sandbox/out" ||
  fail "4: an unreadable running image must say so (out: $(cat "$sandbox/out"))"

# --- 5: moshi-hook not installed at all: exit quietly, bounce nothing --------
# This case is why the seam exists. It also proves the suite is not silently
# passing because the HOST happens to have moshi-hook: point the seam at a path
# that does not exist and the script must take its own not-installed exit.

run_case 111 222
: >"$kickstart_log"
PATH="$sandbox/bin:$PATH" MOSHI_HOOK_BIN="$sandbox/definitely-not-installed" \
  bash "$rendered" >"$sandbox/out" 2>&1 || true
if kickstarted; then
  fail '5: with moshi-hook absent the script must bounce nothing'
fi
[[ ! -s $sandbox/out ]] ||
  fail "5: the not-installed exit must be silent (out: $(cat "$sandbox/out"))"

# --- 6: no daemon running at all: exit 0, because a chezmoi script that exits
# non-zero ABORTS THE APPLY. pgrep exits 1 when it matches nothing and pipefail
# hands that status to the assignment, so this is one `|| true` away from every
# apply on a host with the service stopped dying at this script.

cat >"$sandbox/bin/pgrep" <<'STUB'
#!/bin/bash
exit 1
STUB
chmod +x "$sandbox/bin/pgrep"
status=0
PATH="$sandbox/bin:$PATH" MOSHI_HOOK_BIN="$sandbox/fake-binary" \
  MOSHI_EXTENSION_ROOT="$sandbox/home" PNS_MOSHI_GATE="$sandbox/no-such-gate.sh" \
  bash "$rendered" >"$sandbox/out" 2>&1 || status=$?
[[ $status -eq 0 ]] ||
  fail "6: with no daemon running the script must exit 0, got $status (out: $(cat "$sandbox/out"))"

# --- 7: a RENAMED hook subcommand must be reported ---------------------------
# The per-harness subcommands pns forwards to are undocumented, so a rename
# ships as a silent no-op: every approval would come back "path does not exist"
# instead of a decision, and nothing else in the system would notice. This is
# the detector for that, and a detector whose own pattern is wrong is worse
# than none, so the pattern is what is pinned here.

cat >"$sandbox/fake-binary" <<'STUB'
#!/bin/bash
printf 'path does not exist: %s\n' "$1" >&2
exit 1
STUB
chmod +x "$sandbox/fake-binary"
status=0
PATH="$sandbox/bin:$PATH" MOSHI_HOOK_BIN="$sandbox/fake-binary" \
  MOSHI_EXTENSION_ROOT="$sandbox/home" PNS_MOSHI_GATE="$sandbox/no-such-gate.sh" \
  bash "$rendered" >"$sandbox/out" 2>&1 || status=$?
grep -q 'claude-hook subcommand is gone' "$sandbox/out" ||
  fail "7: a renamed subcommand must be named in the warning (out: $(cat "$sandbox/out"))"
[[ $status -eq 0 ]] ||
  fail "7: a renamed subcommand must WARN, not abort the apply, got $status"

printf 'moshi-hook-bounce-on-upgrade: OK (replaced binary bounces, current daemon is left alone, unparseable and unreadable states refuse, a stopped daemon does not abort the apply, a renamed hook subcommand is reported)\n'
