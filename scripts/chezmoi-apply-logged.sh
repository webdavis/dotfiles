#!/usr/bin/env bash

set -uo pipefail

log_folder="${CHEZMOI_APPLY_LOG_DIR:-$HOME/.local/state/chezmoi-apply}"
repo_root="$(cd -- "${BASH_SOURCE[0]%/*}/.." && pwd)"
timestamp="$(date -u +"%Y-%m-%dT%H-%M-%SZ")"
log="$log_folder/$timestamp.apply.log"
latest_log_link="$log_folder/latest.apply.log"

readonly diff_block_header_regex='^@@ -[0-9]+(,([0-9]+))? \+[0-9]+(,([0-9]+))? @@'
readonly remove_terminal_escape_code_regex=$'\033\\[[0-9;?]*[a-zA-Z]'
readonly remove_carriage_return_regex=$'\r'

chezmoi_apply_output_temp_file=""
apply_status=0
old_lines_left_in_diff_block=0
new_lines_left_in_diff_block=0
redacted_line_count=0

create_owner_only_log_folder() {
  mkdir -p "$log_folder"
  chmod 700 "$log_folder"
}

create_private_log() {
  (umask 077 && : >"$log")
}

create_chezmoi_apply_output_temp_file() {
  chezmoi_apply_output_temp_file="$(mktemp "$log_folder/.$timestamp.raw.XXXXXX")"
}

guarantee_temp_file_is_deleted_on_exit() {
  trap 'rm -f "$chezmoi_apply_output_temp_file"' EXIT INT TERM
}

write_to_log() {
  printf '%s\n' "$1" >>"$log"
}

get_current_time() {
  date -u +"%Y-%m-%dT%H:%M:%SZ"
}

get_git_commit() {
  git -C "$repo_root" rev-parse --short HEAD 2>/dev/null || printf 'unknown'
}

get_git_branch() {
  git -C "$repo_root" rev-parse --abbrev-ref HEAD 2>/dev/null || printf 'unknown'
}

write_log_header() {
  write_to_log "chezmoi apply"
  write_to_log "started:    $(get_current_time)"
  write_to_log "repo:       $repo_root"
  write_to_log "git HEAD:   $(get_git_commit)"
  write_to_log "git branch: $(get_git_branch)"
  write_to_log ""
  write_to_log "apply output:"
}

check_output_goes_to_a_terminal() {
  [[ -t 1 ]]
}

run_apply_in_terminal() {
  script -q "$chezmoi_apply_output_temp_file" chezmoi apply
  apply_status=$?
}

run_apply_through_a_pipe() {
  chezmoi apply 2>&1 | tee "$chezmoi_apply_output_temp_file"
  apply_status=${PIPESTATUS[0]}
}

run_apply_and_capture_its_output() {
  if check_output_goes_to_a_terminal; then
    run_apply_in_terminal
  else
    run_apply_through_a_pipe
  fi
}

check_inside_diff_block() {
  ((old_lines_left_in_diff_block > 0 || new_lines_left_in_diff_block > 0))
}

check_line_starts_diff_block() {
  [[ $1 =~ $diff_block_header_regex ]]
}

check_line_looks_like_file_content() {
  case $1 in
    '' | ' '* | '-'* | '+'* | \\*) return 0 ;;
    *) return 1 ;;
  esac
}

check_line_is_file_content() {
  check_inside_diff_block && check_line_looks_like_file_content "$1"
}

start_diff_block() {
  local header=$1 count_when_omitted=1
  [[ $header =~ $diff_block_header_regex ]]
  old_lines_left_in_diff_block=${BASH_REMATCH[2]:-$count_when_omitted}
  new_lines_left_in_diff_block=${BASH_REMATCH[4]:-$count_when_omitted}
}

end_diff_block() {
  old_lines_left_in_diff_block=0
  new_lines_left_in_diff_block=0
}

count_line_of_diff_block() {
  case $1 in
    '' | ' '*) ((old_lines_left_in_diff_block--, new_lines_left_in_diff_block--)) ;;
    '-'*) ((old_lines_left_in_diff_block--)) ;;
    '+'*) ((new_lines_left_in_diff_block--)) ;;
  esac
}

track_position_in_diff_block() {
  local line=$1
  if check_line_is_file_content "$line"; then
    count_line_of_diff_block "$line"
  elif check_line_starts_diff_block "$line"; then
    start_diff_block "$line"
  else
    end_diff_block
  fi
}

get_line_without_file_content() {
  local line=$1
  if [[ $line =~ $diff_block_header_regex ]]; then
    printf '%s' "${BASH_REMATCH[0]}"
  else
    printf '%s' "$line"
  fi
}

count_redacted_line() {
  ((redacted_line_count++))
}

check_lines_were_redacted() {
  ((redacted_line_count > 0))
}

write_redacted_line_count() {
  if check_lines_were_redacted; then
    write_to_log "    [$redacted_line_count content line(s) withheld]"
    redacted_line_count=0
  fi
}

redact_line() {
  local line=$1
  if check_line_is_file_content "$line"; then
    count_redacted_line
  else
    write_redacted_line_count
    write_to_log "$(get_line_without_file_content "$line")"
  fi
  track_position_in_diff_block "$line"
}

filter_terminal_escape_codes_from_output() {
  sed -E "s/$remove_terminal_escape_code_regex//g; s/$remove_carriage_return_regex//g" "$chezmoi_apply_output_temp_file"
}

verify_last_line_gets_logged() {
  [[ -n $1 ]]
}

write_redacted_log() {
  local line
  while IFS= read -r line || verify_last_line_gets_logged "$line"; do
    redact_line "$line"
  done < <(filter_terminal_escape_codes_from_output)
  write_redacted_line_count
}

check_apply_succeeded() {
  ((apply_status == 0))
}

write_log_footer() {
  write_to_log ""
  write_to_log "finished:   $(get_current_time)"
  write_to_log "exit_code:  $apply_status"
  if check_apply_succeeded; then
    write_to_log "result:     OK"
  else
    write_to_log "result:     FAILED"
  fi
}

point_latest_log_link_at_this_log() {
  ln -sfn "$log" "$latest_log_link"
}

tell_the_user_where_the_log_is() {
  printf '\napply exit %d, log: %s\n' "$apply_status" "$latest_log_link" >&2
}

main() {
  create_owner_only_log_folder
  create_private_log
  create_chezmoi_apply_output_temp_file
  guarantee_temp_file_is_deleted_on_exit

  write_log_header
  run_apply_and_capture_its_output
  write_redacted_log
  write_log_footer

  point_latest_log_link_at_this_log
  tell_the_user_where_the_log_is
  exit "$apply_status"
}

main "$@"
