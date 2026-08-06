#!/usr/bin/env bash
# gitconfig-homebrew-prefix-derivation.sh, dot_gitconfig.tmpl must reach every
# Homebrew binary through one derived prefix rather than a literal /opt/homebrew.
#
# Why this file needs a guard of its own. The template names four absolute
# Homebrew paths, and one of them is [gpg] program. commit.gpgSign is true here,
# so on a machine whose Homebrew prefix is not /opt/homebrew (an Intel Mac at
# /usr/local, Linuxbrew at /home/linuxbrew/.linuxbrew) git cannot find the
# signing binary and EVERY commit aborts with "gpg failed to sign the data". The
# other three cost one command each: git difftool, and GitHub credentials on
# fetch and push.
#
# Why the checks render instead of grepping the paths. This template calls
# keepassxc for the signing key, so treefmt's shellcheck-rendered-template
# formatter excludes it and nothing else in this repo renders it either. A Go
# template mistake in the derivation would therefore surface first at
# `chezmoi apply`, against the operator's real ~/.gitconfig. The checks below
# replace the one vault action with a literal before handing anything to chezmoi,
# so no unlock is ever attempted and a broken derivation fails here instead.
#
# The invariants:
#   1. No literal Homebrew prefix outside the mapping. Pinning a call site back
#      to a literal is the regression this file exists to catch, and invariant 3
#      catches it as well, from the other side: a pinned site keeps answering
#      /opt/homebrew when the platform says otherwise.
#   2. The mapping answers every platform this repo can be applied on, checked by
#      rendering the mapping line itself against a list of platform strings. An
#      unknown platform must fall back rather than resolve to nothing, since an
#      empty prefix yields /bin/gpg, which exists on macOS and is not Homebrew's.
#   3. Each of the four call sites resolves to <prefix>/bin/<binary>, checked by
#      rendering the whole file with the platform forced to an Intel Mac and
#      parsing the result with git config --file, the same parser git uses at
#      runtime. Forcing the platform is what makes the answer identical on this
#      machine, on CI, and on the flake's x86_64-linux, and it exercises the case
#      the derivation exists for rather than the one this machine happens to be.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GITCONFIG_TEMPLATE="$REPO_ROOT/dot_gitconfig.tmpl"

# The line that declares the platform string, and the line that maps it to a
# prefix. Kept apart in the template so this test can force the platform without
# touching the mapping, which is the part under test. Both are Go template source
# searched for literally, so the `$` must not expand.
# shellcheck disable=SC2016
PLATFORM_DECLARATION='$platform :='
# shellcheck disable=SC2016
PREFIX_MAPPING='$homebrewPrefix :='

# Every prefix Homebrew installs under, so invariant 1 can say "no literal
# prefix, anywhere" rather than "no literal /opt/homebrew".
HOMEBREW_PREFIXES=(/opt/homebrew /usr/local /home/linuxbrew/.linuxbrew)

# What the mapping must answer, per platform. The two linux rows are not
# redundant: Homebrew installs at the same prefix on both linux architectures,
# and a mapping keyed on os/arch has to spell that out or leave one of them
# unanswered. The last row is a platform nothing here targets, pinning the
# fallback: an unmapped platform must still name a plausible prefix.
PLATFORMS=(darwin/arm64 darwin/amd64 linux/amd64 linux/arm64 plan9/386)
# The keys are quoted because an unquoted subscript is an arithmetic expression,
# and shfmt rewrites the `/` in one as a division operator.
declare -A EXPECTED_PREFIX_BY_PLATFORM=(
  ["darwin/arm64"]=/opt/homebrew
  ["darwin/amd64"]=/usr/local
  ["linux/amd64"]=/home/linuxbrew/.linuxbrew
  ["linux/arm64"]=/home/linuxbrew/.linuxbrew
  ["plan9/386"]=/home/linuxbrew/.linuxbrew
)

# The platform invariant 3 renders at, and the prefix it must produce. An Intel
# Mac is deliberately not this machine's platform.
RENDER_PLATFORM=darwin/amd64
RENDER_PREFIX=/usr/local

# The four call sites, as git config keys, with the Homebrew binary each must
# name. Config keys rather than line numbers, so the assertion survives any
# reordering of the file and reads what git itself would read.
CONFIG_KEYS=(
  gpg.program
  difftool.nvimdiff.cmd
  'credential.https://github.com.helper'
  'credential.https://gist.github.com.helper'
)
declare -A EXPECTED_BINARY_BY_CONFIG_KEY=(
  ['gpg.program']=gpg
  ['difftool.nvimdiff.cmd']=nvim
  ['credential.https://github.com.helper']=gh
  ['credential.https://gist.github.com.helper']=gh
)

# Stands in for the one keepassxc action, which cannot be rendered by automation.
# A path-shaped literal, so it cannot be mistaken for a resolved prefix.
VAULT_PLACEHOLDER=CHEZMOI_VAULT_PLACEHOLDER

# Bug class 11: git exports GIT_DIR and friends into every hook, and a stale one
# would redirect the config reads below at the wrong repository.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY \
  GIT_CONFIG GIT_CONFIG_GLOBAL GIT_CONFIG_SYSTEM GIT_CONFIG_COUNT GIT_PREFIX
export GIT_CONFIG_NOSYSTEM=1 GIT_PAGER=cat PAGER=cat

fail() {
  printf 'gitconfig-homebrew-prefix-derivation: FAIL -- %s\n' "$*" >&2
  exit 1
}

# Answers "which line numbers hold this fragment?", one per line. Fixed-string,
# because every fragment here carries regex metacharacters.
line_numbers_containing() { # <fragment> <file>
  grep -nF -- "$1" "$2" | cut -d: -f1
}

# Answers "how many lines hold this fragment?".
count_lines_containing() { # <fragment> <file>
  line_numbers_containing "$1" "$2" | grep -c . || true
}

# Renders a template through chezmoi with an isolated HOME, so the authoring
# machine's chezmoi configuration cannot change the answer. Read-only: it renders
# what is handed to it and never touches the destination state.
render_template() { # <input-file> <output-file>
  HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" \
    execute-template --no-tty <"$1" >"$2"
}

# Answers "which prefix does one call site resolve to?", printing the binary path
# the value starts with. A credential helper value carries git's `!` shell escape
# in front of the path, and the difftool value carries arguments after it, so the
# path is the first whitespace-separated token with any leading `!` removed. A
# key can legitimately hold several values (the helpers reset the list with an
# empty value first), and exactly one of them names a binary.
site_binary_path() { # <rendered-config> <config-key>
  local rendered="$1" key="$2" value token found="" matches=0
  while IFS= read -r value; do
    token="${value%%[[:space:]]*}"
    token="${token#!}"
    [[ $token == */bin/* ]] || continue
    matches=$((matches + 1))
    found="$token"
  done < <(git config --file "$rendered" --get-all "$key" || true)
  ((matches == 1)) || return 1
  printf '%s\n' "$found"
}

[[ -f $GITCONFIG_TEMPLATE ]] || fail "missing template: $GITCONFIG_TEMPLATE"
command -v git >/dev/null 2>&1 || fail "git is not on PATH"
command -v chezmoi >/dev/null 2>&1 ||
  fail "chezmoi is not on PATH; the derivation cannot be rendered, and grepping the template alone would not notice a Go template mistake in it"

work="$(mktemp -d)"
# `trash` is the operator's rule for interactive removals; a committed test has
# to run on a bare CI runner, where only coreutils exist.
trap 'rm -rf "$work"' EXIT
render_home="$work/render-home"
mkdir -p "$render_home"

# ---- the derivation exists, once ------------------------------------------
# Both halves are located before anything else runs, because every invariant
# below is stated in terms of them and a missing or duplicated declaration would
# otherwise surface as a confusing render failure.
for fragment in "$PLATFORM_DECLARATION" "$PREFIX_MAPPING"; do
  found="$(count_lines_containing "$fragment" "$GITCONFIG_TEMPLATE")"
  ((found == 1)) ||
    fail "expected exactly one line declaring '$fragment' in $GITCONFIG_TEMPLATE, found $found; the Homebrew prefix must be derived in one place and reused, so that no call site can drift onto a prefix of its own"
done
mapping_line_number="$(line_numbers_containing "$PREFIX_MAPPING" "$GITCONFIG_TEMPLATE")"

# ---- 1: no literal Homebrew prefix outside the mapping --------------------
for prefix in "${HOMEBREW_PREFIXES[@]}"; do
  while IFS= read -r hit; do
    ((hit == mapping_line_number)) ||
      fail "$GITCONFIG_TEMPLATE:$hit pins a literal Homebrew prefix ($prefix). Every absolute Homebrew path in this file has to come from the mapping on line $mapping_line_number, or the file works on one machine's prefix only"
  done < <(line_numbers_containing "$prefix" "$GITCONFIG_TEMPLATE")
done

# ---- 2: the mapping answers every platform --------------------------------
# The mapping line is used verbatim, with the platform supplied by a range rather
# than by the host, so one render covers every platform at once.
mapping_probe="$work/mapping-probe.tmpl"
# shellcheck disable=SC2016 # Go template variables, expanded by chezmoi, not here
{
  printf '{{ range $platform := list'
  printf ' "%s"' "${PLATFORMS[@]}"
  printf ' }}\n'
  grep -F -- "$PREFIX_MAPPING" "$GITCONFIG_TEMPLATE"
  printf '{{ $platform }} {{ $homebrewPrefix }}\n'
  printf '{{ end }}\n'
} >"$mapping_probe"

mapping_answers="$work/mapping-answers"
render_template "$mapping_probe" "$mapping_answers" ||
  fail "chezmoi could not render the prefix mapping from $GITCONFIG_TEMPLATE:$mapping_line_number; that same mistake would abort chezmoi apply on the real ~/.gitconfig"

for platform in "${PLATFORMS[@]}"; do
  expected="${EXPECTED_PREFIX_BY_PLATFORM[$platform]}"
  answered="$(awk -v want="$platform" '$1 == want { print $2 }' "$mapping_answers")"
  [[ $answered == "$expected" ]] ||
    fail "the mapping answers '$answered' for $platform, expected '$expected'; Homebrew's prefix on that platform is $expected, and a wrong or empty answer sends every path in this file at a directory Homebrew never installed to"
done

# ---- 3: every call site resolves through the mapping ----------------------
# The whole file, with the platform forced and the vault action neutralized, so
# the render needs no unlock and answers for a machine that is not this one.
render_input="$work/gitconfig-forced-platform.tmpl"
# shellcheck disable=SC2016 # a Go template variable, expanded by chezmoi, not here
printf '{{- $platform := "%s" -}}\n' "$RENDER_PLATFORM" >"$render_input"
grep -vF -- "$PLATFORM_DECLARATION" "$GITCONFIG_TEMPLATE" |
  sed -E "s/\{\{[^}]*keepassxc[^}]*\}\}/$VAULT_PLACEHOLDER/g" >>"$render_input"
grep -q 'keepassxc' "$render_input" &&
  fail "a keepassxc action survived neutralization; refusing to render $GITCONFIG_TEMPLATE, since doing so would try to unlock the vault"

rendered="$work/gitconfig-forced-platform"
render_template "$render_input" "$rendered" ||
  fail "chezmoi could not render $GITCONFIG_TEMPLATE for $RENDER_PLATFORM; that same mistake would abort chezmoi apply on the real ~/.gitconfig"
git config --file "$rendered" --list >/dev/null 2>&1 ||
  fail "the render of $GITCONFIG_TEMPLATE for $RENDER_PLATFORM is not parseable by git config --file"

for key in "${CONFIG_KEYS[@]}"; do
  binary="${EXPECTED_BINARY_BY_CONFIG_KEY[$key]}"
  path="$(site_binary_path "$rendered" "$key")" ||
    fail "$key does not name exactly one Homebrew binary in the $RENDER_PLATFORM render; expected one value naming $RENDER_PREFIX/bin/$binary"
  [[ $path == "$RENDER_PREFIX/bin/$binary" ]] ||
    fail "$key resolves to '$path' on $RENDER_PLATFORM, expected '$RENDER_PREFIX/bin/$binary'. A path that stayed at another prefix is pinned rather than derived, and on that machine this key names a binary that is not there"
done

printf 'gitconfig-homebrew-prefix-derivation: OK (one mapping, no literal prefixes, %d platforms mapped, %d call sites derived)\n' \
  "${#PLATFORMS[@]}" "${#CONFIG_KEYS[@]}"
