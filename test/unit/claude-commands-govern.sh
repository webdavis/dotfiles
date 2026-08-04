#!/usr/bin/env bash
# claude-commands-govern.sh, the custom slash commands under
# private_dot_claude/commands/ are PROMPTS. Nothing executes them at apply time,
# no formatter reads them (treefmt.nix excludes the whole directory from
# mdformat so the YAML frontmatter survives), and the model that runs one has
# only the file itself for context. So a rule that rots out of one of these
# files fails silently, at the moment the operator invokes the command and the
# wrong thing happens to a real repository. Every property asserted here is one
# of those rules.
#
# WHY EACH GROUP EXISTS.
#
#   gh-axi. The operator's standing rule is that every GitHub operation goes
#   through gh-axi and never through the bare `gh` CLI, which stays installed
#   purely as gh-axi's runtime dependency. This is exactly what disqualified the
#   commit-commands plugin these files replace: it hard-coded `gh pr create` and
#   allowlisted `Bash(gh pr create:*)`. So the bare token is refused in the prose
#   AND in the frontmatter allowlist, and the gh-axi form is required positively,
#   because refusing a token no file contains would pass an empty file.
#
#   The five-section pull request template. The operator locked the structure
#   (Context / Summary / How it was verified / Effect of merging / Review guide)
#   and the order. A command that keeps the words but drops a section produces a
#   body nobody notices is short, so the headings are pinned individually and
#   their ORDER is pinned by line number.
#
#   The review step, and its position. The template alone is a suggestion. What
#   makes it hold is a mandatory re-read of the draft against the structure and
#   the anti-AI-pattern checklist BEFORE the body is posted. A prompt is read top
#   to bottom, so a review instruction sitting after the create call is a review
#   that never runs. That is why this file compares line numbers rather than only
#   asserting both strings are present somewhere.
#
#   --body-file. Passing a multi-paragraph body as an inline argument mangles it.
#   The body file is also what the review step reviews.
#
#   Destructive-action confirmation. CLAUDE.md requires per-invocation
#   confirmation for branch deletion, force-push, worktree removal, and rewriting
#   published history, and says a blanket yes does not carry over. Both halves are
#   required here, because "ask first" without "a previous yes does not count" is
#   the version that gets talked past.
#
#   Em-dashes. A hard operator rule, and these files are the prompts that teach
#   the model what to write, so an em-dash here propagates into commit messages
#   and pull request bodies.
#
#   The roster. The file list is asserted as a SET. Without that, a new GitHub
#   command dropped into the directory would be governed by nothing at all, which
#   is the drift this guard exists to prevent; adding one has to be a deliberate
#   edit here.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly REPO_ROOT
readonly COMMANDS_DIR="$REPO_ROOT/private_dot_claude/commands"

# Every command file in the directory, without the .md suffix. This is the
# complete set: a file present here and absent from disk, or on disk and absent
# here, is a failure either way.
readonly -a ALL_COMMANDS=(
  'amend'
  'clean-gone'
  'commit'
  'commit-push-pr'
  'pr'
  'pr-merge'
  'sync-worktrees'
  'uncommit'
)

# The commands that carry YAML frontmatter. pr-merge.md predates the convention
# and is deliberately left as plain markdown, so it is not in this list; it is
# still covered by every content rule below.
readonly -a FRONTMATTER_COMMANDS=(
  'amend'
  'clean-gone'
  'commit'
  'commit-push-pr'
  'pr'
  'sync-worktrees'
  'uncommit'
)

# The commands that talk to GitHub.
readonly -a GITHUB_COMMANDS=(
  'commit-push-pr'
  'pr'
  'pr-merge'
)

# The commands that open a pull request, so they carry the body template, the
# review step, and the create call.
readonly -a PR_CREATING_COMMANDS=(
  'commit-push-pr'
  'pr'
)

# The commands that write a commit message.
readonly -a COMMIT_MESSAGE_COMMANDS=(
  'amend'
  'commit'
  'commit-push-pr'
)

# The commands that can destroy work: branch deletion and worktree removal
# (clean-gone), rewriting published history (amend, uncommit), force-push
# (commit-push-pr).
readonly -a DESTRUCTIVE_COMMANDS=(
  'amend'
  'clean-gone'
  'commit-push-pr'
  'uncommit'
)

# The five template headings, in the one order the operator locked.
readonly -a TEMPLATE_HEADINGS=(
  '## Context'
  '## Summary'
  '## How it was verified'
  '## Effect of merging'
  '## Review guide'
)

# The review step's heading, and the create call it has to precede.
readonly REVIEW_STEP_HEADING='## Review the draft before posting'
readonly PR_CREATE_CALL='gh-axi pr create'

# Anti-AI-pattern items the review step checks the draft against. Named
# individually so that gutting the checklist down to its heading fails here.
readonly -a REVIEW_CHECKLIST_ITEMS=(
  'em-dashes'
  'not just X but Y'
  'inflated significance'
  'vague attribution'
  'rule-of-three'
  'third person'
)

# A bare `gh` invocation. The leading class keeps `gh-axi pr create` and any
# other hyphenated or word-internal `gh` out of it; only `gh` standing alone and
# followed by a GitHub subcommand matches.
readonly BARE_GH_CALL_PATTERN='(^|[^-[:alnum:]_])gh[[:space:]]+(pr|issue|release)([[:space:]]|$)'

# A Bash allowlist entry for the bare CLI. `Bash(npx -y gh-axi:*)` does not match:
# the character after `gh` is a hyphen.
readonly BARE_GH_ALLOWLIST_PATTERN='Bash\(gh([^-]|$)'

# The inline body flag. `--body-file` does not match, its next character is a
# hyphen.
readonly INLINE_BODY_FLAG_PATTERN='--body([^-]|$)'

# The merge subject convention, matching the placeholder form the command
# documents and a filled-in one alike.
readonly MERGE_SUBJECT_PATTERN='Merge pull request #[^ ]+ from webdavis/[^ ]+ \(#[^)]+\)'

# Built rather than written literally: a test that forbids a character may not
# contain it either.
EM_DASH="$(printf '\xe2\x80\x94')"
readonly EM_DASH

failures=0
assertions=0

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  failures=$((failures + 1))
}

command_path() { # <command>
  printf '%s/%s.md' "$COMMANDS_DIR" "$1"
}

# require_pattern <command> <extended-regex> <what-it-means>
require_pattern() {
  assertions=$((assertions + 1))
  grep -Eq -- "$2" "$(command_path "$1")" ||
    fail "$1.md: $3 (nothing matches /$2/)"
}

# require_literal <command> <fixed-string> <what-it-means>
require_literal() {
  assertions=$((assertions + 1))
  grep -Fq -- "$2" "$(command_path "$1")" ||
    fail "$1.md: $3 (the text '$2' is absent)"
}

# refuse_pattern <command> <extended-regex> <what-it-means>
refuse_pattern() {
  assertions=$((assertions + 1))
  local hits
  hits="$(grep -En -- "$2" "$(command_path "$1")" || true)"
  [[ -z $hits ]] || fail "$1.md: $3; found: $hits"
}

# refuse_literal <command> <fixed-string> <what-it-means>
refuse_literal() {
  assertions=$((assertions + 1))
  local hits
  hits="$(grep -Fn -- "$2" "$(command_path "$1")" || true)"
  [[ -z $hits ]] || fail "$1.md: $3; found: $hits"
}

# A heading is asserted as a WHOLE LINE, not as a substring. Measured by
# mutation: with a substring check, deleting `## Context` from the template
# block still passed, because the prose bullet explaining that section mentions
# the same words. Only the literal heading line is the template.
#
# require_heading <command> <heading-line> <what-it-means>
require_heading() {
  assertions=$((assertions + 1))
  grep -Fxq -- "$2" "$(command_path "$1")" ||
    fail "$1.md: $3 (no line reads exactly '$2')"
}

# literal_line_number <command> <fixed-string> -- the first line containing it,
# or nothing when it is absent.
literal_line_number() {
  local hit
  hit="$(grep -Fn -m1 -- "$2" "$(command_path "$1")")" || return 1
  printf '%s' "${hit%%:*}"
}

# heading_line_number <command> <heading-line> -- the first line that IS it.
heading_line_number() {
  local hit
  hit="$(grep -Fxn -m1 -- "$2" "$(command_path "$1")")" || return 1
  printf '%s' "${hit%%:*}"
}

# The roster is a SET: everything declared is on disk, and nothing else is. A
# per-file existence check alone would leave a new command governed by nothing.
disk_commands=()
for path in "$COMMANDS_DIR"/*.md; do
  [[ -e $path ]] || fail "no command files at all under $COMMANDS_DIR"
  base="$(basename "$path")"
  disk_commands+=("${base%.md}")
done
declared_roster="$(printf '%s\n' "${ALL_COMMANDS[@]}" | sort)"
disk_roster="$(printf '%s\n' ${disk_commands[@]+"${disk_commands[@]}"} | sort)"
assertions=$((assertions + 1))
[[ $declared_roster == "$disk_roster" ]] ||
  fail "the declared roster and $COMMANDS_DIR disagree; declared [$(printf '%s ' "${ALL_COMMANDS[@]}")], on disk [$(printf '%s ' ${disk_commands[@]+"${disk_commands[@]}"})]"

for name in "${ALL_COMMANDS[@]}"; do
  path="$(command_path "$name")"
  assertions=$((assertions + 1))
  [[ -s $path ]] || {
    fail "$name.md is missing or empty at $path"
    continue
  }

  # An em-dash in a prompt propagates into whatever the prompt writes.
  refuse_literal "$name" "$EM_DASH" 'contains an em-dash, which the operator forbids everywhere'

  # Nobody allowlists the bare CLI, whatever the command does.
  refuse_pattern "$name" "$BARE_GH_ALLOWLIST_PATTERN" \
    'allowlists the bare gh CLI in its frontmatter; allowlist Bash(npx -y gh-axi:*) instead'
done

for name in "${FRONTMATTER_COMMANDS[@]}"; do
  assertions=$((assertions + 1))
  [[ $(head -n 1 "$(command_path "$name")") == '---' ]] ||
    fail "$name.md does not open with a --- frontmatter fence, so Claude Code reads no metadata"
  require_pattern "$name" '^description: .+' 'has no frontmatter description'
  require_pattern "$name" '^allowed-tools:' \
    'has no frontmatter allowed-tools, so its Bash is unscoped'
  require_pattern "$name" '^  - [A-Za-z]' \
    'declares no allowed-tools entries, so the allowlist is empty'
done

for name in "${GITHUB_COMMANDS[@]}"; do
  require_literal "$name" 'npx -y gh-axi' 'never names gh-axi, so it has no sanctioned way to reach GitHub'
  refuse_pattern "$name" "$BARE_GH_CALL_PATTERN" \
    'invokes the bare gh CLI; every GitHub operation goes through npx -y gh-axi'
done

for name in "${PR_CREATING_COMMANDS[@]}"; do
  require_literal "$name" 'Bash(npx -y gh-axi:*)' \
    'does not allowlist gh-axi, so the create call it prescribes is not permitted'

  # The five headings, each present, and in the locked order.
  previous_line=0
  previous_heading=''
  for heading in "${TEMPLATE_HEADINGS[@]}"; do
    require_heading "$name" "$heading" "omits the '$heading' section of the pull request template"
    if heading_line="$(heading_line_number "$name" "$heading")"; then
      assertions=$((assertions + 1))
      if ((heading_line <= previous_line)); then
        fail "$name.md: '$heading' (line $heading_line) does not follow '$previous_heading' (line $previous_line); the template order is locked"
      fi
      previous_line="$heading_line"
      previous_heading="$heading"
    fi
  done

  # The review step itself, and its checklist.
  require_heading "$name" "$REVIEW_STEP_HEADING" \
    'has no mandatory review step, so nothing checks the body before it is posted'
  for item in "${REVIEW_CHECKLIST_ITEMS[@]}"; do
    require_literal "$name" "$item" "review checklist no longer mentions '$item'"
  done

  # The review step must come BEFORE the create call: a prompt runs top to
  # bottom, so a review below the create call is a review that never happens.
  require_literal "$name" "$PR_CREATE_CALL" 'never calls gh-axi pr create, so it opens no pull request'
  if review_line="$(heading_line_number "$name" "$REVIEW_STEP_HEADING")" &&
    create_line="$(literal_line_number "$name" "$PR_CREATE_CALL")"; then
    assertions=$((assertions + 1))
    ((review_line < create_line)) ||
      fail "$name.md: the create call (line $create_line) is at or above the review step (line $review_line), so the body can be posted unreviewed"
  fi

  # The body travels as a file, never as an inline argument.
  require_literal "$name" '--body-file' 'does not pass the pull request body with --body-file'
  refuse_pattern "$name" "$INLINE_BODY_FLAG_PATTERN" \
    'passes the pull request body inline, which mangles a multi-paragraph body'
done

for name in "${COMMIT_MESSAGE_COMMANDS[@]}"; do
  require_literal "$name" 'conventional-commits' \
    'does not invoke the conventional-commits skill for the message'
  require_pattern "$name" 'never[[:print:]]*Co-Authored-By' \
    'does not forbid a Co-Authored-By trailer, which every commit here must be free of'
done

for name in "${DESTRUCTIVE_COMMANDS[@]}"; do
  require_literal "$name" 'per-invocation confirmation' \
    'does not demand per-invocation confirmation before a destructive action'
  require_literal "$name" 'does not carry over' \
    'does not say that an earlier yes stops counting, which is what makes the confirmation per-invocation'
done

# pr-merge.md was written for a squash convention this repository does not use.
refuse_pattern 'pr-merge' '[Ss]quash' \
  'still mentions squashing; the convention is a merge commit'
require_pattern 'pr-merge' '(^|[[:space:]])--merge([[:space:]]|$)' \
  'does not pass --merge, so it would take gh-axi default merge method'
require_pattern 'pr-merge' "$MERGE_SUBJECT_PATTERN" \
  'does not spell out the "Merge pull request #N from webdavis/<branch> (#N)" subject convention'

if ((failures > 0)); then
  printf '\nclaude-commands-govern: %d failure(s) across %d assertions\n' "$failures" "$assertions" >&2
  exit 1
fi

printf 'PASS: %d assertions over %d command prompts in %s\n' \
  "$assertions" "${#ALL_COMMANDS[@]}" 'private_dot_claude/commands'
