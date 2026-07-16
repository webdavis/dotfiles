#!/usr/bin/env bash
# assert-hermes-superpowers-routing: re-assert the superpowers→hermes routing patches.
#
# The ~/.hermes/skills/hermes-superpowers mirror is hand-patched so the skills that
# have hermes-native adaptations are referenced by their adaptation names instead of
# superpowers:<name>, otherwise the docs route the model into the disabled legacy
# duplicates. Any re-mirror stomps those patches. This script re-applies them from
# the lock manifest (store the recipe, not the result): the superpowersRouting.map
# table pairs each legacy skill name (superpowers-writing-plans) with its adaptation
# (writing-plans), and superpowersRouting.slashCommandsSkill names the dispatcher
# skill whose SKILL.md must carry the adaptation-map lines.
#
# Three data-driven passes over every *.md under the mirror (never other file types):
#   1. Colon references: superpowers:<base> -> <adaptation>, word-boundary safe,
#      where <base> is the legacy name minus its superpowers- prefix. Non-mapped
#      references (superpowers:executing-plans, the superpowers:code-reviewer agent
#      type, ...) never match, and frontmatter name: lines are skipped outright.
#   2. Dispatcher invocations: name="<legacy>" -> name="<adaptation>" (the
#      skill_view(name="...") literals). The generic superpowers-{skill-name}
#      fallback placeholder is not a legacy name, so it never matches.
#   3. The slash-commands dispatcher SKILL.md must contain one adaptation-map line
#      per pair (- `<adaptation>` replaces `/<legacy>`); when any is missing the
#      canonical map section is appended once at end of file. A missing dispatcher
#      file is skipped: no dispatcher means nothing routes wrong.
# Prose around the references is NOT reconstructed, the recipe restores routing,
# not hand-written wording. Idempotent by construction: rewritten text contains no
# superpowers: prefix to re-match, and the appended section satisfies pass 3.
#
# Usage: assert-hermes-superpowers-routing.sh [--check|--dry-run] [--lock-file <path>]
#   (default)          rewrite stale references in place, log each fixed file
#   --check            exit 1 listing files with stale references; writes nothing
#   --dry-run          print the would-be rewrites as diffs; writes nothing
#   --lock-file <path> read the routing table from <path> instead of
#                      ~/.agents/custom-skill-lock.json (tests use fixtures; the
#                      conductor points at a worktree lock before cutover)
# Exit: 0 clean/fixed/skipped (absent mirror or lock is a fresh-machine skip),
#       1 --check found stale references, 2 usage or malformed lock table.
set -euo pipefail

LOCK="$HOME/.agents/custom-skill-lock.json"
TREE="$HOME/.hermes/skills/hermes-superpowers"
MODE="fix"

log() { printf '[assert-hermes-superpowers-routing] %s\n' "$*"; }
usage() {
  printf 'usage: assert-hermes-superpowers-routing.sh [--check|--dry-run] [--lock-file <path>]\n'
}

while (($#)); do
  case "$1" in
    --check) MODE="check" ;;
    --dry-run) MODE="dry-run" ;;
    --lock-file)
      if [[ -z ${2:-} ]]; then
        usage >&2
        exit 2
      fi
      LOCK="$2"
      shift
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
  shift
done

if [[ ! -d $TREE ]]; then
  log "skipping: $TREE does not exist (hermes superpowers mirror not set up on this machine)"
  exit 0
fi
if [[ ! -f $LOCK ]]; then
  log "skipping: lock file $LOCK does not exist"
  exit 0
fi

# Read the routing pairs. Every name feeds a regex and a written line, so
# validate the shape hard: lowercase kebab, legacy names superpowers-prefixed.
legacy_names=()
adaptation_names=()
while IFS=$'\t' read -r legacy adaptation; do
  if [[ ! $legacy =~ ^superpowers-[a-z0-9-]+$ || ! $adaptation =~ ^[a-z0-9-]+$ ]]; then
    log "malformed superpowersRouting.map pair in $LOCK: ${legacy} -> ${adaptation}"
    exit 2
  fi
  legacy_names+=("$legacy")
  adaptation_names+=("$adaptation")
done < <(jq -r '.superpowersRouting.map // {} | to_entries[] | "\(.key)\t\(.value)"' "$LOCK")
if ((${#legacy_names[@]} == 0)); then
  log "no superpowersRouting.map in $LOCK; nothing to assert"
  exit 0
fi
slash_skill="$(jq -r '.superpowersRouting.slashCommandsSkill // empty' "$LOCK")"
if [[ -n $slash_skill && ! $slash_skill =~ ^[a-z0-9-]+$ ]]; then
  log "malformed superpowersRouting.slashCommandsSkill in $LOCK: $slash_skill"
  exit 2
fi

# One tab-separated pair per line, handed to perl via the environment so no
# skill name is ever interpolated into code.
ROUTING_PAIRS="$(jq -r '.superpowersRouting.map | to_entries[] | "\(.key)\t\(.value)"' "$LOCK")"
export ROUTING_PAIRS

# The canonical adaptation-map section for the slash-commands dispatcher,
# rendered from the same pairs. Appended only when a map line is missing.
slash_map_lines=""
for i in "${!legacy_names[@]}"; do
  slash_map_lines+="- \`${adaptation_names[i]}\` replaces \`/${legacy_names[i]}\`"$'\n'
done
slash_section="## Hermes Adaptation Map

Hermes-adapted equivalents should use native Hermes skill names, not \`superpowers-*\` names:

${slash_map_lines}
Route overlapping commands through this map before falling back to \`skill_view(name=\"superpowers-{skill-name}\")\`."

# rewrite <original >staged: passes 1 and 2, line-wise, frontmatter name: skipped.
# perl over sed: BSD sed has no \b word boundary, and lookarounds keep a mapped
# base from matching inside a longer non-mapped skill name.
rewrite() {
  perl -e '
    my %map;
    for my $pair (split /\n/, $ENV{ROUTING_PAIRS}) {
      my ($legacy, $adaptation) = split /\t/, $pair;
      $map{$legacy} = $adaptation;
    }
    local $/;
    my $text = <STDIN>;
    my @lines = split /^/m, $text;
    for my $line (@lines) {
      next if $line =~ /^name:[ \t]/;
      for my $legacy (sort keys %map) {
        my $adaptation = $map{$legacy};
        (my $base = $legacy) =~ s/^superpowers-//;
        $line =~ s/(?<![\w-])superpowers:\Q$base\E(?![\w-])/$adaptation/g;
        $line =~ s/name="\Q$legacy\E"/name="$adaptation"/g;
      }
    }
    print join("", @lines);
  '
}

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT
staged="$tmpdir/staged.md"

slash_file=""
[[ -n $slash_skill ]] && slash_file="$TREE/$slash_skill/SKILL.md"

stale_count=0
fixed_count=0
# read on fd 3: the loop body runs perl, which reads stdin by design
while IFS= read -r -u3 -d '' file; do
  rewrite <"$file" >"$staged"
  if [[ $file == "$slash_file" ]]; then
    map_line_missing=""
    for i in "${!legacy_names[@]}"; do
      grep -qxF -- "- \`${adaptation_names[i]}\` replaces \`/${legacy_names[i]}\`" "$staged" ||
        map_line_missing=1
    done
    [[ -n $map_line_missing ]] && printf '\n%s\n' "$slash_section" >>"$staged"
  fi
  cmp -s "$file" "$staged" && continue
  case "$MODE" in
    check)
      log "STALE: $file (stale superpowers routing references; run assert-hermes-superpowers-routing.sh to fix)"
      ((++stale_count))
      ;;
    dry-run)
      log "would rewrite: $file"
      diff -u "$file" "$staged" || true
      ((++stale_count))
      ;;
    fix)
      cat "$staged" >"$file"
      log "rewrote: $file"
      ((++fixed_count))
      ;;
  esac
done 3< <(find "$TREE" -type f -name '*.md' -print0 | sort -z)

case "$MODE" in
  check)
    if ((stale_count > 0)); then
      log "check FAILED: $stale_count file(s) carry stale superpowers routing references"
      exit 1
    fi
    log "check OK: routing references match the lock"
    ;;
  dry-run)
    log "dry-run: $stale_count file(s) would be rewritten"
    ;;
  fix)
    if ((fixed_count > 0)); then
      log "re-asserted routing in $fixed_count file(s), something rewrote the mirror since the last run"
    else
      log "routing clean: nothing to rewrite"
    fi
    ;;
esac
