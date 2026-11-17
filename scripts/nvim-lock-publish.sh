#!/usr/bin/env bash
# Publishes the lock file to main as a commit that holds only that file, when the plugins it lists have changed.
# It runs inside the repo and takes the lock file's path in the repo.
# Nothing here is fatal: when it cannot publish it says why and the next apply tries again.

set -uo pipefail

readonly lock=$1
scripts_folder="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly scripts_folder

blob_at() {
  local revision=$1
  git rev-parse -q --verify "$revision:$lock" 2>/dev/null || true
}

say() {
  printf 'plugin versions: %s\n' "$*"
}

# Kept per checkout, because a second worktree that has not pulled must not take this one's publish for its own.
published_marker() {
  printf '%s/nvim-lock-published\n' "$(git rev-parse --git-dir)"
}

last_published_blob() {
  local marker
  marker="$(published_marker)"
  [[ -f $marker ]] && cat "$marker"
}

commits_are_signed() {
  [[ $(git config --type=bool --get commit.gpgsign 2>/dev/null) == true ]]
}

# The commit is built on main's tree in an index of its own, so the checkout and its staged work are left alone.
# `git commit-tree` signs only when told to, whatever `commit.gpgsign` says.
commit_with_only_the_lock() {
  local blob=$1 index tree
  local sign=--no-gpg-sign
  if commits_are_signed; then
    sign=--gpg-sign
  fi
  index="$(git rev-parse --git-dir)/nvim-lock-publish-index"
  GIT_INDEX_FILE=$index git read-tree origin/main || return 1
  GIT_INDEX_FILE=$index git update-index --add --cacheinfo "100644,$blob,$lock" || return 1
  tree="$(GIT_INDEX_FILE=$index git write-tree)" || return 1
  git commit-tree "$sign" "$tree" -p origin/main -m 'chore(nvim): update plugins'
}

main() {
  local blob published commit refusal
  blob="$(git hash-object -w "$lock" 2>/dev/null)" || return 0
  published="$(last_published_blob || true)"
  if [[ $blob == "$(blob_at origin/main)" ]]; then
    return 0
  fi
  # A lock equal to the last commit pulled is news only when it undoes what this checkout published.
  if [[ $blob == "$(blob_at HEAD)" && $(blob_at origin/main) != "$published" ]]; then
    return 0
  fi
  if ! git fetch -q origin main 2>/dev/null; then
    say 'cannot reach the remote; the next apply retries.'
    return 0
  fi
  if [[ $blob == "$(blob_at origin/main)" ]]; then
    return 0
  fi
  if ! "$scripts_folder/nvim-lock-is-complete.sh" "$lock"; then
    say 'error[incomplete-lock]: not published.' >&2
    return 0
  fi
  # Starting Neovim for that check installs a plugin that was missing, which rewrites the lock.
  blob="$(git hash-object -w "$lock" 2>/dev/null)" || return 0
  if [[ $blob == "$(blob_at origin/main)" ]]; then
    return 0
  fi
  if [[ $(blob_at origin/main) != "$(blob_at HEAD)" && $(blob_at origin/main) != "$published" ]]; then
    say 'main holds versions this checkout has not pulled; pull, then apply again.'
    return 0
  fi
  if ! commit="$(commit_with_only_the_lock "$blob")"; then
    say 'error[commit-failed]: could not make or sign the commit; the next apply retries.' >&2
    return 0
  fi
  if ! refusal="$(git push -q origin "$commit:refs/heads/main" 2>&1)"; then
    say "error[push-refused]: the remote did not take the commit; the next apply retries. git said: $(tr '\n' ' ' <<<"$refusal")" >&2
    return 0
  fi
  printf '%s\n' "$blob" >"$(published_marker)"
  say "published ${commit:0:7} to main."
}

main
