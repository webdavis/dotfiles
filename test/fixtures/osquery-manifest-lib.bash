#!/usr/bin/env bash
#
# osquery-manifest-lib.bash - fixture harness for the known-good manifest runner
# (.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh).
#
# The runner derives the manifest from chezmoi's managed intent, so a fixture needs
# a REAL (tiny) chezmoi source tree plus its own config, isolated from the
# operator's: HOME is redirected and XDG_CONFIG_HOME is unset, so nested chezmoi
# calls resolve the fixture config and never the live one. `sudo` is PATH-shadowed
# by a stub that records argv and emulates the copy, so no test ever needs real
# privilege and nothing is written under /var.

# manifest_fixture_setup: build the scratch source, dest HOME, config, and stubs.
# Exports MF_ROOT, MF_SRC, MF_HOME, MF_MANIFEST, MF_BIN_MANIFEST, MF_SUDO_LOG.
#
# TWO manifest destinations, because the runner writes two: the osquery pipeline's
# own known-good list and the separate one for the chezmoi-managed scripts under
# ~/.local/bin. Both are redirected into the scratch dir, so no test ever needs
# privilege and nothing is written under /var.
manifest_fixture_setup() {
  MF_ROOT="$(mktemp -d)"
  MF_SRC="$MF_ROOT/src"
  MF_HOME="$MF_ROOT/home"
  MF_MANIFEST="$MF_ROOT/pipeline-known-good.sha256"
  MF_BIN_MANIFEST="$MF_ROOT/managed-bin-known-good.sha256"
  MF_SUDO_LOG="$MF_ROOT/sudo.log"
  mkdir -p "$MF_SRC/dot_local/libexec/osquery/results-alerter" \
    "$MF_SRC/dot_local/bin" \
    "$MF_SRC/Library/LaunchAgents" \
    "$MF_SRC/dot_config/osquery" \
    "$MF_HOME/.config/chezmoi" \
    "$MF_ROOT/bin"
  printf 'sourceDir = "%s"\ndestDir = "%s"\n' "$MF_SRC" "$MF_HOME" \
    >"$MF_HOME/.config/chezmoi/chezmoi.toml"
  # Stub sudo: record argv, then emulate `install ... <src> <dest>` by copying.
  cat >"$MF_ROOT/bin/sudo" <<'STUB'
#!/usr/bin/env bash
# Record argv, then emulate the two `install` shapes the runner uses without any
# privilege: `install -d ... <dir>` creates the directory, and
# `install ... <src> <dest>` copies the file.
printf '%s\n' "$*" >>"$SUDO_LOG"
dest="${!#}"
for arg in "$@"; do
  if [[ $arg == -d ]]; then
    mkdir -p "$dest"
    exit $?
  fi
done
src_pos=$(($# - 1))
src="${!src_pos}"
cp "$src" "$dest"
STUB
  chmod +x "$MF_ROOT/bin/sudo"
  : >"$MF_SUDO_LOG"
}

manifest_fixture_teardown() {
  [[ -n ${MF_ROOT:-} ]] && rm -rf "$MF_ROOT"
}

# manifest_fixture_add_script <relative-name> <content>: add a MANAGED pipeline
# script (chezmoi's executable_ prefix is dropped in the target).
manifest_fixture_add_script() {
  local rel="$1" content="$2" dir
  dir="$(dirname "$MF_SRC/dot_local/libexec/osquery/$rel")"
  mkdir -p "$dir"
  printf '%s\n' "$content" >"$dir/executable_$(basename "$rel")"
}

# manifest_fixture_add_bin_script <name> <content>: add a MANAGED ~/.local/bin tool
# (chezmoi's executable_ prefix is dropped in the target).
manifest_fixture_add_bin_script() {
  printf '%s\n' "$2" >"$MF_SRC/dot_local/bin/executable_$1"
}

# manifest_fixture_add_plist <label> <content>: add a MANAGED LaunchAgent template
# (the real plists are templates, so the fixture mirrors that shape).
manifest_fixture_add_plist() {
  printf '%s\n' "$2" >"$MF_SRC/Library/LaunchAgents/$1.plist.tmpl"
}

# manifest_fixture_add_config <source-basename> <content>: add a MANAGED file under
# ~/.config/osquery. The basename is passed with its chezmoi attribute prefix
# intact (private_ for the 0600 files), because the mode those prefixes encode is
# exactly what the manifest's mode column has to come from.
manifest_fixture_add_config() {
  printf '%s\n' "$2" >"$MF_SRC/dot_config/osquery/$1"
}

# manifest_fixture_chezmoi <args...>: run chezmoi against the fixture, isolated.
manifest_fixture_chezmoi() {
  env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$MF_HOME" CI=1 \
    chezmoi --config "$MF_HOME/.config/chezmoi/chezmoi.toml" \
    --source "$MF_SRC" --destination "$MF_HOME" "$@"
}

# manifest_fixture_apply: deploy the fixture source, mirroring the mandated agent
# apply path (templates excluded).
manifest_fixture_apply() {
  manifest_fixture_apply_mode --exclude=templates
}

manifest_fixture_apply_mode() {
  manifest_fixture_chezmoi apply "$@" --force >/dev/null 2>&1
}

# manifest_fixture_run_runner: run the manifest runner exactly as chezmoi would
# (CHEZMOI_SOURCE_DIR / CHEZMOI_HOME_DIR set, stub sudo first on PATH, the manifest
# destination redirected into the scratch dir). Clears the sudo log first, so a
# caller can assert on this run alone. Returns the runner's exit status.
manifest_fixture_run_runner() {
  local runner="$1"
  : >"$MF_SUDO_LOG"
  env -u XDG_CONFIG_HOME -u XDG_DATA_HOME \
    HOME="$MF_HOME" \
    PATH="$MF_ROOT/bin:$PATH" \
    CHEZMOI_SOURCE_DIR="$MF_SRC" \
    CHEZMOI_HOME_DIR="$MF_HOME" \
    OSQUERY_PIPELINE_MANIFEST="$MF_MANIFEST" \
    OSQUERY_MANAGED_BIN_MANIFEST="$MF_BIN_MANIFEST" \
    SUDO_LOG="$MF_SUDO_LOG" \
    bash "$runner"
}

# manifest_fixture_installed: did this run perform a privileged install?
manifest_fixture_installed() { [[ -s $MF_SUDO_LOG ]]; }

# The manifest is "<sha256> <mode> <uid> <path>", path LAST. awk splits on runs of
# whitespace, so field 4 is the path only while the path itself has no spaces; the
# fixture never creates one, and the runner's own reader takes the remainder of the
# line rather than a fixed field.

# manifest_hash_of <target-path>: the manifest's recorded hash for a path, or empty.
manifest_hash_of() {
  awk -v p="$1" '$4 == p {print $1}' "$MF_MANIFEST" 2>/dev/null
}

# manifest_mode_of <target-path>: the manifest's recorded mode (four octal digits).
manifest_mode_of() {
  awk -v p="$1" '$4 == p {print $2}' "$MF_MANIFEST" 2>/dev/null
}

# manifest_uid_of <target-path>: the manifest's recorded owner uid (decimal).
manifest_uid_of() {
  awk -v p="$1" '$4 == p {print $3}' "$MF_MANIFEST" 2>/dev/null
}

# bin_manifest_hash_of <target-path>: the MANAGED-BIN manifest's recorded hash for a
# path, or empty. A separate reader rather than a parameter on manifest_hash_of, so a
# call site names which manifest it means and a copy-paste cannot silently assert
# against the wrong one.
bin_manifest_hash_of() {
  awk -v p="$1" '$4 == p {print $1}' "$MF_BIN_MANIFEST" 2>/dev/null
}

# bin_manifest_mode_of / bin_manifest_uid_of: the other two bound columns, for the
# managed-bin manifest. Same four-column shape as the pipeline manifest.
bin_manifest_mode_of() {
  awk -v p="$1" '$4 == p {print $2}' "$MF_BIN_MANIFEST" 2>/dev/null
}
bin_manifest_uid_of() {
  awk -v p="$1" '$4 == p {print $3}' "$MF_BIN_MANIFEST" 2>/dev/null
}

# verdict_says_page <target> [verdict-helper]: run the REAL pipeline_verdict over
# the fixture manifest with the file's CURRENT on-disk hash, and return 0 when the
# verdict is PAGE. Driven in a `bash -c` child rather than a (..) subshell so the
# environment the verdict reads is set for the process that actually runs it.
# MF_VERDICT may be set by the caller; otherwise pass the helper as $2.
verdict_says_page() {
  local target="$1" helper="${2:-${MF_VERDICT:-}}" target_hash
  target_hash="$(shasum -a 256 "$target" | awk '{print $1}')"
  HOME="$MF_HOME" \
    OSQUERY_PIPELINE_MANIFEST="$MF_MANIFEST" \
    OSQUERY_PIPELINE_REHASH_DELAY=0 \
    OSQUERY_PIPELINE_SETTLE_SECONDS=0 \
    bash -c 'source "$1"; pipeline_verdict "$2" "$3" UPDATED' _ "$helper" "$target" "$target_hash"
}
