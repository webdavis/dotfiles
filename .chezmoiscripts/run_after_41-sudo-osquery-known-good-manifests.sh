#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=.chezmoitemplates/cli-print-style-lib.sh.tmpl
source "${CHEZMOI_SOURCE_DIR:?}/.chezmoitemplates/cli-print-style-lib.sh.tmpl"

# shellcheck source=.chezmoitemplates/sudo-session.sh.tmpl
source "${CHEZMOI_SOURCE_DIR:?}/.chezmoitemplates/sudo-session.sh.tmpl"

readonly exit_bad_arguments=2
readonly pipeline_manifest="${OSQUERY_PIPELINE_MANIFEST:-/var/osquery/pipeline-known-good.sha256}"
readonly managed_bin_manifest="${OSQUERY_MANAGED_BIN_MANIFEST:-/var/osquery/managed-bin-known-good.sha256}"
readonly home="${CHEZMOI_HOME_DIR:-$HOME}"
readonly chezmoi_config_template="$CHEZMOI_SOURCE_DIR/.chezmoi.toml.tmpl"
readonly highest_valid_permission=4095
readonly rust_tools_install_directory="$home/.cargo/bin"
declare -rA rust_tools_max_artifact_bytes=(
  [pns]=14680064
  [posture]=8388608
)
readonly -a built_binaries_in_path_order=(pns posture)
readonly built_binary_mode=0755
readonly build_record_directory="$home/.local/state"
readonly unbuilt_binary_digest=unbuilt

refresh_scope=all
installed_manifest_count=0
owner_user_id=''
pipeline_paths=()
managed_bin_paths=()
declare -A intended_permissions=()

running_on_macos() {
  [[ "$(uname)" == Darwin ]]
}

arguments_ask_for_every_manifest() {
  (($# == 0))
}

arguments_ask_for_the_pipeline_manifest_only() {
  (($# == 1)) && [[ $1 == --pipeline-only ]]
}

arguments_are_supported() {
  arguments_ask_for_every_manifest "$@" || arguments_ask_for_the_pipeline_manifest_only "$@"
}

managed_bin_manifest_is_in_scope() {
  [[ $refresh_scope == all ]]
}

create_scratch_files() {
  managed_files="$(mktemp)"
  sorted_managed_files="$(mktemp)"
  managed_files_dump="$(mktemp)"
  permission_pairs="$(mktemp)"
  generated_manifest="$(mktemp)"
  throwaway_state_directory="$(mktemp -d)"
  trap remove_scratch_files EXIT
}

remove_scratch_files() {
  rm -f "$managed_files" "$sorted_managed_files" "$managed_files_dump" "$permission_pairs" "$generated_manifest"
  rm -rf "$throwaway_state_directory"
}

chezmoi_with_apply_source() {
  chezmoi --source "$CHEZMOI_SOURCE_DIR" "$@"
}

chezmoi_with_throwaway_state() {
  chezmoi_with_apply_source --persistent-state "$throwaway_state_directory/state.boltdb" "$@"
}

list_managed_files() {
  chezmoi_with_apply_source managed --path-style=absolute --include=files
}

render_managed_file() {
  local target=$1
  chezmoi_with_apply_source cat "$target"
}

dump_managed_files_as_json() {
  chezmoi_with_throwaway_state dump --format=json "$@"
}

store_config_template_hash_in_throwaway_state() {
  local config_template_hash=$1
  chezmoi_with_throwaway_state state set --bucket=configState --key=configState \
    --value="{\"configTemplateContentsSHA256\":\"$config_template_hash\"}"
}

sort_in_byte_order() {
  local file=$1
  LC_ALL=C sort "$file"
}

config_template_hash() {
  shasum -a 256 "$chezmoi_config_template" | awk '{print $1}'
}

intended_content_hash() {
  local target=$1
  render_managed_file "$target" | shasum -a 256 | awk '{print $1}'
}

permission_and_path_pairs_in_dump() {
  local dump=$1
  jq -r 'to_entries[] | "\(.value.perm) \(.key)"' "$dump"
}

save_managed_file_listing() {
  if ! list_managed_files >"$managed_files"; then
    report_line "error[list-failed]: could not list managed files, refusing to rewrite any manifest." >&2
    return 1
  fi
}

sort_managed_file_listing() {
  if ! sort_in_byte_order "$managed_files" >"$sorted_managed_files"; then
    report_line "error[sort-failed]: could not sort the managed listing, refusing to rewrite any manifest." >&2
    return 1
  fi
}

path_belongs_to_the_pipeline_manifest() {
  local path=$1
  case "$path" in
    "$home"/.local/libexec/osquery/* | "$home"/.local/libexec/posture/*) return 0 ;;
    "$home"/Library/LaunchAgents/com.webdavis.*.plist) return 0 ;;
    "$home"/.config/osquery/page-launchd-allowlist.txt) return 0 ;;
    *) return 1 ;;
  esac
}

path_belongs_to_the_managed_bin_manifest() {
  local path=$1
  case "$path" in
    "$home"/.local/bin/*/*) return 1 ;;
    "$home"/.local/bin/* | "$home"/.local/libexec/*) return 0 ;;
    *) return 1 ;;
  esac
}

assign_managed_files_to_manifests() {
  local path
  while IFS= read -r path; do
    if path_belongs_to_the_pipeline_manifest "$path"; then
      pipeline_paths+=("$path")
    elif path_belongs_to_the_managed_bin_manifest "$path"; then
      managed_bin_paths+=("$path")
    fi
  done <"$sorted_managed_files"
}

no_pipeline_files_were_found() {
  ((${#pipeline_paths[@]} == 0))
}

no_managed_bin_files_were_found() {
  ((${#managed_bin_paths[@]} == 0))
}

require_managed_files_before_the_dump() {
  if no_pipeline_files_were_found; then
    report_line "error[no-pipeline-files]: no managed pipeline files resolved, refusing to rewrite any manifest." >&2
    return 1
  fi
  if managed_bin_manifest_is_in_scope && no_managed_bin_files_were_found; then
    report_line "error[no-bin-files]: no managed ~/.local/bin files resolved, refusing to rewrite any manifest." >&2
    return 1
  fi
}

config_template_is_readable() {
  [[ -r $chezmoi_config_template ]]
}

keep_the_dump_from_warning_about_the_config_template() {
  if ! config_template_is_readable; then
    return
  fi
  local config_template_sha256
  config_template_sha256="$(config_template_hash)"
  store_config_template_hash_in_throwaway_state "$config_template_sha256" 2>/dev/null || true
}

dump_managed_files_in_scope() {
  local paths_in_scope=("${pipeline_paths[@]}")
  if managed_bin_manifest_is_in_scope; then
    paths_in_scope+=("${managed_bin_paths[@]}")
  fi
  if ! dump_managed_files_as_json "${paths_in_scope[@]}" >"$managed_files_dump"; then
    report_line "error[dump-failed]: could not dump the managed files, refusing to rewrite any manifest." >&2
    return 1
  fi
}

extract_intended_permissions() {
  if ! permission_and_path_pairs_in_dump "$managed_files_dump" >"$permission_pairs"; then
    report_line "error[mode-read-failed]: could not read the intended modes out of the dump, refusing to rewrite any manifest." >&2
    return 1
  fi
}

permission_pair_is_complete() {
  local permission=$1 relative_path=$2
  [[ -n $permission && -n $relative_path ]]
}

load_intended_permissions() {
  local permission relative_path
  while read -r permission relative_path; do
    if permission_pair_is_complete "$permission" "$relative_path"; then
      intended_permissions["$home/$relative_path"]="$permission"
    fi
  done <"$permission_pairs"
}

no_intended_permissions_were_found() {
  ((${#intended_permissions[@]} == 0))
}

require_intended_permissions() {
  if no_intended_permissions_were_found; then
    report_line "error[no-modes]: the dump yielded no modes, refusing to rewrite any manifest." >&2
    return 1
  fi
}

read_owner_user_id() {
  owner_user_id="$(id -u)"
}

user_id_is_numeric() {
  local user_id=$1
  [[ $user_id =~ ^[0-9]{1,10}$ ]]
}

require_a_numeric_owner_user_id() {
  if ! user_id_is_numeric "$owner_user_id"; then
    report_line "error[bad-user-id]: id -u did not report a numeric uid, refusing to rewrite any manifest." >&2
    return 1
  fi
}

hash_is_a_sha256_digest() {
  local hash=$1
  [[ $hash =~ ^[0-9a-f]{64}$ ]]
}

permission_is_a_valid_mode() {
  local permission=$1
  [[ $permission =~ ^[0-9]{1,4}$ ]] && ((10#$permission <= highest_valid_permission))
}

append_manifest_tuple() {
  local hash=$1 mode=$2 path=$3
  printf '%s %s %s %s\n' "$hash" "$mode" "$owner_user_id" "$path" >>"$generated_manifest"
}

write_managed_file_tuple() {
  local label=$1 target=$2
  local content_hash permission mode
  if ! content_hash="$(intended_content_hash "$target")"; then
    report_line "error[hash-failed]: could not hash $target, refusing to rewrite the $label manifest." >&2
    return 1
  fi
  if ! hash_is_a_sha256_digest "$content_hash"; then
    report_line "error[implausible-hash]: implausible hash for $target, refusing to rewrite the $label manifest." >&2
    return 1
  fi
  permission="${intended_permissions["$target"]:-}"
  if ! permission_is_a_valid_mode "$permission"; then
    report_line "error[missing-mode]: no usable intended mode for $target, refusing to rewrite the $label manifest." >&2
    return 1
  fi
  printf -v mode '%04o' "$((10#$permission))"
  append_manifest_tuple "$content_hash" "$mode" "$target"
}

write_managed_file_tuples() {
  local label=$1
  shift
  local target
  for target in "$@"; do
    write_managed_file_tuple "$label" "$target"
  done
}

artifact_ceiling_is_declared() {
  local tool=$1
  [[ ${rust_tools_max_artifact_bytes[$tool]:-} =~ ^[1-9][0-9]{0,18}$ ]]
}

build_record_is_absent() {
  local record=$1
  [[ ! -e $record && ! -L $record ]]
}

build_record_is_a_regular_file() {
  local record=$1
  [[ -f $record && ! -L $record ]]
}

digest_line_is_well_formed() {
  local line=$1
  [[ $line =~ ^sha256\ [0-9a-f]{64}$ ]]
}

bytes_line_is_well_formed() {
  local line=$1
  [[ $line =~ ^bytes\ [1-9][0-9]{0,9}$ ]]
}

compiler_line_names_rustc() {
  local line=$1
  [[ $line == 'rustc '?* ]]
}

artifact_fits_under_its_ceiling() {
  local tool=$1 artifact_bytes=$2
  ((artifact_bytes <= ${rust_tools_max_artifact_bytes[$tool]}))
}

authorized_digest_in_build_record() {
  local tool=$1 record=$2
  local digest_line bytes_line compiler_line
  build_record_is_a_regular_file "$record" &&
    { IFS= read -r digest_line && IFS= read -r bytes_line && IFS= read -r compiler_line; } <"$record" &&
    digest_line_is_well_formed "$digest_line" &&
    bytes_line_is_well_formed "$bytes_line" &&
    compiler_line_names_rustc "$compiler_line" &&
    artifact_fits_under_its_ceiling "$tool" "${bytes_line#bytes }" &&
    printf '%s' "${digest_line#sha256 }"
}

built_binary_digest() {
  local tool=$1
  local record="$build_record_directory/$tool-build-record"
  local digest
  if ! artifact_ceiling_is_declared "$tool"; then
    report_line "error[missing-ceiling]: no artifact ceiling declared for $tool, refusing to rewrite the pipeline manifest." >&2
    return 1
  fi
  if build_record_is_absent "$record"; then
    printf '%s' "$unbuilt_binary_digest"
    return
  fi
  if ! digest="$(authorized_digest_in_build_record "$tool" "$record")"; then
    report_line "error[malformed-build-record]: malformed $tool build record, refusing to rewrite the pipeline manifest." >&2
    return 1
  fi
  printf '%s' "$digest"
}

write_built_binary_tuples() {
  local tool digest
  for tool in "${built_binaries_in_path_order[@]}"; do
    digest="$(built_binary_digest "$tool")"
    append_manifest_tuple "$digest" "$built_binary_mode" "$rust_tools_install_directory/$tool"
  done
}

clear_generated_manifest() {
  : >"$generated_manifest"
}

manifest_is_the_pipeline_manifest() {
  local manifest=$1
  [[ $manifest == "$pipeline_manifest" ]]
}

generated_manifest_is_empty() {
  [[ ! -s $generated_manifest ]]
}

generate_manifest() {
  local label=$1 manifest=$2
  shift 2
  clear_generated_manifest
  write_managed_file_tuples "$label" "$@"
  if manifest_is_the_pipeline_manifest "$manifest"; then
    write_built_binary_tuples
  fi
  if generated_manifest_is_empty; then
    report_line "error[empty-manifest]: refusing to install an EMPTY $label manifest (no managed files resolved)." >&2
    return 1
  fi
}

manifest_is_current() {
  local manifest=$1
  cmp -s "$generated_manifest" "$manifest"
}

directory_exists() {
  local directory=$1
  [[ -d $directory ]]
}

create_root_owned_directory() {
  local directory=$1
  sudo install -d -o root -g wheel -m 0755 "$directory"
}

install_root_owned_manifest() {
  local manifest=$1
  sudo install -o root -g wheel -m 0644 "$generated_manifest" "$manifest"
}

install_manifest() {
  local label=$1 manifest=$2
  local manifest_directory
  report_line "installing the $label manifest..."
  manifest_directory="$(dirname "$manifest")"
  authenticate_sudo_once
  if ! directory_exists "$manifest_directory"; then
    create_root_owned_directory "$manifest_directory"
  fi
  install_root_owned_manifest "$manifest"
  installed_manifest_count=$((installed_manifest_count + 1))
  report_line "installed $manifest."
}

refresh_manifest() {
  local label=$1 manifest=$2
  shift 2
  generate_manifest "$label" "$manifest" "$@"
  if manifest_is_current "$manifest"; then
    return
  fi
  install_manifest "$label" "$manifest"
}

no_manifest_was_installed() {
  ((installed_manifest_count == 0))
}

main() {
  if ! running_on_macos; then
    return
  fi
  if ! arguments_are_supported "$@"; then
    report_line "error[bad-arguments]: usage: ${0##*/} [--pipeline-only]" >&2
    exit "$exit_bad_arguments"
  fi
  if arguments_ask_for_the_pipeline_manifest_only "$@"; then
    refresh_scope=pipeline
  fi

  report_section 'osquery' 'known-good manifests'
  create_scratch_files
  save_managed_file_listing
  sort_managed_file_listing
  assign_managed_files_to_manifests
  require_managed_files_before_the_dump
  keep_the_dump_from_warning_about_the_config_template
  dump_managed_files_in_scope
  extract_intended_permissions
  load_intended_permissions
  require_intended_permissions
  read_owner_user_id
  require_a_numeric_owner_user_id
  refresh_manifest 'osquery pipeline' "$pipeline_manifest" "${pipeline_paths[@]}"
  if managed_bin_manifest_is_in_scope; then
    refresh_manifest 'managed bin' "$managed_bin_manifest" "${managed_bin_paths[@]}"
  fi
  if no_manifest_was_installed; then
    report_line "already up to date, skipping."
  fi
}

main "$@"
