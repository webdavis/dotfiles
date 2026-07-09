#!/usr/bin/env bash
# post-commit-graphify-dispatcher.sh — decision logic of the user-wide post-commit
# dispatcher (dot_config/git/hooks/executable_post-commit).
#
# The dispatcher runs graphify's knowledge-graph rebuild after every commit by
# default, skips it in repos carrying a `.githooks/no-graphify` marker, and then
# chains a repo's own `.githooks/post-commit`. This test drives real `git commit`s
# in sandboxed fixture repos with a stub interpreter standing in for graphify's
# pinned uv-tool python (HOME is redirected, so the dispatcher's first interpreter
# probe hits the stub and never the real graphifyy install):
#   1. repo WITHOUT the marker      -> the rebuild launch is recorded (exactly once;
#      a graphify-out/-only commit records nothing — preserved guard)
#   2. repo WITH the marker         -> no launch recorded at all
#   3. either repo                  -> its own .githooks/post-commit is chained
#   4. rebuild launch fails         -> the hook still exits 0 (a post-commit hook
#      must never surface as a commit failure)
#   5. outside any repo             -> exits 0
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DISPATCHER="$REPO_ROOT/dot_config/git/hooks/executable_post-commit"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -f $DISPATCHER ]] || fail "dispatcher not found: $DISPATCHER"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# --- Sandbox -----------------------------------------------------------------
# Fake HOME: the dispatcher's pinned-interpreter probe resolves under $HOME, so
# it finds the stub below instead of the real graphifyy tool env; git reads no
# user/system config. Unset the repo-context env git exports to hooks so the
# fixtures don't inherit this repo's.
export HOME="$tmp/home"
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null
export GIT_AUTHOR_NAME=test GIT_AUTHOR_EMAIL=test@invalid
export GIT_COMMITTER_NAME=test GIT_COMMITTER_EMAIL=test@invalid
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GRAPHIFY_SKIP_HOOK STUB_LAUNCH_EXIT || true

# Stub python at graphify's pinned uv-tool path: answers the availability probe
# (find_spec) with success, records any rebuild launch, exits STUB_LAUNCH_EXIT.
stub_dir="$HOME/.local/share/uv/tools/graphifyy/bin"
mkdir -p "$stub_dir"
cat >"$stub_dir/python" <<'EOF'
#!/usr/bin/env bash
set -u
if [[ "${2:-}" == *find_spec* ]]; then
  exit 0
fi
printf 'launch cwd=%s\n' "$PWD" >>"$STUB_LOG"
exit "${STUB_LAUNCH_EXIT:-0}"
EOF
chmod +x "$stub_dir/python"

# Fixture hooks dir: the dispatcher deployed as `post-commit`, wired per repo
# via core.hooksPath (standing in for the user-wide hooks dir).
hooks_dir="$tmp/hooks"
mkdir -p "$hooks_dir"
cp "$DISPATCHER" "$hooks_dir/post-commit"
chmod +x "$hooks_dir/post-commit"

# make_repo <dir>: git repo wired to the dispatcher, with a chain hook that
# appends to $CHAIN_LOG — a path OUTSIDE the repo, so the commit_change helper's
# `git add -A` never sweeps hook output into the fixture's history.
make_repo() {
  mkdir -p "$1/.githooks"
  git -C "$1" init -q -b main
  git -C "$1" config core.hooksPath "$hooks_dir"
  cat >"$1/.githooks/post-commit" <<'EOF'
#!/usr/bin/env bash
printf 'chained\n' >>"$CHAIN_LOG"
EOF
  chmod +x "$1/.githooks/post-commit"
}

# commit_change <dir> <path> <msg>: stage one file and commit.
commit_change() {
  mkdir -p "$1/$(dirname "$2")"
  printf '%s\n' "$3" >"$1/$2"
  git -C "$1" add -A
  git -C "$1" commit -q -m "$3"
}

# --- 1 + 3: no marker -> graphify launched once; chain hook always runs -------
repo_a="$tmp/repo-default"
make_repo "$repo_a"
export STUB_LOG="$tmp/a.log" CHAIN_LOG="$tmp/a-chain.log"
commit_change "$repo_a" file.txt first
commit_change "$repo_a" file.txt second
[[ -f $STUB_LOG ]] || fail "no-marker repo: graphify launch was never recorded"
launches="$(wc -l <"$STUB_LOG" | tr -d ' ')"
# The first commit has no HEAD~1 and a clean tree -> the changed-files guard
# skips it; only the second commit launches.
[[ $launches == 1 ]] || fail "no-marker repo: expected 1 launch, got $launches"
# pwd -P on both sides: on macOS mktemp hands out /var/... while a fresh
# process sees the physical /private/var/... path.
repo_a_phys="$(cd "$repo_a" && pwd -P)"
grep -q "cwd=$repo_a_phys" "$STUB_LOG" || fail "launch did not run at the repo toplevel"

# Preserved guard: a commit touching only graphify-out/ must not rebuild.
commit_change "$repo_a" graphify-out/graph.json artifacts-only
launches="$(wc -l <"$STUB_LOG" | tr -d ' ')"
[[ $launches == 1 ]] || fail "graphify-out/-only commit triggered a rebuild"

chain_lines="$(wc -l <"$tmp/a-chain.log" | tr -d ' ')"
[[ $chain_lines == 3 ]] || fail "no-marker repo: chain hook ran $chain_lines/3 times"

# --- 2 + 3: marker -> graphify skipped; chain hook still runs -----------------
repo_b="$tmp/repo-optout"
make_repo "$repo_b"
: >"$repo_b/.githooks/no-graphify"
export STUB_LOG="$tmp/b.log" CHAIN_LOG="$tmp/b-chain.log"
commit_change "$repo_b" file.txt first
commit_change "$repo_b" file.txt second
[[ ! -e $STUB_LOG ]] || fail "opt-out repo: graphify was launched despite the marker"
chain_lines="$(wc -l <"$tmp/b-chain.log" | tr -d ' ')"
[[ $chain_lines == 2 ]] || fail "opt-out repo: chain hook ran $chain_lines/2 times"

# --- 4: failing rebuild launch must not fail the hook -------------------------
# Fresh non-artifact commit so HEAD~1..HEAD is a launch-worthy change (the last
# repo_a commit above was graphify-out/-only), then rerun the hook directly with
# a failing stub.
export STUB_LOG="$tmp/fail.log"
commit_change "$repo_a" file.txt third
rc=0
(cd "$repo_a" && STUB_LAUNCH_EXIT=1 "$hooks_dir/post-commit") >/dev/null || rc=$?
[[ $rc -eq 0 ]] || fail "hook exited $rc when the rebuild launch failed (must be 0)"
# Line 1 = the commit's own launch, line 2 = the direct failing rerun.
launches="$(wc -l <"$STUB_LOG" | tr -d ' ')"
[[ $launches == 2 ]] || fail "failing-launch scenario never reached the launch (got $launches/2)"

# --- 5: outside a repo the dispatcher is a silent no-op -----------------------
rc=0
(cd "$tmp" && "$hooks_dir/post-commit") >/dev/null 2>&1 || rc=$?
[[ $rc -eq 0 ]] || fail "hook exited $rc outside a git repo (must be 0)"

echo "post-commit-graphify-dispatcher: OK"
