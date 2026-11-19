#!/usr/bin/env bash

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly REPO_ROOT
readonly apply_logged="$REPO_ROOT/scripts/chezmoi-apply-logged.sh"

scratch_folder="$(mktemp -d)"
readonly scratch_folder
failures=0

expect() {
  local label=$1 got=$2 wanted=$3
  if [[ $got == "$wanted" ]]; then
    printf 'ok    %s\n' "$label"
    return
  fi
  printf 'FAIL  %s\n      got:    %s\n      wanted: %s\n' "$label" "$got" "$wanted"
  failures=$((failures + 1))
}

install_chezmoi_stand_in_that_prints() {
  local case_folder=$1 apply_output=$2
  mkdir "$case_folder/bin"
  printf '%s\n' "$apply_output" >"$case_folder/output"
  printf '#!/bin/sh\ncat "%s"\n' "$case_folder/output" >"$case_folder/bin/chezmoi"
  chmod +x "$case_folder/bin/chezmoi"
}

run_logged_apply_with_stand_in() {
  local case_folder=$1
  CHEZMOI_APPLY_LOG_DIR="$case_folder/logs" PATH="$case_folder/bin:$PATH" "$apply_logged" >/dev/null 2>&1 </dev/null
}

lines_between_header_and_footer() {
  local log=$1
  sed -n '/^apply output:$/,/^finished: /p' "$log" | sed '1d;$d' | sed '$d'
}

log_of_an_apply_that_prints() {
  local apply_output=$1 case_folder
  case_folder="$(mktemp -d "$scratch_folder/case.XXXXXX")"
  install_chezmoi_stand_in_that_prints "$case_folder" "$apply_output"
  run_logged_apply_with_stand_in "$case_folder"
  lines_between_header_and_footer "$case_folder/logs/latest.apply.log"
}

readonly indented_installer_errors=$'verify FAILED, 2 problem(s):\n  PermitRootLogin is yes\n  - PasswordAuthentication is yes\n+ a line that starts with a plus'
readonly diff_of_a_secret_file=$'diff --git a/.env b/.env\nindex 111..222 100600\n--- a/.env\n+++ b/.env\n@@ -1,3 +1,3 @@ NEIGHBOUR_KEY=leaked\n TOKEN_A=keep\n-TOKEN_B=old\n+TOKEN_B=new\n TOKEN_C=keep'
readonly diff_removing_a_line_that_looks_like_a_file_header=$'--- a/notes\n+++ b/notes\n@@ -1,2 +1 @@\n--- a secret rule\n kept'
readonly one_line_diff_whose_header_gives_no_counts=$'--- a/.env\n+++ b/.env\n@@ -1 +1 @@\n-TOKEN=old\n+TOKEN=new\ndone'

expect 'indented output outside a diff is kept' \
  "$(log_of_an_apply_that_prints "$indented_installer_errors")" \
  "$indented_installer_errors"

expect 'the lines of a diff hunk are withheld and its path is kept' \
  "$(log_of_an_apply_that_prints "$diff_of_a_secret_file")" \
  $'diff --git a/.env b/.env\nindex 111..222 100600\n--- a/.env\n+++ b/.env\n@@ -1,3 +1,3 @@\n    [4 content line(s) withheld]'

expect 'no file content follows a hunk header' \
  "$(log_of_an_apply_that_prints "$diff_of_a_secret_file" | grep -c 'leaked\|TOKEN')" \
  '0'

expect 'indented output right after a hunk is kept' \
  "$(log_of_an_apply_that_prints "$diff_of_a_secret_file"$'\n  cargo failed with exit code 101' | tail -1)" \
  '  cargo failed with exit code 101'

expect 'a removed line that looks like a file header is withheld' \
  "$(log_of_an_apply_that_prints "$diff_removing_a_line_that_looks_like_a_file_header" | tail -1)" \
  '    [2 content line(s) withheld]'

expect 'a one-line change, whose header leaves out the counts, is withheld' \
  "$(log_of_an_apply_that_prints "$one_line_diff_whose_header_gives_no_counts" | tail -3)" \
  $'@@ -1 +1 @@\n    [2 content line(s) withheld]\ndone'

rm -rf "$scratch_folder"
((failures == 0))
