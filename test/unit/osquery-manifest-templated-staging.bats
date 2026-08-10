#!/usr/bin/env bats
# The known-good manifest's COVERAGE of the converge staging tree, on both sides
# of the trust boundary: which files run_after_05 signs, and which files the
# alerter treats as pipeline infrastructure.
#
# THE PROBLEM THIS PINS. run_after_05 derives every hash from chezmoi's INTENT
# (`chezmoi cat`), never from the deployed tree, which is what stops a tampered
# file being signed with its own tampered bytes. The mandated agent apply is
# `chezmoi apply --exclude=templates`, which does not write template-sourced
# targets at all. For the two TEMPLATED staging files those two facts collide:
# the manifest records the newly rendered intent while the deployed copies still
# hold the previous render, and the periodic audit re-reads every manifested path
# every 15 minutes, so the pair pages a CRIT until someone runs a full apply.
#
# Excluding them from the manifest is only half an answer, because anything under
# the pipeline home that the manifest does not list pages FOREVER through the
# event path instead. Both halves are therefore pinned here together: excluded
# from the manifest AND not treated as pipeline infrastructure. The four STATIC
# staging files keep full intent coverage, which is the guarantee that matters:
# they are the ones a plain apply really does deploy.

setup() {
  REPO="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)"
  RUNNER="$REPO/.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh"
  VERDICT="$REPO/dot_local/libexec/osquery/results-alerter/pipeline-verdict.sh"
  SANDBOX="$BATS_TEST_TMPDIR"
  HOME_DIR="$SANDBOX/home"
  BIN="$SANDBOX/bin"
  mkdir -p "$HOME_DIR" "$BIN"

  STAGING="$HOME_DIR/.local/libexec/osquery/osquery-converge/desired"
  TEMPLATED=(
    "$STAGING/osquery.conf"
    "$STAGING/packs/agent-attack-surface.conf"
  )
  STATIC=(
    "$STAGING/osquery.flags"
    "$STAGING/packs/installed-software-drift.conf"
    "$STAGING/packs/intrusion-detection.conf"
    "$STAGING/packs/security-policy-regression.conf"
  )
}

# The managed listing the chezmoi stub answers with: the converge tool itself,
# the whole staging tree, and one ~/.local/bin file so the bin arm resolves.
managed_listing() {
  printf '%s\n' \
    "$HOME_DIR/.local/libexec/osquery/osquery-converge.sh" \
    "${TEMPLATED[@]}" \
    "${STATIC[@]}" \
    "$HOME_DIR/.local/bin/ssh-hardening.sh"
}

# A chezmoi stub answering the three read-only subcommands the runner uses, and a
# sudo stub that installs without the ownership flags a sandbox cannot honor.
install_stubs() {
  managed_listing >"$SANDBOX/managed.txt"
  cat >"$BIN/chezmoi" <<'STUB'
#!/bin/bash
# Skip the global flags and their values to find the subcommand.
while (($#)); do
  case "$1" in
    --source | --persistent-state) shift 2 ;;
    -*) shift ;;
    *) break ;;
  esac
done
case "${1:-}" in
  managed) cat "$MANAGED_LIST" ;;
  state) ;;
  dump)
    shift
    # "<destination-relative name>": {"perm": <decimal>}, which is the shape the
    # runner reads the intended mode out of.
    printf '{'
    separator=''
    for target in "$@"; do
      [[ $target == /* ]] || continue
      printf '%s"%s":{"perm":420}' "$separator" "${target#"$HOME_DIR"/}"
      separator=','
    done
    printf '}\n'
    ;;
  cat) printf 'rendered intent for %s\n' "$2" ;;
  *) exit 1 ;;
esac
exit 0
STUB
  cat >"$BIN/sudo" <<'STUB'
#!/bin/bash
[[ ${1:-} == -n ]] && shift
if [[ ${1:-} == install || ${1:-} == */install ]]; then
  shift
  args=()
  while (($#)); do
    case "$1" in
      -o | -g) shift 2 ;;
      *)
        args+=("$1")
        shift
        ;;
    esac
  done
  exec install "${args[@]}"
fi
exec "$@"
STUB
  chmod +x "$BIN/chezmoi" "$BIN/sudo"
}

# Run the manifest generator against the sandbox and print the pipeline manifest.
generate_manifests() {
  install_stubs
  PATH="$BIN:$PATH" \
    MANAGED_LIST="$SANDBOX/managed.txt" \
    HOME_DIR="$HOME_DIR" \
    CHEZMOI_HOME_DIR="$HOME_DIR" \
    OSQUERY_PIPELINE_MANIFEST="$SANDBOX/pipeline-known-good.sha256" \
    OSQUERY_MANAGED_BIN_MANIFEST="$SANDBOX/managed-bin-known-good.sha256" \
    bash "$RUNNER"
}

manifest_lists() { # <path>
  grep -qF -- " $1" "$SANDBOX/pipeline-known-good.sha256"
}

# The negative assertions go through refute helpers rather than `! predicate`,
# which set -e ignores inside bats and which would therefore pass whatever the
# predicate answered.
refute_manifest_lists() { # <path>
  if manifest_lists "$1"; then
    printf 'expected %s NOT to be in the pipeline manifest, but it is:\n%s\n' \
      "$1" "$(cat "$SANDBOX/pipeline-known-good.sha256")" >&2
    return 1
  fi
  return 0
}

refute_tracked() { # <path>
  if _pipeline_is_tracked "$1"; then
    printf 'expected %s NOT to be treated as pipeline infrastructure, but it is\n' "$1" >&2
    return 1
  fi
  return 0
}

@test "the two TEMPLATED staging files are left out of the pipeline manifest" {
  # Signed from intent they would page a CRIT every 15 minutes, because the
  # apply that is mandated here never writes them.
  run generate_manifests
  [ "$status" -eq 0 ]
  local path
  for path in "${TEMPLATED[@]}"; do
    refute_manifest_lists "$path"
  done
}

@test "the four STATIC staging files are still manifested from intent" {
  # These are what a plain apply really does deploy, so intent and deployment
  # agree and the integrity guarantee over them is honest. They are also the
  # files this tool installs root-owned into the root daemon's directory, so
  # losing coverage of them is not an acceptable price for fixing the other two.
  run generate_manifests
  [ "$status" -eq 0 ]
  local path
  for path in "${STATIC[@]}"; do
    manifest_lists "$path"
  done
}

@test "the two TEMPLATED staging files are not treated as pipeline infrastructure either" {
  # The other half. A file under the pipeline home that the manifest does not
  # list pages forever through the event path, so dropping them from the
  # manifest alone would trade a stale-manifest CRIT for a permanent one.
  HOME="$HOME_DIR"
  # shellcheck source=dot_local/libexec/osquery/results-alerter/pipeline-verdict.sh
  source "$VERDICT"
  local path
  for path in "${TEMPLATED[@]}"; do
    refute_tracked "$path"
  done
}

@test "the four STATIC staging files ARE treated as pipeline infrastructure" {
  HOME="$HOME_DIR"
  # shellcheck source=dot_local/libexec/osquery/results-alerter/pipeline-verdict.sh
  source "$VERDICT"
  local path
  for path in "${STATIC[@]}"; do
    _pipeline_is_tracked "$path"
  done
}
