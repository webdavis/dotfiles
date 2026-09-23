# shellcheck shell=bash
FZF_TEST_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)

function test_fzf_inserts_quoted_arguments_without_clobbering_pending_command() {
  local output
  output=$(FZF_PICKERS_TEST=1 bash --noprofile --norc -c '
    source "$1/dot_fzf_bindings" 2>/dev/null
    READLINE_LINE="git show  --stat" READLINE_POINT=9
    __fzf_insert_arguments "a b" "x;y" 2>/dev/null || true
    printf "%s" "$READLINE_LINE"
  ' _ "$FZF_TEST_ROOT")
  assert_same 'git show a\ b x\;y --stat' "$output"
}

function test_fzf_cancel_preserves_line_and_cursor() {
  local output
  output=$(FZF_PICKERS_TEST=1 bash --noprofile --norc -c '
    source "$1/dot_fzf_bindings" 2>/dev/null
    FZF_PICKERS_HELPER="$1/dot_local/libexec/fzf-pickers/executable_picker.py"
    __fzf_run_picker() { return 130; }
    READLINE_LINE="echo pending" READLINE_POINT=4
    __fzf_pick commits 2>/dev/null || true
    printf "%s|%s" "$READLINE_LINE" "$READLINE_POINT"
  ' _ "$FZF_TEST_ROOT")
  assert_same 'echo pending|4' "$output"
}

function test_fzf_failed_partial_result_preserves_command() {
  local output
  output=$(FZF_PICKERS_TEST=1 bash --noprofile --norc -c '
    source "$1/dot_fzf_bindings"
    FZF_PICKERS_HELPER="$1/dot_local/libexec/fzf-pickers/executable_picker.py"
    __fzf_run_picker() { printf "command\0dangerous\0"; return 1; }
    READLINE_LINE="echo pending" READLINE_POINT=4
    __fzf_pick commits
    printf "%s|%s" "$READLINE_LINE" "$READLINE_POINT"
  ' _ "$FZF_TEST_ROOT")
  assert_same 'echo pending|4' "$output"
}

function test_binding_menu_prepares_without_executing() {
  local output
  output=$(bash --noprofile --norc -c '
    source "$1/dot_bash_bindings_functions" 2>/dev/null
    fzf() { cat >/dev/null; printf "label\trun\tprintf bad\tdescription\tctrl-x z\n"; }
    READLINE_LINE="pending" READLINE_POINT=3
    __bash_bindings_list_bash_bindings
    printf "%s|%s" "$READLINE_LINE" "$READLINE_POINT"
  ' _ "$FZF_TEST_ROOT")
  assert_same 'printf bad|10' "$output"
}

__test_fzf_prefix() {
  local result
  result=$(python3 -B "$FZF_TEST_ROOT/test/helpers/fzf-prefix.py" "$1" "$2")
  assert_same 'OK' "$result"
}

function test_delayed_git_chord_in_vi_insert() { __test_fzf_prefix vi-insert git; }
function test_delayed_git_chord_in_vi_command() { __test_fzf_prefix vi-command git; }
function test_delayed_git_chord_in_emacs() { __test_fzf_prefix emacs-standard git; }
function test_delayed_file_chord_in_vi_insert() { __test_fzf_prefix vi-insert file; }
function test_delayed_file_chord_in_vi_command() { __test_fzf_prefix vi-command file; }
function test_delayed_file_chord_in_emacs() { __test_fzf_prefix emacs-standard file; }
