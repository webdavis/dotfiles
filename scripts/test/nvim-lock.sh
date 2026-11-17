#!/usr/bin/env bash
# Specs for the scripts that keep Neovim's lock file in this repo and its paired plugins on their program's commit.
# Every case builds its own repositories under one temporary folder, which the run removes when it ends.

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly REPO_ROOT
readonly merge_lock="$REPO_ROOT/scripts/nvim-lock-merge.sh"
readonly lock_is_complete="$REPO_ROOT/scripts/nvim-lock-is-complete.sh"
readonly publish_lock="$REPO_ROOT/scripts/nvim-lock-publish.sh"
readonly pair_plugins="$REPO_ROOT/scripts/nvim-paired-plugins.sh"
readonly lock=lock/lazy-lock.json

work="$(mktemp -d)"
readonly work
failures=0

# Specs build temporary repositories, which must inherit neither the hook variables git exports nor this machine's git settings.
isolate_git() {
  local name
  while IFS= read -r name; do
    unset "$name"
  done < <(env | sed -n 's/^\(GIT_[A-Za-z0-9_]*\)=.*/\1/p')
  export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null
}

expect() {
  local label=$1 got=$2 wanted=$3
  if [[ $got == "$wanted" ]]; then
    printf 'ok    %s\n' "$label"
    return
  fi
  printf 'FAIL  %s\n      got:    %s\n      wanted: %s\n' "$label" "$got" "$wanted"
  failures=$((failures + 1))
}

commit_of() {
  printf '%040d' "$1"
}

# Writes a lock file in lazy.nvim's own layout from "name=number" pairs.
lock_with() {
  local pair separator=''
  printf '{\n'
  for pair in "$@"; do
    printf '%s  "%s": { "branch": "main", "commit": "%s" }' "$separator" "${pair%%=*}" "$(commit_of "${pair##*=}")"
    separator=$',\n'
  done
  printf '\n}\n'
}

versions_in() {
  jq -c 'map_values(.commit | tonumber)' "$1" 2>/dev/null || printf 'unreadable'
}

# Stands in for Neovim when a script asks which plugins lazy.nvim installs from a remote.
# It prints install progress around the names, and can rewrite the lock first, as lazy.nvim does when it clones a new plugin.
install_fake_nvim() {
  mkdir -p "$work/bin"
  cat >"$work/bin/nvim" <<FAKE
#!/usr/bin/env bash
[[ -x "$work/neovim-installs-a-plugin" ]] && "$work/neovim-installs-a-plugin"
printf '[new.nvim]    clone | Receiving objects: 40%%'
sed 's/^/\\nremote-plugin /' "$work/spec-plugins"
printf '\\n[new.nvim]    clone | done\\n'
FAKE
  chmod +x "$work/bin/nvim"
  PATH="$work/bin:$PATH"
}

spec_has_plugins() {
  printf '%s\n' "$@" >"$work/spec-plugins"
}

neovim_says_nothing() {
  : >"$work/spec-plugins"
}

new_repo_pair() {
  local name=$1
  mkdir -p "$work/$name/mine/lock"
  cd "$work/$name/mine" || exit 1
  git init -q -b main .
  git config user.name spec
  git config user.email spec@example.invalid
  git config commit.gpgsign false
  git config pull.rebase true
  git config rebase.autoStash true
  mkdir -p scripts
  cp "$merge_lock" scripts/nvim-lock-merge.sh
  git config merge.lazylock.driver 'scripts/nvim-lock-merge.sh %O %A %B'
  printf '%s merge=lazylock\n' "$lock" >.gitattributes
  lock_with a=1 b=1 >"$lock"
  printf 'notes\n' >notes.txt
  git add -A
  git commit -qm start
  git init -q --bare -b main "$work/$name/remote.git"
  git remote add origin "$work/$name/remote.git"
  git push -q -u origin main
  git clone -q "$work/$name/remote.git" "$work/$name/theirs"
  git -C "$work/$name/theirs" config user.name other
  git -C "$work/$name/theirs" config user.email other@example.invalid
  git -C "$work/$name/theirs" config commit.gpgsign false
}

another_machine_publishes() {
  (
    cd ../theirs || exit 1
    git pull -q
    lock_with "$@" >"$lock"
    git commit -qam 'update plugins elsewhere'
    git push -q
  )
}

versions_on_main() {
  git fetch -q origin
  git show "origin/main:$lock" | jq -c 'map_values(.commit | tonumber)'
}

merged() {
  local base=$1 current=$2 incoming=$3
  printf '%s' "$base" >"$work/base"
  printf '%s' "$current" >"$work/current"
  printf '%s' "$incoming" >"$work/incoming"
  "$merge_lock" "$work/base" "$work/current" "$work/incoming"
  versions_in "$work/current"
}

merge_specs() {
  expect 'merge: each side changed a different plugin, both changes are kept' \
    "$(merged "$(lock_with a=1 b=1)" "$(lock_with a=2 b=1)" "$(lock_with a=1 b=3)")" '{"a":2,"b":3}'
  expect 'merge: both sides changed one plugin, the incoming side wins' \
    "$(merged "$(lock_with a=1)" "$(lock_with a=2)" "$(lock_with a=3)")" '{"a":3}'
  expect 'merge: a plugin one side removed stays removed' \
    "$(merged "$(lock_with a=1 b=1)" "$(lock_with a=1)" "$(lock_with a=2 b=1)")" '{"a":2}'
  expect 'merge: a plugin one side added is kept' \
    "$(merged "$(lock_with a=1)" "$(lock_with a=1 c=5)" "$(lock_with a=2)")" '{"a":2,"c":5}'
  expect 'merge: an unreadable incoming side is ignored' \
    "$(merged "$(lock_with a=1)" "$(lock_with a=2)" '<<<<<<< broken')" '{"a":2}'
  expect 'merge: an empty current side gives way to the incoming side' \
    "$(merged "$(lock_with a=1)" '{}' "$(lock_with a=2)")" '{"a":2}'
  expect 'merge: a file with no common ancestor still merges' \
    "$(merged '' "$(lock_with a=1)" "$(lock_with a=2 b=1)")" '{"a":2,"b":1}'
  merged "$(lock_with a=1 b=1)" "$(lock_with a=2 b=1)" "$(lock_with a=1 b=3)" >/dev/null
  expect 'merge: the result is laid out as lazy.nvim writes it' "$(<"$work/current")" "$(lock_with a=2 b=3)"

  new_repo_pair pull
  another_machine_publishes a=1 b=2
  lock_with a=3 b=1 >"$lock"
  git pull -q >/dev/null 2>&1
  expect 'merge: a pull over an unpublished change to a neighbouring line leaves a valid file' \
    "$(versions_in "$lock")" '{"a":3,"b":2}'
}

complete_specs() {
  local lock_file="$work/complete.json"
  spec_has_plugins a b
  lock_with a=1 b=1 >"$lock_file"
  expect 'complete: a lock with a line for every plugin passes' \
    "$("$lock_is_complete" "$lock_file" 2>&1 && printf passes)" 'passes'
  lock_with a=1 b=1 extra=1 >"$lock_file"
  expect 'complete: a line for a plugin that is switched off is allowed' \
    "$("$lock_is_complete" "$lock_file" 2>&1 && printf passes)" 'passes'
  lock_with a=1 >"$lock_file"
  expect 'complete: a plugin with no line fails and is named' \
    "$("$lock_is_complete" "$lock_file" 2>&1 || printf 'fails')" 'the lock file has no line for: b '$'\n''fails'
  printf '{}\n' >"$lock_file"
  expect 'complete: an empty lock fails' \
    "$("$lock_is_complete" "$lock_file" >/dev/null 2>&1 || printf fails)" 'fails'
  printf '{ "a": { "branch": "main", "commit": "abc123" } }\n' >"$lock_file"
  expect 'complete: a shortened commit fails' \
    "$("$lock_is_complete" "$lock_file" >/dev/null 2>&1 || printf fails)" 'fails'
  printf '<<<<<<< broken\n' >"$lock_file"
  expect 'complete: an unreadable lock fails' \
    "$("$lock_is_complete" "$lock_file" >/dev/null 2>&1 || printf fails)" 'fails'
  lock_with a=1 b=1 >"$lock_file"
  neovim_says_nothing
  expect 'complete: a lock cannot pass while Neovim names no plugins' \
    "$("$lock_is_complete" "$lock_file" 2>&1 || printf 'fails')" \
    'Neovim did not say which plugins it installs, so the lock file could not be checked'$'\n''fails'
  spec_has_plugins a b
}

publish_specs() {
  spec_has_plugins a b

  new_repo_pair publish
  lock_with a=2 b=1 >"$lock"
  printf 'unfinished\n' >>notes.txt
  expect 'publish: a changed lock is published' "$("$publish_lock" "$lock" 2>&1 | sed 's/ [0-9a-f]\{7\} / COMMIT /')" \
    'plugin versions: published COMMIT to main.'
  expect 'publish: main now holds the new versions' "$(versions_on_main)" '{"a":2,"b":1}'
  expect 'publish: the commit holds the lock file and nothing else' \
    "$(git diff --name-only origin/main~1 origin/main | tr '\n' ' ')" "$lock "
  expect 'publish: unfinished work in the checkout is left alone' \
    "$(git status --short | tr '\n' ' ')" " M $lock  M notes.txt "
  expect 'publish: a second apply has nothing to say' "$("$publish_lock" "$lock" 2>&1)" ''
  lock_with a=3 b=1 >"$lock"
  "$publish_lock" "$lock" >/dev/null 2>&1
  expect 'publish: a further update before any pull is published too' "$(versions_on_main)" '{"a":3,"b":1}'
  git pull -q >/dev/null 2>&1
  expect 'publish: after a pull only the unfinished work remains' "$(git status --short | tr '\n' ' ')" ' M notes.txt '

  new_repo_pair stale
  another_machine_publishes a=1 b=9
  lock_with a=5 b=1 >"$lock"
  expect 'publish: versions another machine published are not overwritten' "$("$publish_lock" "$lock" 2>&1)" \
    'plugin versions: main holds versions this checkout has not pulled; pull, then apply again.'
  expect 'publish: main keeps the other machine'"'"'s versions' "$(versions_on_main)" '{"a":1,"b":9}'
  git pull -q >/dev/null 2>&1
  "$publish_lock" "$lock" >/dev/null 2>&1
  expect 'publish: after the pull both machines'"'"' updates reach main' "$(versions_on_main)" '{"a":5,"b":9}'

  new_repo_pair rollback
  lock_with a=2 b=1 >"$lock"
  "$publish_lock" "$lock" >/dev/null 2>&1
  lock_with a=1 b=1 >"$lock"
  "$publish_lock" "$lock" >/dev/null 2>&1
  expect 'publish: putting the old file back publishes the rollback' "$(versions_on_main)" '{"a":1,"b":1}'

  new_repo_pair second-machine
  lock_with a=2 b=1 >"$lock"
  "$publish_lock" "$lock" >/dev/null 2>&1
  another_machine_publishes a=3 b=1
  git pull -q >/dev/null 2>&1
  expect 'publish: a newer version from another machine replaces the one this checkout published' \
    "$(versions_in "$lock")" '{"a":3,"b":1}'
  expect 'publish: and the older one is not published again' "$("$publish_lock" "$lock" 2>&1)$(versions_on_main)" '{"a":3,"b":1}'

  new_repo_pair refused
  printf '#!/bin/sh\necho "main is locked" >&2\nexit 1\n' >"$work/refused/remote.git/hooks/pre-receive"
  chmod +x "$work/refused/remote.git/hooks/pre-receive"
  lock_with a=2 b=1 >"$lock"
  expect 'publish: a push the remote refuses is reported with its reason' \
    "$("$publish_lock" "$lock" 2>&1 | grep -c 'error\[push-refused\].*main is locked')" '1'

  new_repo_pair new-plugin
  lock_with a=2 >"$lock"
  printf '#!/usr/bin/env bash\nprintf %%s %q >%q\n' "$(lock_with a=2 b=7)"$'\n' "$work/new-plugin/mine/$lock" >"$work/neovim-installs-a-plugin"
  chmod +x "$work/neovim-installs-a-plugin"
  "$publish_lock" "$lock" >/dev/null 2>&1
  chmod -x "$work/neovim-installs-a-plugin"
  expect 'publish: a plugin Neovim installs during the check is in the published lock' "$(versions_on_main)" '{"a":2,"b":7}'

  new_repo_pair incomplete
  lock_with a=2 >"$lock"
  expect 'publish: a lock that lost a plugin still in use is not published' "$("$publish_lock" "$lock" 2>&1)" \
    "the lock file has no line for: b "$'\n''plugin versions: error[incomplete-lock]: not published.'
  expect 'publish: main is untouched by the incomplete lock' "$(versions_on_main)" '{"a":1,"b":1}'

  new_repo_pair offline
  lock_with a=2 b=1 >"$lock"
  git remote set-url origin "$work/offline/nowhere.git"
  expect 'publish: an unreachable remote is reported and retried later' "$(
    "$publish_lock" "$lock" 2>&1
    printf 'exit=%s' $?
  )" \
    'plugin versions: cannot reach the remote; the next apply retries.'$'\n''exit=0'
}

# Builds a repository standing in for a tool's GitHub repo, and prints the commit of its newest version.
new_tool_repo() {
  local name=$1 folder="$work/github/acme/$1"
  mkdir -p "$folder/lua"
  (
    cd "$folder" || exit 1
    git init -q -b main .
    git config user.name spec
    git config user.email spec@example.invalid
    git config commit.gpgsign false
    git config uploadpack.allowAnySHA1InWant true
    printf 'return 1\n' >lua/init.lua
    git add -A
    git commit -qm one
    printf 'return 2\n' >lua/init.lua
    git commit -qam two
    git rev-parse HEAD
  )
}

cargo_has_installed() {
  local entries='' pair
  for pair in "$@"; do
    entries+="${entries:+,}\"${pair%%=*} 0.1.0 (git+https://github.com/acme/${pair%%=*}#${pair##*=})\":{}"
  done
  printf '{"installs":{%s}}\n' "$entries" >"$work/crates.json"
}

plugin_commit() {
  git -C "$work/paired/$1" rev-parse -q --verify HEAD 2>/dev/null || printf 'none'
}

paired_plugin_specs() {
  local newest older
  newest="$(new_tool_repo tool)"
  older="$(git -C "$work/github/acme/tool" rev-parse HEAD~1)"
  new_tool_repo other >/dev/null
  export CARGO_INSTALLED_CRATES_RECORD="$work/crates.json" CARGO_GIT_DATABASE="$work/cargo-db"
  export GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0="url.$work/github/.insteadOf" GIT_CONFIG_VALUE_0='https://github.com/'

  cargo_has_installed "tool=$older"
  expect 'paired: the plugin is put on the commit the program was built from' \
    "$("$pair_plugins" "$work/paired" acme/tool 2>&1)" "tool: plugin now matches the program at ${older:0:7}"
  expect 'paired: the checkout is on that commit' "$(plugin_commit tool)" "$older"
  expect 'paired: a second apply has nothing to say' "$("$pair_plugins" "$work/paired" acme/tool 2>&1)" ''
  cargo_has_installed "tool=$newest"
  "$pair_plugins" "$work/paired" acme/tool >/dev/null 2>&1
  expect 'paired: a rebuilt program moves its plugin' "$(plugin_commit tool)" "$newest"

  mkdir -p "$work/cargo-db"
  git clone -q --bare "$work/github/acme/other" "$work/cargo-db/other-0123456789abcdef"
  cargo_has_installed "other=$(git -C "$work/github/acme/other" rev-parse HEAD)"
  GIT_CONFIG_VALUE_0='https://unreachable.invalid/' "$pair_plugins" "$work/paired" acme/other >/dev/null 2>&1
  expect 'paired: the commit comes from cargo'"'"'s own download when the network is away' \
    "$(plugin_commit other)" "$(git -C "$work/github/acme/other" rev-parse HEAD)"

  cargo_has_installed "tool=$(commit_of 7)" "other=$(git -C "$work/github/acme/other" rev-parse HEAD~1)"
  expect 'paired: a commit that cannot be fetched is reported and fails' \
    "$(
      "$pair_plugins" "$work/paired" acme/tool acme/other 2>&1
      printf 'exit=%s' $?
    )" \
    "tool: error[paired-plugin]: could not fetch $(commit_of 7), so the plugin stays as it is; the next apply retries."$'\n'"other: plugin now matches the program at $(git -C "$work/github/acme/other" rev-parse --short=7 HEAD~1)"$'\n''exit=1'
  expect 'paired: that plugin stays where it was' "$(plugin_commit tool)" "$newest"
  expect 'paired: the plugins after it are still handled' \
    "$(plugin_commit other)" "$(git -C "$work/github/acme/other" rev-parse HEAD~1)"

  cargo_has_installed
  expect 'paired: a program that is not installed gets no plugin' \
    "$(
      "$pair_plugins" "$work/paired" acme/absent 2>&1
      printf 'exit=%s %s' $? "$(plugin_commit absent)"
    )" 'exit=0 none'
  unset GIT_CONFIG_COUNT GIT_CONFIG_KEY_0 GIT_CONFIG_VALUE_0
}

# The repositories are this run's own, made under a folder the run created.
remove_work_folder() {
  find "$work" -delete
}

main() {
  trap remove_work_folder EXIT
  isolate_git
  install_fake_nvim
  merge_specs
  complete_specs
  publish_specs
  paired_plugin_specs
  if ((failures > 0)); then
    printf '\n%s failed\n' "$failures"
    exit 1
  fi
}

main "$@"
