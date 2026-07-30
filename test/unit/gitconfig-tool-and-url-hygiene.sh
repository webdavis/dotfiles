#!/usr/bin/env bash
# gitconfig-tool-and-url-hygiene.sh, three invariants of dot_gitconfig.tmpl that
# git itself can answer, asserted by handing the file to git rather than by
# grepping it.
#
# dot_gitconfig.tmpl calls keepassxc for the signing key, so it can never be
# rendered by automation (that is also why the rendered-template shellcheck
# formatter excludes it). This test does not render it: it neutralizes the
# template directives into a literal placeholder and parses the result with
# `git config --file`, which is the same parser git uses at runtime.
#
# The invariants:
#   1. diff.tool and merge.tool name a tool git can RESOLVE. Both keys take a
#      tool name; a command string in either makes git report an unknown tool,
#      reset to its default, and guess a different editor. `git <mode>tool
#      --tool-help` is the authority on the answer, and it lists built-ins
#      (available or not) as `<name>` and user-defined tools as `<name>.cmd`,
#      so one lookup covers both ways of being resolvable.
#   2. No url rewrite TARGETS the git:// protocol. GitHub permanently disabled
#      it on 2022-03-15, so a `[url "git://..."]` base sends fetches at a port
#      that no longer answers. A git:// prefix on the insteadOf VALUE side is
#      fine and deliberate: that rescues a legacy remote by rewriting it away.
#   3. Global ignores resolve to a file this repo actually deploys. Either
#      core.excludesfile names a deployed path, or the key is absent and the
#      repo deploys git's documented default, dot_config/git/ignore. Deployment
#      is decided by `chezmoi managed`, not by the source file's presence on
#      disk: a source file hidden behind .chezmoiignore is present but never
#      delivered, and git would then load no global ignores at all, which is the
#      failure this invariant exists to catch.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GITCONFIG_TEMPLATE="$REPO_ROOT/dot_gitconfig.tmpl"
# git's documented default for core.excludesFile is $XDG_CONFIG_HOME/git/ignore,
# and dot_bashrc.tmpl / dot_profile both export XDG_CONFIG_HOME=$HOME/.config.
XDG_IGNORE_TARGET="$HOME/.config/git/ignore"
XDG_IGNORE_SOURCE="dot_config/git/ignore"
PLACEHOLDER="CHEZMOI_TEMPLATE_PLACEHOLDER"
# Go template actions that produce control flow rather than a value. A literal
# placeholder cannot stand in for one, so their presence means this test's
# neutralizer no longer models the file and must be taught to.
CONTROL_FLOW_ACTIONS='{{-?[[:space:]]*(if|else|end|range|with|block|define|template)\b'

# Bug class 11: git exports GIT_DIR and friends into every hook, and a stale one
# would redirect the config reads below at the wrong repository. chezmoi needs no
# equivalent scrub: it has no environment override for sourceDir or destDir
# (measured: CHEZMOI_SOURCE_DIR is ignored), and both are passed explicitly
# below, so the only environment input left is where its own config file is
# found, which the listing does not depend on.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY \
  GIT_CONFIG GIT_CONFIG_GLOBAL GIT_CONFIG_SYSTEM GIT_CONFIG_COUNT GIT_PREFIX
export GIT_CONFIG_NOSYSTEM=1 GIT_PAGER=cat PAGER=cat

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# Answers "which absolute target paths does chezmoi deliver as files from this
# source tree?". This is the authority on deployment: it applies .chezmoiignore
# and every source-name prefix, neither of which a source-path existence test
# can see. Read-only, and it lists the target state without rendering file
# contents, so the keepassxc templates in this repo are never unlocked.
list_chezmoi_delivered_target_paths() {
  chezmoi managed \
    --source "$REPO_ROOT" \
    --destination "$HOME" \
    --include=files \
    --path-style=absolute
}

# Answers "does chezmoi deliver exactly this target path?".
chezmoi_delivers_target_path() {
  local target="$1"
  printf '%s\n' "$DELIVERED_TARGET_PATHS" | grep -Fxq -- "$target"
}

# Answers "can git resolve this tool name?" for the given mode, by asking git.
git_resolves_tool_name() {
  local mode="$1" name="$2" config="$3" first
  while IFS= read -r first; do
    [[ $first == "$name" || $first == "$name.cmd" ]] && return 0
  done < <(GIT_CONFIG_GLOBAL="$config" git "${mode}tool" --tool-help 2>/dev/null |
    awk '{print $1}')
  return 1
}

[[ -f $GITCONFIG_TEMPLATE ]] || fail "missing template: $GITCONFIG_TEMPLATE"
command -v git >/dev/null 2>&1 || fail "git is not on PATH"
command -v chezmoi >/dev/null 2>&1 ||
  fail "chezmoi is not on PATH; invariant 3 cannot tell which target paths this repo delivers"

# Fail closed: an unreadable or empty listing must not read as "nothing is
# missing". Collected once, since both branches of invariant 3 consult it.
DELIVERED_TARGET_PATHS="$(list_chezmoi_delivered_target_paths)" ||
  fail "chezmoi managed failed against source $REPO_ROOT; invariant 3 cannot be decided"
[[ -n $DELIVERED_TARGET_PATHS ]] ||
  fail "chezmoi managed listed no files at all for source $REPO_ROOT; invariant 3 cannot be decided"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
parsed="$work/gitconfig"

# Fail closed: neutralizing a control-flow action into a literal would silently
# change which sections the parser sees.
if grep -Eq "$CONTROL_FLOW_ACTIONS" "$GITCONFIG_TEMPLATE"; then
  fail "$GITCONFIG_TEMPLATE now uses Go template control flow; this test's neutralizer only handles value actions and must be updated"
fi
sed -E "s/\{\{[^}]*\}\}/$PLACEHOLDER/g" "$GITCONFIG_TEMPLATE" >"$parsed"
grep -q '{{' "$parsed" && fail "template directives survived neutralization; the parsed config is not trustworthy"
git config --file "$parsed" --list >/dev/null 2>&1 ||
  fail "the neutralized template is not parseable by git config --file"

# ---- 1: diff.tool and merge.tool name a tool git can resolve ---------------
for mode in diff merge; do
  tool="$(git config --file "$parsed" --get "$mode.tool" || true)"
  [[ -n $tool ]] || fail "$mode.tool is unset; a resolvable tool name is expected"
  git_resolves_tool_name "$mode" "$tool" "$parsed" ||
    fail "$mode.tool = '$tool' is not a tool git can resolve; git ${mode}tool would report an unknown tool and guess a different editor. Use a built-in name, or add ${mode}tool.<name>.cmd"
done

# ---- 2: no url rewrite targets the dead git:// protocol --------------------
while IFS= read -r key; do
  [[ $key == url.git://* ]] &&
    fail "a url rewrite targets the git:// protocol ($key); GitHub permanently disabled it on 2022-03-15, so fetches through this base cannot connect"
done < <(git config --file "$parsed" --name-only --list)

# ---- 3: global ignores resolve to a file this repo deploys ----------------
excludes="$(git config --file "$parsed" --get core.excludesfile || true)"
if [[ -n $excludes ]]; then
  chezmoi_delivers_target_path "${excludes/#\~/$HOME}" ||
    fail "core.excludesfile = '$excludes' names a path chezmoi does not deliver, so git would load no global ignores at all"
else
  chezmoi_delivers_target_path "$XDG_IGNORE_TARGET" ||
    fail "core.excludesfile is unset, so git falls back to $XDG_IGNORE_TARGET, but chezmoi does not deliver that path from $XDG_IGNORE_SOURCE (present in the source tree is not enough; check .chezmoiignore); global ignores would be empty on a fresh machine"
fi

printf 'gitconfig-tool-and-url-hygiene: OK (diff.tool/merge.tool resolve, no git:// rewrite target, global ignores are deployed)\n'
