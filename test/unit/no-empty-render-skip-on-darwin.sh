#!/usr/bin/env bash
# no-empty-render-skip-on-darwin.sh, a REPO-WIDE guard: no test may treat an
# empty template render as a reason to skip itself on darwin.
#
# The class this closes: nineteen tests carried
#
#   if [[ ! -s $rendered ]]; then
#     printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
#     exit 0
#   fi
#
# The condition tests EMPTINESS while the message asserts a NON-DARWIN HOST.
# Those are different claims. Off darwin the render is empty by design and the
# skip is right. On darwin the template must produce output, so empty means the
# template is broken, and the test reports that as a pass. Every one of these
# suites runs on darwin in CI, which is exactly where the masking mattered.
#
# The fix at each site asserts the host rather than inferring it. This guard
# exists so the next test written from an existing one as a template cannot
# reintroduce the inference: a bare skip is invisible in a green run, so nothing
# else would catch it.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SKIP_MESSAGE='SKIP: empty render'

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -d $REPO_ROOT/test ]] || fail "no test directory under $REPO_ROOT"

unguarded=()
while IFS= read -r file; do
  # Every occurrence of the skip must sit under a darwin assertion. Checked per
  # occurrence, not per file: a file can hold several render sites and guarding
  # one says nothing about the others.
  while IFS= read -r line_number; do
    # The guard is emitted directly above the skip, so a short window is enough
    # and a distant unrelated uname elsewhere in the file cannot vouch for it.
    window_start=$((line_number > 8 ? line_number - 8 : 1))
    if ! sed -n "${window_start},${line_number}p" "$file" | grep -q 'uname -s'; then
      unguarded+=("$file:$line_number")
    fi
  done < <(grep -n -F "$SKIP_MESSAGE" "$file" | cut -d: -f1)
  # This file quotes the pattern in its own prose and stores it in a variable,
  # so scanning itself reports two sites that are documentation, not skips.
done < <(grep -rl -F "$SKIP_MESSAGE" "$REPO_ROOT/test" 2>/dev/null | grep -vF "${BASH_SOURCE[0]##*/}" || true)

if [[ ${#unguarded[@]} -gt 0 ]]; then
  printf 'FAIL: %s test site(s) skip on an empty render without asserting the host:\n' "${#unguarded[@]}" >&2
  printf '  %s\n' "${unguarded[@]}" >&2
  printf 'On darwin an empty render is a BROKEN TEMPLATE, not a reason to skip. Assert the host with uname, do not infer it from emptiness.\n' >&2
  exit 1
fi

guarded_count="$(grep -rc -F "$SKIP_MESSAGE" "$REPO_ROOT/test" 2>/dev/null | grep -vF "${BASH_SOURCE[0]##*/}" | awk -F: '{total += $2} END {print total + 0}')"
# The guard must have something to guard. If every empty-render skip is ever
# deleted this check turns into an assertion about nothing, passing forever
# while proving nothing, so it refuses that state loudly instead.
[[ $guarded_count -ge 1 ]] ||
  fail "no empty-render skip sites found at all; this guard now asserts nothing and should be retired deliberately rather than left passing vacuously"

printf 'no-empty-render-skip-on-darwin: OK (%s empty-render skip site(s), every one asserting the host)\n' "$guarded_count"
