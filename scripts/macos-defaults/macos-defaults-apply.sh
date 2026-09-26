#!/usr/bin/env bash
# macos-defaults-apply.sh, forced reapply of tracked macOS defaults.
#
# Same defaults-write loop as the Tier 1 chezmoiscript runner, but invocable
# on demand without bumping the chezmoi hash gate. Use after fiddling in
# System Settings to revert disk state to the YAML.
#
# The file is applied WHOLE or not at all. macos_defaults.yaml describes what the
# machine should be, so applying part of it leaves the Mac in a state neither the
# old file nor the new one describes, with no record of where the run stopped.
# That is why every record is validated and resolved before the first write, in
# two passes below: the runner template has always worked this way (a complete
# validation pass, then a render, and chezmoi runs nothing if the render failed),
# and this tool bypasses the render, so it has to hold the same line itself.
#
# Exit codes:
#   0: every enforce record was applied
#   2: the data file is missing, unreadable, or carries a record that failed
#      validation. Nothing was written.
#   other: a `defaults` write itself failed; earlier writes stand.

set -euo pipefail
# Note: no `shopt -s lastpipe` here, the loops below don't mutate
# outer-scope state (no counter to preserve, unlike drift.sh).

# shellcheck source=dot_local/libexec/macos-defaults/helpers/defaults-records.sh
source "$(dirname "${BASH_SOURCE[0]}")/macos-defaults-lib.sh"

# The status every refusal exits with, named once: the shared "data file missing,
# unreadable, or carrying an unusable record" code the sibling tools also use.
DATA_FILE_UNUSABLE_STATUS=2

DATA_FILE="$(macos_defaults_data_file)" || exit $?
require_readable_data_file "$DATA_FILE" || exit $?

# Pre-flight: close System Settings if open (same reason as runner).
osascript -e 'tell application "System Settings" to quit' 2>/dev/null || true

# format_planned_write <kind> <target> <key> <type> <value>, print one resolved
# write as a single line for the plan below. The five fields are joined with the
# same ASCII unit separator the record stream uses, which is safe BY
# CONSTRUCTION rather than by convention: the stream refuses any record carrying
# that byte or a newline in a field, so a plan line splits back into exactly the
# five fields that went into it. The reader is the fixed-arity `read` in the
# write pass; fixed arity is what preserves an empty trailing value, which
# `read -a` drops.
format_planned_write() { # <kind> <target> <key> <type> <value>
  local unit_separator=$'\x1f'
  printf '%s%s%s%s%s%s%s%s%s' \
    "$1" "$unit_separator" "$2" "$unit_separator" "$3" "$unit_separator" \
    "$4" "$unit_separator" "$5"
}

# The whole file, read and validated before anything is written. Held in a
# variable rather than piped into the loop for two reasons: the stream's REFUSAL
# status arrives here instead of being swallowed by a pipeline, and a refused
# stream emits nothing at all, so there is no partial record list to act on.
record_stream="$(defaults_records_unit_separated "$DATA_FILE")" || exit $?

# PASS 1, PLAN. Every record is classified, validated, and resolved into the
# exact write it will become. Nothing is written here, so a record that fails
# validation ends the run with the machine untouched, whatever its position in
# the file.
#
# The tier decides what happens before any payload field is looked at: verify
# records are read-only by declaration and manual records are runbook-applied, so
# both are skipped without a write, and an unrecognized tier refuses the run (the
# stream already refused the whole file; the case here keeps this planner from
# ever failing open into a write if the stream's rules drift). A system-scope
# record resolves to a plist path and must clear the write-time allowlist, which
# is apply's own gate: drift reads records it must never write, so gating reads
# would hide rows instead of reporting them.
planned_writes=()
while IFS= read -r record_line; do
  [[ -z $record_line ]] && continue
  IFS=$'\x1f' read -r domain key value_type value host scope plist_path tier <<<"$record_line"
  case "$tier" in
    enforce) ;;
    verify | manual)
      continue
      ;;
    *)
      printf 'error: unrecognized tier %q on record %s %s; refusing to write\n' \
        "$tier" "$domain" "$key" >&2
      exit "$DATA_FILE_UNUSABLE_STATUS"
      ;;
  esac
  validate_defaults_record "$domain" "$key" "$value_type" "$value" \
    "$host" "$scope" "$plist_path" "$tier" || exit "$DATA_FILE_UNUSABLE_STATUS"
  if [[ $scope == system ]]; then
    resolved_plist_path="$(resolve_system_plist_path "$domain" "$plist_path")" ||
      exit "$DATA_FILE_UNUSABLE_STATUS"
    # The same write-directory allowlist the render enforces, at the tool that
    # bypasses the render: apply reads the YAML directly, so this is the only
    # gate between a hand-edited record and a root write.
    require_system_plist_path_permitted "$resolved_plist_path" ||
      exit "$DATA_FILE_UNUSABLE_STATUS"
    planned_writes+=("$(format_planned_write system "$resolved_plist_path" "$key" "$value_type" "$value")")
  elif [[ -n $host ]]; then
    planned_writes+=("$(format_planned_write currenthost "$domain" "$key" "$value_type" "$value")")
  else
    planned_writes+=("$(format_planned_write user "$domain" "$key" "$value_type" "$value")")
  fi
done <<<"$record_stream"

# PASS 2, WRITE. Nothing about the DATA is decided here: every record was
# validated and every target resolved above, so this loop only runs plans. The
# kind is one of three words this script itself wrote a few lines up, never a
# field from the data file, and its refusal arm exists so an impossible fourth
# value cannot fall through into a write.
#
# The emptiness guard is not cosmetic: under `set -u` an older bash treats
# "${array[@]}" on an empty array as an unbound variable, and a file with no
# enforce records is a legitimate file.
if [[ ${#planned_writes[@]} -gt 0 ]]; then
  for planned_write in "${planned_writes[@]}"; do
    IFS=$'\x1f' read -r write_kind write_target write_key write_type write_value <<<"$planned_write"
    case "$write_kind" in
      user)
        defaults write "$write_target" "$write_key" "-$write_type" "$write_value"
        ;;
      currenthost)
        defaults -currentHost write "$write_target" "$write_key" "-$write_type" "$write_value"
        ;;
      system)
        system_defaults_write "$write_target" "$write_key" "$write_type" "$write_value"
        ;;
      *)
        printf 'error: planned write %q carries an unrecognized kind %q; refusing to write\n' \
          "$planned_write" "$write_kind" >&2
        exit "$DATA_FILE_UNUSABLE_STATUS"
        ;;
    esac
  done
fi

# Post-loop: restart processes per killall list.
yq eval -r '.macos.killall[]' "$DATA_FILE" |
  while read -r proc; do
    [[ -z $proc ]] && continue
    killall "$proc" 2>/dev/null || true
  done

exit 0
