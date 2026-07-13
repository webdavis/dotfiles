#!/usr/bin/env bash
#
# Behavioral check: the `j` alias (for just) actually produces tab-completions.
#
# Round-1 bound carapace's completer to `j` directly, but carapace keys on
# COMP_WORDS[0] and only has a just spec, so `j <TAB>` returned ZERO completions
# (a silent regression of the old ~/.bash_just_completions, which gave real
# recipes). The structural check "complete -p j is set" PASSED on that broken
# version, so this test DRIVES the completion and asserts a non-empty COMPREPLY
# that MATCHES just -- the only assertion that catches a delegating-wrapper bug.
#
# e2e: needs the real carapace binary + an interactive shell + a justfile in cwd.
# Skips cleanly where carapace or chezmoi is absent (e.g. CI without carapace).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

command -v chezmoi >/dev/null 2>&1 || {
  printf 'SKIP: chezmoi not on PATH; cannot render dot_bashrc.tmpl\n'
  exit 0
}
command -v carapace >/dev/null 2>&1 || {
  printf 'SKIP: carapace not installed; cannot drive j completion\n'
  exit 0
}
command -v just >/dev/null 2>&1 || {
  printf 'SKIP: just not installed; no recipes to complete\n'
  exit 0
}

render="$(CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$REPO_ROOT/dot_bashrc.tmpl")" ||
  { printf 'j-completion: FAIL -- render failed\n' >&2; exit 1; }

# Drive both `j` and just completion for the same prefix in one interactive
# shell (carapace only registers under `-i`), from the repo root so just sees a
# justfile. Print the two COMPREPLY counts for comparison.
counts="$(cd "$REPO_ROOT" && bash -i -c "
  source /dev/stdin <<'RC'
$render
RC
  COMP_WORDS=(j t); COMP_CWORD=1; COMP_LINE='j t'; COMP_POINT=3; COMPREPLY=()
  _j_carapace_complete 2>/dev/null || true
  printf 'j=%s\n' \"\${#COMPREPLY[@]}\"
  COMP_WORDS=(just t); COMP_CWORD=1; COMP_LINE='just t'; COMP_POINT=6; COMPREPLY=()
  _carapace_completer 2>/dev/null || true
  printf 'just=%s\n' \"\${#COMPREPLY[@]}\"
" 2>/dev/null)"

j_n="$(sed -n 's/^j=//p' <<<"$counts")"
just_n="$(sed -n 's/^just=//p' <<<"$counts")"

[[ ${just_n:-0} -gt 0 ]] || {
  printf 'j-completion: SKIP -- just t yielded no completions here (carapace/just state); cannot compare\n'
  exit 0
}
[[ ${j_n:-0} -gt 0 ]] ||
  { printf 'j-completion: FAIL -- j t yielded 0 completions (wrapper not delegating to just)\n' >&2; exit 1; }
[[ ${j_n:-0} -eq ${just_n:-0} ]] ||
  { printf 'j-completion: FAIL -- j (%s) != just (%s) completions\n' "$j_n" "$just_n" >&2; exit 1; }

printf 'j-completion: OK -- j completes identically to just (%s candidates for prefix "t")\n' "$j_n"
