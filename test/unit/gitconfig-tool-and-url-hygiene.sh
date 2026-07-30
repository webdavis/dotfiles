#!/usr/bin/env bash
# gitconfig-tool-and-url-hygiene.sh, four invariants of dot_gitconfig.tmpl that
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
#   2. No tool is PINNED to a literal binary path. A <diff|merge>tool.<name>.path
#      key drops git's $PATH lookup for a filesystem path that has no fallback,
#      so any machine with a different install prefix loses the tool even with a
#      working one on $PATH. This is a key-shape question, exactly like invariant
#      3, and is answered from the same key listing: the regression is the
#      PRESENCE of the key, so nothing here touches the filesystem and the answer
#      cannot differ between this machine, CI, and the flake's x86_64-linux.
#   3. No url rewrite TARGETS the git:// protocol. GitHub permanently disabled
#      it on 2022-03-15, so a `[url "git://..."]` base sends fetches at a port
#      that no longer answers. A git:// prefix on the insteadOf VALUE side is
#      fine and deliberate: that rescues a legacy remote by rewriting it away.
#   4. Global ignores resolve to a file this repo actually deploys. git reads
#      THREE states out of core.excludesfile, not two, and this invariant covers
#      all three: ABSENT loads git's documented default, SET loads exactly that
#      path, and PRESENT-BUT-EMPTY loads nothing at all, reproducing the original
#      defect this file dropped the key to fix. Deployment is decided by `chezmoi
#      managed`, not by the source file's presence on disk: a source file hidden
#      behind .chezmoiignore is present but never delivered, and git would then
#      load no global ignores either. Being LISTED by chezmoi is not enough on
#      its own: the delivered path has to resolve to a readable regular file,
#      because two of the shapes chezmoi can legitimately deliver are not one.
#      Measured on git 2.55.0 with `git check-ignore -v` against a matching path:
#        symlink -> regular file : the patterns apply, exit 0
#        symlink -> missing      : nothing applies, exit 1, global ignores are
#                                  dead and nothing on stderr says so
#        symlink -> directory    : "fatal: cannot use <path> as an exclude file",
#                                  exit 128
#      So the delivered shape is classified, one arm per state, and anything that
#      is not a readable regular file is a failure with its own message.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GITCONFIG_TEMPLATE="$REPO_ROOT/dot_gitconfig.tmpl"
GIT_DEFAULT_EXCLUDES_SOURCE="dot_config/git/ignore"
# Where git looks when core.excludesFile is unset. git consults XDG_CONFIG_HOME
# only when it is set AND non-empty, and uses $HOME/.config otherwise, which is
# precisely what bash's `:-` does (measured on git 2.55.0 with `git check-ignore`
# for all three states of the variable: pointed elsewhere, set empty, unset).
# Deriving the path instead of hardcoding $HOME/.config is what lets this test
# notice an environment in which the file this repo deploys is not the file git
# would read. dot_bashrc.tmpl and dot_profile both export $HOME/.config, so on a
# machine this repo has deployed the two agree.
GIT_DEFAULT_EXCLUDES_TARGET="${XDG_CONFIG_HOME:-$HOME/.config}/git/ignore"
# The two chezmoi entry types that can put a readable file at a target path, kept
# apart because they are answerable from different places. A `files` entry writes
# a regular file, which is settled by the source state alone, so its verdict is
# the same on this machine, in CI, and on a machine chezmoi has never run on. A
# `symlinks` entry writes a symlink, and where that symlink lands is NOT in the
# source state: git follows one to a regular file, reads nothing at all through a
# dangling one, and refuses one that points at a directory, so it has to be
# resolved on the machine. Every other entry type (dirs, scripts, remove) puts no
# readable file anywhere, which is why this is not simply `all`.
CHEZMOI_REGULAR_FILE_ENTRY_TYPE="files"
CHEZMOI_SYMLINK_ENTRY_TYPE="symlinks"
# `git config --get` exits 1 when the key is ABSENT and 0 when it is present,
# including when its value is the empty string (measured). Both print nothing on
# stdout, so this exit code is the only signal separating two states that mean
# opposite things to git.
GIT_CONFIG_KEY_ABSENT_EXIT_CODE=1
PLACEHOLDER="CHEZMOI_TEMPLATE_PLACEHOLDER"
# Go template actions that produce control flow rather than a value. A literal
# placeholder cannot stand in for one, so their presence means this test's
# neutralizer no longer models the file and must be taught to.
CONTROL_FLOW_ACTIONS='{{-?[[:space:]]*(if|else|end|range|with|block|define|template)\b'

# Bug class 11: git exports GIT_DIR and friends into every hook, and a stale one
# would redirect the config reads below at the wrong repository. XDG_CONFIG_HOME
# is deliberately NOT scrubbed: it is an input to the answer rather than a
# redirection of it, and is read above through git's own rule. chezmoi needs no
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

# Answers "which absolute target paths does chezmoi deliver, of one entry type,
# from this source tree?". This is the authority on deployment: it applies
# .chezmoiignore and every source-name prefix, neither of which a source-path
# existence test can see. Read-only, and it lists the target state without
# rendering file contents, so the keepassxc templates here are never unlocked.
list_chezmoi_delivered_target_paths() {
  local entry_type="$1"
  chezmoi managed \
    --source "$REPO_ROOT" \
    --destination "$HOME" \
    --include="$entry_type" \
    --path-style=absolute
}

# Answers "is this exact target path in this listing?".
listing_contains_target_path() {
  local listing="$1" target="$2"
  printf '%s\n' "$listing" | grep -Fxq -- "$target"
}

# Answers "what does this path resolve to on this machine?", printing exactly one
# of: readable-file, dangling-symlink, directory, missing, not-a-regular-file,
# unreadable. Only readable-file is a file git can load as an exclude file; every
# other state gets its own arm, so a shape nobody anticipated cannot fall through
# to a pass. A symlink loop reports dangling-symlink, which is what it is in
# effect: the path resolves to nothing.
classify_path_resolution() {
  local path="$1"
  if [[ -L $path && ! -e $path ]]; then
    printf 'dangling-symlink\n'
  elif [[ ! -e $path ]]; then
    printf 'missing\n'
  elif [[ -d $path ]]; then
    printf 'directory\n'
  elif [[ ! -f $path ]]; then
    printf 'not-a-regular-file\n'
  elif [[ ! -r $path ]]; then
    printf 'unreadable\n'
  else
    printf 'readable-file\n'
  fi
}

# Answers "does this repo put a file git could load at this target path?",
# printing readable-file when it does, not-delivered when chezmoi delivers
# nothing there, and otherwise the resolution state that disqualifies it. The two
# listings are arguments rather than globals so this stays a pure function of its
# inputs and the fixtures below can hand it a listing of their own.
classify_delivered_target_path() {
  local regular_file_listing="$1" symlink_listing="$2" target="$3"
  if listing_contains_target_path "$regular_file_listing" "$target"; then
    # A regular-file entry needs no filesystem lookup: chezmoi writes a regular
    # file at that path by construction, whether or not it has run here yet.
    printf 'readable-file\n'
  elif listing_contains_target_path "$symlink_listing" "$target"; then
    classify_path_resolution "$target"
  else
    printf 'not-delivered\n'
  fi
}

# Answers "why is this target path not a file git can load?" for one rejected
# verdict. Separate from the classifier so the verdict stays a pure function of
# the path and both invariant-4 arms share one set of explanations.
explain_rejected_delivery() {
  local verdict="$1" target="$2"
  case "$verdict" in
    not-delivered)
      printf 'chezmoi delivers nothing to %s (present in the source tree is not enough; check .chezmoiignore)' "$target"
      ;;
    dangling-symlink)
      printf 'chezmoi delivers %s as a symlink that points at nothing, and git reads no patterns through it at all: measured on git 2.55.0, check-ignore exits 1 and matches nothing, with no diagnostic' "$target"
      ;;
    directory)
      printf 'chezmoi delivers %s as a symlink to a directory, which git refuses outright: measured on git 2.55.0, "fatal: cannot use %s as an exclude file"' "$target" "$target"
      ;;
    missing)
      printf 'chezmoi delivers a symlink to %s but nothing exists there, so this machine cannot say what it resolves to; apply the symlink, then re-run' "$target"
      ;;
    not-a-regular-file)
      printf '%s exists but is not a regular file, so git cannot read exclude patterns out of it' "$target"
      ;;
    unreadable)
      printf '%s is a regular file this user cannot read, so git loads no patterns from it' "$target"
      ;;
    *)
      printf 'the delivered shape of %s classified as an unrecognized verdict (%s); classify_delivered_target_path and explain_rejected_delivery have drifted apart' "$target" "$verdict"
      ;;
  esac
}

# Fails the test unless this repo puts a readable regular file at the target
# path. The single place the passing verdict is named, so the two invariant-4
# arms cannot drift apart on what counts as delivered.
require_delivered_readable_file() {
  local target="$1" preamble="$2" verdict
  verdict="$(classify_delivered_target_path \
    "$DELIVERED_REGULAR_FILE_TARGET_PATHS" "$DELIVERED_SYMLINK_TARGET_PATHS" "$target")"
  [[ $verdict == readable-file ]] ||
    fail "$preamble: $(explain_rejected_delivery "$verdict" "$target")"
}

# Answers "does this config key pin a diff or merge tool to a literal binary
# path?". Key shape alone decides it, so the verdict is the same everywhere.
is_tool_binary_path_key() {
  local key="$1"
  [[ $key == difftool.*.path || $key == mergetool.*.path ]]
}

# Answers "which of git's three core.excludesfile states does this config hold?",
# printing one of: absent, set, empty, unreadable. Separating absent from empty
# is the point: they read identically on stdout and mean opposite things to git.
classify_core_excludesfile() {
  local config="$1" value exit_code=0
  value="$(git config --file "$config" --get core.excludesfile)" || exit_code=$?
  if ((exit_code == GIT_CONFIG_KEY_ABSENT_EXIT_CODE)); then
    printf 'absent\n'
  elif ((exit_code != 0)); then
    printf 'unreadable\n'
  elif [[ -z $value ]]; then
    printf 'empty\n'
  else
    printf 'set\n'
  fi
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
  fail "chezmoi is not on PATH; invariant 4 cannot tell which target paths this repo delivers"

# Fail closed: an unreadable or empty listing must not read as "nothing is
# missing". Collected once, since every branch of invariant 4 consults them. Only
# the regular-file listing is required to be non-empty: this repo certainly
# delivers regular files, so an empty one means chezmoi read the wrong source
# tree, while a repo with no symlink entries at all is a legitimate state.
DELIVERED_REGULAR_FILE_TARGET_PATHS="$(list_chezmoi_delivered_target_paths "$CHEZMOI_REGULAR_FILE_ENTRY_TYPE")" ||
  fail "chezmoi managed failed against source $REPO_ROOT; invariant 4 cannot be decided"
[[ -n $DELIVERED_REGULAR_FILE_TARGET_PATHS ]] ||
  fail "chezmoi managed listed no file entries at all for source $REPO_ROOT; invariant 4 cannot be decided"
DELIVERED_SYMLINK_TARGET_PATHS="$(list_chezmoi_delivered_target_paths "$CHEZMOI_SYMLINK_ENTRY_TYPE")" ||
  fail "chezmoi managed failed to list symlink entries for source $REPO_ROOT; invariant 4 cannot be decided"

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

# ---- 2 and 3: key-shape prohibitions, decided from one key listing ---------
while IFS= read -r key; do
  is_tool_binary_path_key "$key" &&
    fail "$key pins a tool to a literal binary path, dropping git's \$PATH lookup for a path with no fallback. Measured on git 2.55.0 against a nonexistent path, with a working nvim on \$PATH the whole time: git mergetool exits 1 and leaves the file conflicted, git difftool exits 128, both reporting 'is not available as'. Let git resolve the tool name through \$PATH instead"
  [[ $key == url.git://* ]] &&
    fail "a url rewrite targets the git:// protocol ($key); GitHub permanently disabled it on 2022-03-15, so fetches through this base cannot connect"
done < <(git config --file "$parsed" --name-only --list)

# ---- 4a: the delivered-shape classifier discriminates ---------------------
# The three symlink shapes chezmoi can legitimately deliver, one fixture each,
# because only the first of them is an ignore file git can load and the other two
# are the states a bare "is it listed?" test silently admitted. Two non-symlink
# controls sit alongside them so a classifier that answered readable-file (or
# directory) for everything is caught here too, at the predicate, rather than
# twenty lines later as a mystery verdict from the real config.
fixtures="$work/path-resolution-fixtures"
mkdir -p "$fixtures/a-directory"
printf 'ignored-pattern\n' >"$fixtures/a-regular-file"
ln -s "$fixtures/a-regular-file" "$fixtures/symlink-to-file"
ln -s "$fixtures/nothing-here" "$fixtures/symlink-to-missing"
ln -s "$fixtures/a-directory" "$fixtures/symlink-to-directory"

assert_path_resolution() {
  local name="$1" expected="$2" actual
  actual="$(classify_path_resolution "$fixtures/$name")"
  [[ $actual == "$expected" ]] ||
    fail "classify_path_resolution answered '$actual' for the $name fixture, expected '$expected'; the check that keeps a symlink-delivered core.excludesfile honest no longer discriminates the shapes git treats differently"
}
assert_path_resolution symlink-to-file readable-file
assert_path_resolution symlink-to-missing dangling-symlink
assert_path_resolution symlink-to-directory directory
assert_path_resolution a-regular-file readable-file
assert_path_resolution a-directory directory
assert_path_resolution nothing-here missing

# The same three shapes again, this time through the function invariant 4 calls,
# with a synthetic chezmoi listing. Classifying a path correctly is worth nothing
# if the delivery check never consults the classifier, which is exactly how a
# widened `--include=files,symlinks` membership test passed a dangling symlink.
assert_delivered_classification() {
  local regular_file_listing="$1" symlink_listing="$2" name="$3" expected="$4" actual
  actual="$(classify_delivered_target_path \
    "$regular_file_listing" "$symlink_listing" "$fixtures/$name")"
  [[ $actual == "$expected" ]] ||
    fail "classify_delivered_target_path answered '$actual' for the $name fixture, expected '$expected'; a chezmoi entry is being accepted or rejected on membership alone rather than on what it resolves to"
}
assert_delivered_classification "" "$fixtures/symlink-to-file" symlink-to-file readable-file
assert_delivered_classification "" "$fixtures/symlink-to-missing" symlink-to-missing dangling-symlink
assert_delivered_classification "" "$fixtures/symlink-to-directory" symlink-to-directory directory
assert_delivered_classification "" "" symlink-to-file not-delivered
# A regular-file entry is trusted from the source state alone, deliberately: it
# is what keeps the verdict identical on this machine, in CI, and on a machine
# chezmoi has never run on. Pinned so the asymmetry stays a decision.
assert_delivered_classification "$fixtures/nothing-here" "" nothing-here readable-file

# The guard the two invariant-4 arms call must act on those verdicts. fail() exits,
# so a subshell is the only way to observe that it fired; `if` runs the subshell
# without set -e deciding the outcome for us.
assert_guard_verdict() {
  local symlink_listing="$1" name="$2" expected="$3" actual=accepted
  if ! (
    DELIVERED_REGULAR_FILE_TARGET_PATHS=""
    DELIVERED_SYMLINK_TARGET_PATHS="$symlink_listing"
    require_delivered_readable_file "$fixtures/$name" "fixture probe" 2>/dev/null
  ); then
    actual=rejected
  fi
  [[ $actual == "$expected" ]] ||
    fail "require_delivered_readable_file $actual the $name fixture, expected $expected; invariant 4 no longer acts on the verdict it computes"
}
assert_guard_verdict "$fixtures/symlink-to-file" symlink-to-file accepted
assert_guard_verdict "$fixtures/symlink-to-missing" symlink-to-missing rejected
assert_guard_verdict "$fixtures/symlink-to-directory" symlink-to-directory rejected
assert_guard_verdict "" symlink-to-file rejected

# ---- 4b: global ignores resolve to a file this repo deploys ---------------
case "$(classify_core_excludesfile "$parsed")" in
  set)
    excludes="$(git config --file "$parsed" --get core.excludesfile)"
    require_delivered_readable_file "${excludes/#\~/$HOME}" \
      "core.excludesfile = '$excludes' does not name a readable file this repo delivers, so git would load no global ignores at all"
    ;;
  empty)
    fail "core.excludesfile is present with an empty value, which git does not treat as absent: it loads no global ignores at all rather than falling back to $GIT_DEFAULT_EXCLUDES_TARGET (measured with git check-ignore), which is the exact defect this file dropped the key to fix. Remove the key, do not blank it"
    ;;
  absent)
    require_delivered_readable_file "$GIT_DEFAULT_EXCLUDES_TARGET" \
      "core.excludesfile is unset, so git falls back to $GIT_DEFAULT_EXCLUDES_TARGET, which this repo ships from $GIT_DEFAULT_EXCLUDES_SOURCE, but global ignores would be empty on a fresh machine"
    ;;
  *)
    fail "core.excludesfile could not be read out of $parsed; invariant 4 cannot be decided"
    ;;
esac

printf 'gitconfig-tool-and-url-hygiene: OK (diff.tool/merge.tool resolve, no tool pinned to a binary path, no git:// rewrite target, global ignores are deployed)\n'
