# shellcheck shell=bash

text_is_empty() {
  local text=$1
  [[ -z $text ]]
}

file_is_readable() {
  local file=$1
  [[ -r $file ]]
}

source_directory_override_is_set() {
  [[ -n ${MACOS_DEFAULTS_SOURCE_DIR+x} ]]
}

print_source_directory_override() {
  if text_is_empty "$MACOS_DEFAULTS_SOURCE_DIR"; then
    printf 'error: MACOS_DEFAULTS_SOURCE_DIR is set but empty; refusing to resolve another checkout\n' >&2
    return 1
  fi
  printf '%s\n' "$MACOS_DEFAULTS_SOURCE_DIR"
}

current_worktree_top() {
  (
    unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE
    git rev-parse --show-toplevel 2>/dev/null
  )
}

directory_is_a_chezmoi_source_tree() {
  local directory=$1
  [[ -n $directory && -f "$directory/.chezmoiversion" ]]
}

print_worktree_source_path() {
  local worktree_top=$1
  local source_path
  if ! source_path="$(chezmoi --source="$worktree_top" source-path)"; then
    printf 'error: chezmoi --source=%s source-path failed; refusing to fall back to another checkout\n' \
      "$worktree_top" >&2
    return 1
  fi
  printf '%s\n' "$source_path"
}

print_configured_source_path() {
  local source_path
  if ! source_path="$(chezmoi source-path)"; then
    printf 'error: chezmoi source-path failed; the chezmoi source directory is unknown\n' >&2
    return 1
  fi
  printf '%s\n' "$source_path"
}

resolve_source_directory() {
  local worktree_top
  if source_directory_override_is_set; then
    print_source_directory_override
    return
  fi
  worktree_top="$(current_worktree_top)"
  if directory_is_a_chezmoi_source_tree "$worktree_top"; then
    print_worktree_source_path "$worktree_top"
    return
  fi
  print_configured_source_path
}

macos_defaults_data_file() {
  local source_directory
  source_directory="$(resolve_source_directory)" || return 2
  if text_is_empty "$source_directory"; then
    printf 'error: resolved an empty chezmoi source directory for macos_defaults.yaml\n' >&2
    return 2
  fi
  printf '%s/.chezmoidata/macos_defaults.yaml\n' "$source_directory"
}

require_readable_data_file() {
  local data_file=$1
  if ! file_is_readable "$data_file"; then
    printf 'error: cannot read %s\n' "$data_file" >&2
    return 2
  fi
}

defaults_records_join_expression() {
  local record_selector=$1
  local unit_separator=$'\x1f'
  printf '%s | [.domain, .key, .type, .value, (.host // ""), (.scope // "user"), (.plist_path // ""), .tier] | join("%s")' \
    "$record_selector" "$unit_separator"
}

defaults_records_field_count() {
  local unit_separator=$'\x1f'
  local separators_only="${1//[!$unit_separator]/}"
  printf '%s' "$((${#separators_only} + 1))"
}

first_non_blank_line() {
  local text=$1
  local line
  while IFS= read -r line; do
    if [[ -n $line ]]; then
      printf '%s' "$line"
      return 0
    fi
  done <<<"$text"
}

defaults_records_locate_malformed() {
  local data_file="$1" declared_record_count="$2"
  local index record_render line_count
  for ((index = 0; index < declared_record_count; index++)); do
    record_render="$(yq eval -r "$(defaults_records_join_expression ".macos.defaults[$index]")" "$data_file")" || continue
    line_count="$(printf '%s\n' "$record_render" | wc -l | tr -d ' ')"
    if [[ $line_count -ne 1 || $(defaults_records_field_count "$record_render") -ne 8 ]]; then
      printf 'record %d (domain %s, key %s)' "$index" \
        "$(yq eval -r ".macos.defaults[$index].domain" "$data_file" | head -1)" \
        "$(yq eval -r ".macos.defaults[$index].key" "$data_file" | head -1)"
      return 0
    fi
  done
  printf 'a record this locator could not identify'
}

DEFAULTS_RECORDS_LIST_KIND='seq'
DEFAULTS_RECORDS_LIST_TAG='!!seq'
DEFAULTS_RECORDS_NONSPECIFIC_TAG='!'
DEFAULTS_RECORDS_MAP_KIND='map'
DEFAULTS_RECORDS_MAP_TAG='!!map'
DEFAULTS_RECORDS_SCALAR_KIND='scalar'
DEFAULTS_RECORDS_ABSENT_TAG='!!null'

DEFAULTS_RECORDS_SHAPE_EXPRESSION='.macos.defaults | [kind, tag] | join(" ")'
DEFAULTS_RECORDS_COUNT_EXPRESSION='.macos.defaults | length'
MACOS_DEFAULTS_SHAPE_ANSWER_PATTERN='^[[:alpha:]]+ [^[:space:]]+$'

shape_answer_is_one_kind_and_one_tag() {
  local shape_answer=$1
  [[ $shape_answer =~ $MACOS_DEFAULTS_SHAPE_ANSWER_PATTERN ]]
}

node_kind_in_shape_answer() {
  local shape_answer=$1
  printf '%s' "${shape_answer%% *}"
}

node_tag_in_shape_answer() {
  local shape_answer=$1
  printf '%s' "${shape_answer#* }"
}

tag_is_a_plain_list_tag() {
  local node_tag=$1
  [[ $node_tag == "$DEFAULTS_RECORDS_LIST_TAG" ]]
}

tag_is_a_plain_or_nonspecific_list_tag() {
  local node_tag=$1
  [[ $node_tag == "$DEFAULTS_RECORDS_LIST_TAG" || $node_tag == "$DEFAULTS_RECORDS_NONSPECIFIC_TAG" ]]
}

tag_is_a_plain_map_tag() {
  local node_tag=$1
  [[ $node_tag == "$DEFAULTS_RECORDS_MAP_TAG" ]]
}

tag_marks_an_absent_node() {
  local node_tag=$1
  [[ $node_tag == "$DEFAULTS_RECORDS_ABSENT_TAG" ]]
}

records_declaration_verdict() {
  local shape_answer=$1
  local node_kind node_tag
  if ! shape_answer_is_one_kind_and_one_tag "$shape_answer"; then
    printf 'other\n'
    return 0
  fi
  node_kind="$(node_kind_in_shape_answer "$shape_answer")"
  node_tag="$(node_tag_in_shape_answer "$shape_answer")"
  case $node_kind in
    "$DEFAULTS_RECORDS_LIST_KIND")
      if tag_is_a_plain_or_nonspecific_list_tag "$node_tag"; then
        printf 'list\n'
      else
        printf 'mistagged\n'
      fi
      ;;
    "$DEFAULTS_RECORDS_MAP_KIND") printf 'map\n' ;;
    *)
      if tag_marks_an_absent_node "$node_tag"; then
        printf 'absent\n'
      else
        printf 'other\n'
      fi
      ;;
  esac
}

DEFAULTS_DATA_FILE_RULES_EXPRESSION='[[.. | select(kind == "map") | keys | select(length != (unique | length))] | length, [.. | select(kind == "map") | to_entries | .[] | .key | select(kind != "scalar")] | length, [.. | select(kind == "alias")] | length] | join(" ")'

MACOS_DEFAULTS_DOCUMENT_START_MARKER_PATTERN='^---([[:space:]].*)?$'

line_is_blank() {
  local line=$1
  [[ $line =~ ^[[:space:]]*$ ]]
}

line_is_a_yaml_comment() {
  local line=$1
  [[ $line =~ ^[[:space:]]*# ]]
}

line_is_a_yaml_directive() {
  local line=$1
  [[ $line == %* ]]
}

line_starts_a_yaml_document() {
  local line=$1
  [[ $line =~ $MACOS_DEFAULTS_DOCUMENT_START_MARKER_PATTERN ]]
}

data_file_line_carries_document_content() {
  local line=$1
  ! line_is_blank "$line" && ! line_is_a_yaml_comment "$line" && ! line_is_a_yaml_directive "$line"
}

no_document_start_seen_yet() {
  local start_marker_count=$1
  ((start_marker_count == 0))
}

data_file_document_count() {
  local data_file=$1
  local line start_marker_count=0 content_before_first_marker=0
  file_is_readable "$data_file" || return 1
  while IFS= read -r line || [[ -n $line ]]; do
    if line_starts_a_yaml_document "$line"; then
      start_marker_count=$((start_marker_count + 1))
    elif no_document_start_seen_yet "$start_marker_count" && data_file_line_carries_document_content "$line"; then
      content_before_first_marker=1
    fi
  done <"$data_file" || return 1
  printf '%s\n' "$((start_marker_count + content_before_first_marker))"
}

print_multiple_documents_refusal() {
  local data_file=$1
  printf 'error: %s contains more than one YAML document; chezmoi keeps the FIRST document and silently discards the rest, so an apply would write only the records above the first --- while every tool here refuses the file; merge the documents into one\n' \
    "$data_file" >&2
}

document_count_is_a_number() {
  local document_count=$1
  [[ $document_count =~ ^[0-9]+$ ]]
}

document_count_is_at_most_one() {
  local document_count=$1
  ((document_count <= 1))
}

require_data_file_holds_one_document() {
  local data_file=$1
  local document_count
  if ! document_count="$(data_file_document_count "$data_file")" || ! document_count_is_a_number "$document_count"; then
    printf 'error: cannot count the YAML documents in %s; refusing a file whose document count could not be read\n' \
      "$data_file" >&2
    return 2
  fi
  if document_count_is_at_most_one "$document_count"; then
    return 0
  fi
  print_multiple_documents_refusal "$data_file"
  return 2
}

MACOS_DEFAULTS_BOUNDED_COUNT_PATTERN='(0|[1-9][0-9]{0,6})'
MACOS_DEFAULTS_USABLE_COUNT_PATTERN="^$MACOS_DEFAULTS_BOUNDED_COUNT_PATTERN\$"
MACOS_DEFAULTS_RULES_ANSWER_PATTERN="^$MACOS_DEFAULTS_BOUNDED_COUNT_PATTERN $MACOS_DEFAULTS_BOUNDED_COUNT_PATTERN $MACOS_DEFAULTS_BOUNDED_COUNT_PATTERN\$"

text_spans_several_lines() {
  local text=$1
  [[ $text == *$'\n'* ]]
}

rules_answer_is_three_counts() {
  local rules_answer=$1
  [[ $rules_answer =~ $MACOS_DEFAULTS_RULES_ANSWER_PATTERN ]]
}

count_is_above_zero() {
  local count=$1
  ((count > 0))
}

data_file_rules_verdict() {
  local rules_answer=$1
  local duplicate_mapping_key_count complex_mapping_key_count alias_count
  if text_spans_several_lines "$rules_answer"; then
    printf 'multiple_documents\n'
    return 0
  fi
  if ! rules_answer_is_three_counts "$rules_answer"; then
    printf 'unclassifiable\n'
    return 0
  fi
  IFS=' ' read -r duplicate_mapping_key_count complex_mapping_key_count alias_count <<<"$rules_answer"
  if count_is_above_zero "$duplicate_mapping_key_count"; then
    printf 'duplicate_mapping_key\n'
  elif count_is_above_zero "$complex_mapping_key_count"; then
    printf 'complex_mapping_key\n'
  elif count_is_above_zero "$alias_count"; then
    printf 'alias\n'
  else
    printf 'satisfied\n'
  fi
}

read_data_file_rules_answer() {
  local data_file=$1
  if ! yq eval -r "$DEFAULTS_DATA_FILE_RULES_EXPRESSION" "$data_file"; then
    printf 'error: cannot check the whole-file rules of %s\n' "$data_file" >&2
    return 2
  fi
}

data_file_rules_are_satisfied() {
  local rules_verdict=$1
  [[ $rules_verdict == satisfied ]]
}

print_data_file_rules_refusal() {
  local data_file=$1 rules_verdict=$2 rules_answer=$3
  case $rules_verdict in
    multiple_documents)
      print_multiple_documents_refusal "$data_file"
      ;;
    duplicate_mapping_key)
      printf 'error: %s declares the same mapping key twice; chezmoi refuses the whole file (mapping key already defined) while yq keeps both entries and reads the LAST, so the two readers would not even agree on which records exist; delete the duplicate key\n' \
        "$data_file" >&2
      ;;
    complex_mapping_key)
      printf 'error: %s uses a mapping key that is not a scalar (a sequence or a mapping as a key); chezmoi refuses the whole file with "found an invalid key for this map" while yq reads it, so the runner template would apply nothing; give every key a plain scalar name\n' \
        "$data_file" >&2
      ;;
    alias)
      printf 'error: %s uses a YAML alias or merge key; both are ordinary YAML and the runner template resolves them, but this schema does not allow them, because yq resolves an alias in some expressions and not others and this reader would judge a record by fields it cannot see; write the record out in full\n' \
        "$data_file" >&2
      ;;
    *)
      printf 'error: cannot classify the whole-file rules of %s; yq answered %q\n' \
        "$data_file" "$rules_answer" >&2
      ;;
  esac
}

require_data_file_rules_satisfied() {
  local data_file=$1
  local rules_answer rules_verdict
  require_data_file_holds_one_document "$data_file" || return 2
  rules_answer="$(read_data_file_rules_answer "$data_file")" || return 2
  rules_verdict="$(data_file_rules_verdict "$rules_answer")"
  if data_file_rules_are_satisfied "$rules_verdict"; then
    return 0
  fi
  print_data_file_rules_refusal "$data_file" "$rules_verdict" "$rules_answer"
  return 2
}

UTF8_BYTE_ORDER_MARK=$'\xef\xbb\xbf'
UTF8_BYTE_ORDER_MARK_BYTE_COUNT=3

leading_bytes_of_file() {
  local file=$1 byte_count=$2
  LC_ALL=C head -c "$byte_count" -- "$file" 2>/dev/null
}

data_file_begins_with_byte_order_mark() {
  local data_file=$1
  local leading_bytes
  leading_bytes="$(leading_bytes_of_file "$data_file" "$UTF8_BYTE_ORDER_MARK_BYTE_COUNT")" || return 1
  [[ $leading_bytes == "$UTF8_BYTE_ORDER_MARK" ]]
}

require_no_byte_order_mark() {
  local data_file=$1
  if data_file_begins_with_byte_order_mark "$data_file"; then
    printf 'error: %s begins with a UTF-8 byte order mark; yq strips it and reads the file, but the runner template does not and cannot then find .macos at all, so the two readers disagree about this file; remove the first three bytes\n' \
      "$data_file" >&2
    return 2
  fi
}

count_is_usable() {
  local count=$1
  [[ $count =~ $MACOS_DEFAULTS_USABLE_COUNT_PATTERN ]]
}

read_records_shape() {
  local data_file=$1
  if ! yq eval -r "$DEFAULTS_RECORDS_SHAPE_EXPRESSION" "$data_file"; then
    printf 'error: cannot determine the shape of .macos.defaults in %s\n' "$data_file" >&2
    return 2
  fi
}

records_are_declared_as_a_list() {
  local declaration_verdict=$1
  [[ $declaration_verdict == list ]]
}

print_records_declaration_refusal() {
  local data_file=$1 declaration_verdict=$2 shape_answer=$3
  case $declaration_verdict in
    mistagged)
      printf 'error: %s tags .macos.defaults as %q; the record list is a real sequence, so only its TAG is wrong, and the only tags accepted on it are %s and the non-specific %s, because the runner template refuses several of the others with a parse error while this reader would take the records, so the two readers would disagree about whether this file has any settings at all; delete the tag\n' \
        "$data_file" "$(node_tag_in_shape_answer "$shape_answer")" "$DEFAULTS_RECORDS_LIST_TAG" "$DEFAULTS_RECORDS_NONSPECIFIC_TAG" >&2
      ;;
    map)
      printf 'error: %s declares .macos.defaults as a map, but it must be a LIST of records; a map is read in sorted key order by the runner template and in document order here, so the two would apply records in different orders\n' \
        "$data_file" >&2
      ;;
    absent)
      printf 'error: %s declares no .macos.defaults record list, so every tracked setting would be silently skipped and the run would still report success; to track no records, declare an explicitly empty list, defaults: []\n' \
        "$data_file" >&2
      ;;
    *)
      printf 'error: %s does not declare .macos.defaults as a LIST of records; yq answered %q for its kind and tag\n' \
        "$data_file" "$shape_answer" >&2
      ;;
  esac
}

require_records_declared_as_a_list() {
  local data_file=$1 shape_answer=$2
  local declaration_verdict
  declaration_verdict="$(records_declaration_verdict "$shape_answer")"
  if records_are_declared_as_a_list "$declaration_verdict"; then
    return 0
  fi
  print_records_declaration_refusal "$data_file" "$declaration_verdict" "$shape_answer"
  return 2
}

read_declared_record_count() {
  local data_file=$1
  if ! yq eval -r "$DEFAULTS_RECORDS_COUNT_EXPRESSION" "$data_file"; then
    printf 'error: cannot count the records in %s\n' "$data_file" >&2
    return 2
  fi
}

require_usable_record_count() {
  local data_file=$1 declared_record_count=$2
  if count_is_usable "$declared_record_count"; then
    return 0
  fi
  printf 'error: %s produced an unusable record count %q; refusing to emit a stream that cannot be checked\n' \
    "$data_file" "$declared_record_count" >&2
  return 2
}

defaults_records_declared_count() {
  local data_file=$1
  local shape_answer declared_record_count
  require_no_byte_order_mark "$data_file" || return 2
  shape_answer="$(read_records_shape "$data_file")" || return 2
  require_data_file_rules_satisfied "$data_file" || return 2
  require_records_declared_as_a_list "$data_file" "$shape_answer" || return 2
  declared_record_count="$(read_declared_record_count "$data_file")" || return 2
  require_usable_record_count "$data_file" "$declared_record_count" || return 2
  printf '%s\n' "$declared_record_count"
}

defaults_records_raw_stream() {
  local data_file="$1"
  if ! yq eval -r "$(defaults_records_join_expression '.macos.defaults[]')" "$data_file"; then
    printf 'error: cannot read the records in %s\n' "$data_file" >&2
    return 2
  fi
}

defaults_records_validate_stream() {
  local data_file="$1" declared_record_count="$2" raw_records="$3"
  local line field_count checked_line_count=0
  local record_domain record_key record_type record_value
  local record_host record_scope record_plist_path record_tier
  while IFS= read -r line; do
    [[ -z $line ]] && continue
    field_count="$(defaults_records_field_count "$line")"
    if [[ $field_count -ne 8 ]]; then
      printf 'error: %s: %s renders %s fields, not 8; a field value contains a unit separator (0x1f) or a newline\n' \
        "$data_file" "$(defaults_records_locate_malformed "$data_file" "$declared_record_count")" \
        "$field_count" >&2
      return 2
    fi
    IFS=$'\x1f' read -r record_domain record_key record_type record_value \
      record_host record_scope record_plist_path record_tier <<<"$line"
    case "$record_tier" in
      enforce | verify | manual) ;;
      *)
        printf 'error: %s: record (domain %s, key %s) has a missing, blank, or unrecognized tier %q; declare tier: enforce, verify, or manual\n' \
          "$data_file" "$record_domain" "$record_key" "$record_tier" >&2
        return 2
        ;;
    esac
    if ! validate_defaults_record "$record_domain" "$record_key" "$record_type" \
      "$record_value" "$record_host" "$record_scope" "$record_plist_path" "$record_tier"; then
      printf 'error: %s: the record above is not usable; the whole file is refused\n' "$data_file" >&2
      return 2
    fi
    checked_line_count=$((checked_line_count + 1))
  done <<<"$raw_records"

  if [[ $checked_line_count -ne $declared_record_count ]]; then
    printf 'error: %s declares %s record(s) but the record stream has %s line(s); %s contains a newline\n' \
      "$data_file" "$declared_record_count" "$checked_line_count" \
      "$(defaults_records_locate_malformed "$data_file" "$declared_record_count")" >&2
    return 2
  fi
}

MACOS_DEFAULTS_KILLALL_SHAPE_EXPRESSION='[(.macos.killall | kind), (.macos.killall | tag)] | join(" ")'

killall_list_verdict() {
  local shape_answer=$1
  local node_kind node_tag
  if ! shape_answer_is_one_kind_and_one_tag "$shape_answer"; then
    printf 'unclassifiable\n'
    return 0
  fi
  node_kind="$(node_kind_in_shape_answer "$shape_answer")"
  node_tag="$(node_tag_in_shape_answer "$shape_answer")"
  case $node_kind in
    "$DEFAULTS_RECORDS_LIST_KIND")
      if tag_is_a_plain_list_tag "$node_tag"; then
        printf 'iterable\n'
      else
        printf 'mistagged\n'
      fi
      ;;
    "$DEFAULTS_RECORDS_MAP_KIND")
      if tag_is_a_plain_map_tag "$node_tag"; then
        printf 'iterable\n'
      else
        printf 'mistagged\n'
      fi
      ;;
    "$DEFAULTS_RECORDS_SCALAR_KIND")
      if tag_marks_an_absent_node "$node_tag"; then
        printf 'undeclared\n'
      else
        printf 'scalar\n'
      fi
      ;;
    *) printf 'unclassifiable\n' ;;
  esac
}

read_killall_shape() {
  local data_file=$1
  if ! yq eval -r "$MACOS_DEFAULTS_KILLALL_SHAPE_EXPRESSION" "$data_file"; then
    printf 'error: cannot determine the shape of .macos.killall in %s\n' "$data_file" >&2
    return 2
  fi
}

killall_list_is_usable() {
  local killall_verdict=$1
  [[ $killall_verdict == iterable || $killall_verdict == undeclared ]]
}

print_killall_refusal() {
  local data_file=$1 killall_verdict=$2 shape_answer=$3
  case $killall_verdict in
    mistagged)
      printf 'error: %s tags .macos.killall as %q; the list of process names is a real container, so only its TAG is wrong, and the only tags accepted on it are %s on a list and %s on a mapping, because the runner template refuses several of the others with a parse error while this reader would call the file usable, so the two readers would disagree about whether this file can be applied at all; delete the tag\n' \
        "$data_file" "$(node_tag_in_shape_answer "$shape_answer")" "$DEFAULTS_RECORDS_LIST_TAG" "$DEFAULTS_RECORDS_MAP_TAG" >&2
      ;;
    scalar)
      printf 'error: %s declares .macos.killall as a plain scalar, but it must be a LIST of process names; the runner template walks it and dies with "range can%st iterate over" that value, refusing the whole apply, while every tool here would read the file as usable; write it as a list, killall: [Dock]\n' \
        "$data_file" "'" >&2
      ;;
    *)
      printf 'error: %s does not declare .macos.killall as a list of process names; yq answered %q for its kind and tag\n' \
        "$data_file" "$shape_answer" >&2
      ;;
  esac
}

require_data_file_killall_is_iterable() {
  local data_file=$1
  local shape_answer killall_verdict
  shape_answer="$(read_killall_shape "$data_file")" || return 2
  killall_verdict="$(killall_list_verdict "$shape_answer")"
  if killall_list_is_usable "$killall_verdict"; then
    return 0
  fi
  print_killall_refusal "$data_file" "$killall_verdict" "$shape_answer"
  return 2
}

defaults_records_declare_a_value() {
  local data_file="$1"
  local valueless_indices first_valueless_index
  if ! valueless_indices="$(yq eval -r '.macos.defaults | to_entries | .[] | select(.value.tier == "enforce" or .value.tier == "verify") | select((.value | has("value") | not) or (.value.value == null)) | .key' "$data_file")"; then
    printf 'error: cannot check which records in %s declare a value\n' "$data_file" >&2
    return 2
  fi
  [[ -z $valueless_indices ]] && return 0
  first_valueless_index="$(printf '%s\n' "$valueless_indices" | head -1)"
  printf 'error: %s: record %s (domain %s, key %s) has a blank value; give it a value or remove the field\n' \
    "$data_file" "$first_valueless_index" \
    "$(yq eval -r ".macos.defaults[$first_valueless_index].domain" "$data_file" | head -1)" \
    "$(yq eval -r ".macos.defaults[$first_valueless_index].key" "$data_file" | head -1)" >&2
  return 2
}

MACOS_DEFAULTS_STRING_ONLY_RECORD_FIELDS=("domain" "key" "host" "scope" "plist_path")
MACOS_DEFAULTS_VALUE_RECORD_FIELD="value"
MACOS_DEFAULTS_PLAIN_STRING_TAG='!!str'

MACOS_DEFAULTS_CANONICAL_BOOL_PATTERN='^(true|false)$'
MACOS_DEFAULTS_CANONICAL_INT_PATTERN='^(0|-?[1-9][0-9]*)$'
MACOS_DEFAULTS_CANONICAL_FLOAT_PATTERN='^-?(0|[1-9][0-9]*)[.][0-9]*[1-9]$'

MACOS_DEFAULTS_CANONICAL_FLOAT_DIGIT_BOUND_PATTERN='^-?[0-9.]{1,16}$'

record_field_node_description() {
  local node_kind="$1" node_tag="$2"
  if [[ $node_kind == "$DEFAULTS_RECORDS_SCALAR_KIND" ]]; then
    printf '%s' "$node_tag"
    return 0
  fi
  printf '%s tagged %s' "$node_kind" "$node_tag"
}

# shellcheck disable=SC2016  # deliberate: $entry, $field, $fieldKind, $fieldTag
defaults_records_field_type_expression() {
  local unit_separator=$'\x1f'
  local string_only_field string_only_selection='' schema_field_selection
  for string_only_field in "${MACOS_DEFAULTS_STRING_ONLY_RECORD_FIELDS[@]}"; do
    [[ -n $string_only_selection ]] && string_only_selection+=' or '
    string_only_selection+="(\$field.key == \"$string_only_field\")"
  done
  schema_field_selection="$string_only_selection or (\$field.key == \"$MACOS_DEFAULTS_VALUE_RECORD_FIELD\")"
  local canonical_value_spellings
  canonical_value_spellings="$(printf '((%s == "!!bool") and (%s | test("%s"))) or ((%s == "!!int") and (%s | test("%s"))) or ((%s == "!!float") and (%s | test("%s")) and (%s | test("%s")))' \
    '$fieldTag' '$fieldText' "$MACOS_DEFAULTS_CANONICAL_BOOL_PATTERN" \
    '$fieldTag' '$fieldText' "$MACOS_DEFAULTS_CANONICAL_INT_PATTERN" \
    '$fieldTag' '$fieldText' "$MACOS_DEFAULTS_CANONICAL_FLOAT_PATTERN" \
    '$fieldText' "$MACOS_DEFAULTS_CANONICAL_FLOAT_DIGIT_BOUND_PATTERN")"
  printf '[.macos.defaults | to_entries | .[] | . as $entry | ($entry.value | to_entries | .[]) as $field | ($field.value | kind) as $fieldKind | ($field.value | tag) as $fieldTag | ($field.value | tostring) as $fieldText | select(((%s) and ($fieldKind != "%s")) or ((%s) and ($fieldTag != "%s")) or (($field.key == "%s") and ($fieldTag != "%s") and ((%s) | not))) | [($entry.key | tostring), ($field.key | tostring), $fieldKind, $fieldTag] | join("%s")] | .[]' \
    "$schema_field_selection" "$DEFAULTS_RECORDS_SCALAR_KIND" \
    "$string_only_selection" "$MACOS_DEFAULTS_PLAIN_STRING_TAG" \
    "$MACOS_DEFAULTS_VALUE_RECORD_FIELD" "$MACOS_DEFAULTS_PLAIN_STRING_TAG" \
    "$canonical_value_spellings" "$unit_separator"
}

defaults_records_declare_agreeing_field_types() {
  local data_file="$1"
  local field_types first_offender
  local record_index offending_field offending_kind offending_tag
  if ! field_types="$(yq eval -r "$(defaults_records_field_type_expression)" "$data_file")"; then
    printf 'error: cannot check the field types of the records in %s\n' "$data_file" >&2
    return 2
  fi
  first_offender="$(first_non_blank_line "$field_types")"
  [[ -z $first_offender ]] && return 0
  IFS=$'\x1f' read -r record_index offending_field offending_kind offending_tag <<<"$first_offender"
  printf 'error: %s: record %s (%s) declares %s as %s; this reader renders a scalar as the text the file spells it with and the runner template renders it as Go formats the parsed value, so the two would not write the same thing out of this record; quote the value\n' \
    "$data_file" "$record_index" "$(defaults_record_reference "$data_file" "$record_index")" \
    "$offending_field" "$(record_field_node_description "$offending_kind" "$offending_tag")" >&2
  return 2
}

MACOS_DEFAULTS_FIELDS_FORBIDDEN_ON_MANUAL=("type" "value" "host" "scope" "plist_path")
MACOS_DEFAULTS_FIELDS_FORBIDDEN_ON_ENFORCE=("runbook")

record_field_is_forbidden_for_tier() {
  local tier="$1" field="$2" forbidden_field
  local -a forbidden_fields=()
  case $tier in
    manual) forbidden_fields=("${MACOS_DEFAULTS_FIELDS_FORBIDDEN_ON_MANUAL[@]}") ;;
    enforce) forbidden_fields=("${MACOS_DEFAULTS_FIELDS_FORBIDDEN_ON_ENFORCE[@]}") ;;
    *) return 1 ;;
  esac
  for forbidden_field in "${forbidden_fields[@]}"; do
    [[ $field == "$forbidden_field" ]] && return 0
  done
  return 1
}

record_tier_requires_a_runbook() {
  local tier=$1
  [[ $tier == manual ]]
}

# shellcheck disable=SC2016  # deliberate: $entry and $field are yq's own
DEFAULTS_RECORD_DECLARED_FIELDS_EXPRESSION='.macos.defaults | to_entries | .[] | . as $entry | ($entry.value | keys | .[]) as $field | [($entry.key | tostring), ($entry.value.tier // ""), (($entry.value | has("runbook")) and ($entry.value.runbook != null) and ($entry.value.runbook != "")) | tostring, ($field | tostring)] | join("'$'\x1f''")'

DEFAULTS_RECORD_DECLARED_FIELDS_FIELD_COUNT=4

DEFAULTS_RECORD_DECLARED_FIELD_TOTAL_EXPRESSION='[.macos.defaults | .[] | keys | .[]] | length'

defaults_records_declared_field_line_count() {
  local declared_fields_stream=$1
  local line line_count=0
  while IFS= read -r line; do
    [[ -z $line ]] && continue
    line_count=$((line_count + 1))
  done <<<"$declared_fields_stream"
  printf '%s\n' "$line_count"
}

defaults_records_match_declared_tier() {
  local data_file="$1"
  local declared_fields declared_field_total line field_count read_line_count=0
  local record_index record_tier record_runbook_is_usable record_field
  local -A record_tier_by_index=() runbook_usable_by_index=()
  local -a record_indices_in_order=()
  if ! declared_fields="$(yq eval -r "$DEFAULTS_RECORD_DECLARED_FIELDS_EXPRESSION" "$data_file")"; then
    printf 'error: cannot read which fields the records in %s declare\n' "$data_file" >&2
    return 2
  fi
  if ! declared_field_total="$(yq eval -r "$DEFAULTS_RECORD_DECLARED_FIELD_TOTAL_EXPRESSION" "$data_file")"; then
    printf 'error: cannot count the fields the records in %s declare\n' "$data_file" >&2
    return 2
  fi
  if ! count_is_usable "$declared_field_total"; then
    printf 'error: %s produced an unusable declared-field count %q; refusing to check rules against a stream that cannot be checked\n' \
      "$data_file" "$declared_field_total" >&2
    return 2
  fi
  read_line_count="$(defaults_records_declared_field_line_count "$declared_fields")"
  if [[ $read_line_count -ne $declared_field_total ]]; then
    printf 'error: %s: the records declare %s field(s) but the field stream has %s line(s); a field NAME contains a newline, so the rules for its tier cannot be checked against it; rename the field\n' \
      "$data_file" "$declared_field_total" "$read_line_count" >&2
    return 2
  fi
  while IFS= read -r line; do
    [[ -z $line ]] && continue
    field_count="$(defaults_records_field_count "$line")"
    if [[ $field_count -ne $DEFAULTS_RECORD_DECLARED_FIELDS_FIELD_COUNT ]]; then
      printf 'error: %s: a record declares a field name containing a newline or a unit separator (0x1f), which cannot be checked against the rules for its tier\n' \
        "$data_file" >&2
      return 2
    fi
    IFS=$'\x1f' read -r record_index record_tier record_runbook_is_usable record_field <<<"$line"
    [[ -n ${record_tier_by_index[$record_index]+x} ]] || record_indices_in_order+=("$record_index")
    record_tier_by_index["$record_index"]="$record_tier"
    runbook_usable_by_index["$record_index"]="$record_runbook_is_usable"
    if record_field_is_forbidden_for_tier "$record_tier" "$record_field"; then
      printf 'error: %s: record %s (%s) carries %s; a %s control renders no such payload, so a field it cannot use means the declared tier is wrong; either drop the field or declare the tier that consumes it\n' \
        "$data_file" "$record_index" "$(defaults_record_reference "$data_file" "$record_index")" \
        "$record_field" "$record_tier" >&2
      return 2
    fi
  done <<<"$declared_fields"

  for record_index in "${record_indices_in_order[@]}"; do
    record_tier_requires_a_runbook "${record_tier_by_index[$record_index]}" || continue
    [[ ${runbook_usable_by_index[$record_index]} == true ]] && continue
    printf 'error: %s: record %s (%s) declares tier %s but names no runbook section; a manual control renders no write, so the runbook pointer is the whole record; name the runbook section\n' \
      "$data_file" "$record_index" "$(defaults_record_reference "$data_file" "$record_index")" \
      "${record_tier_by_index[$record_index]}" >&2
    return 2
  done
}

defaults_record_reference() {
  local data_file=$1 record_index=$2
  printf 'domain %s, key %s' \
    "$(yq eval -r ".macos.defaults[$record_index].domain" "$data_file" | head -1)" \
    "$(yq eval -r ".macos.defaults[$record_index].key" "$data_file" | head -1)"
}

defaults_records_unit_separated() {
  local data_file="$1"
  local declared_record_count raw_records line
  declared_record_count="$(defaults_records_declared_count "$data_file")" || return 2
  raw_records="$(defaults_records_raw_stream "$data_file")" || return 2
  defaults_records_validate_stream "$data_file" "$declared_record_count" "$raw_records" || return 2
  defaults_records_declare_a_value "$data_file" || return 2
  defaults_records_declare_agreeing_field_types "$data_file" || return 2
  defaults_records_match_declared_tier "$data_file" || return 2
  require_data_file_killall_is_iterable "$data_file" || return 2
  while IFS= read -r line; do
    [[ -z $line ]] && continue
    printf '%s\n' "$line"
  done <<<"$raw_records"
}

validate_record_scope() {
  local scope="$1" host="$2" plist_path="$3"
  case "$scope" in
    user | system) ;;
    *)
      printf 'error: unknown scope %q (expected user or system)\n' "$scope" >&2
      return 1
      ;;
  esac
  if [[ $scope == system && -n $host ]]; then
    printf 'error: scope system cannot be combined with host %q; ByHost storage is per-user\n' "$host" >&2
    return 1
  fi
  if [[ $scope == user && -n $plist_path ]]; then
    printf 'error: plist_path %q is only honored on scope system records\n' "$plist_path" >&2
    return 1
  fi
  printf '%s\n' "$scope"
}

validate_record_identity() {
  local domain="$1" key="$2"
  if [[ -z $domain ]]; then
    printf 'error: record with key %q has a blank domain; give it a value or remove the field\n' \
      "$key" >&2
    return 1
  fi
  if [[ -z $key ]]; then
    printf 'error: record %q has a blank key; give it a value or remove the field\n' \
      "$domain" >&2
    return 1
  fi
}

MACOS_DEFAULTS_SUPPORTED_TYPES=("array" "bool" "data" "date" "dict" "float" "int" "string")

validate_record_type() {
  local value_type="$1" domain="$2" key="$3" supported_type
  for supported_type in "${MACOS_DEFAULTS_SUPPORTED_TYPES[@]}"; do
    if [[ $value_type == "$supported_type" ]]; then
      return 0
    fi
  done
  printf 'error: unsupported type %q on record %s %s; expected one of %s\n' \
    "$value_type" "$domain" "$key" "${MACOS_DEFAULTS_SUPPORTED_TYPES[*]}" >&2
  return 1
}

print_offending_record_reference() {
  local domain=$1 key=$2
  printf 'error: the refusal above is on record (domain %s, key %s)\n' "$domain" "$key" >&2
}

validate_system_domain() {
  local domain="$1"
  if [[ $domain == */* ]]; then
    printf 'error: system-scope domain %q contains a slash; it would escape %s\n' \
      "$domain" '/Library/Preferences' >&2
    return 1
  fi
  if [[ -z ${domain//./} ]]; then
    printf 'error: system-scope domain %q is empty or nothing but dots; it names no plist\n' \
      "$domain" >&2
    return 1
  fi
}

validate_explicit_plist_path() {
  local plist_path="$1" domain="$2"
  if [[ $plist_path == / ]]; then
    printf 'error: plist_path %q (domain %s) is the filesystem root; it names no plist\n' \
      "$plist_path" "$domain" >&2
    return 1
  fi
  if [[ $plist_path != /* ]]; then
    printf 'error: relative plist_path %q (domain %s); an absolute path is required\n' \
      "$plist_path" "$domain" >&2
    return 1
  fi
  if [[ $plist_path == *"/../"* || $plist_path == *"/.." ]]; then
    printf 'error: plist_path %q (domain %s) contains a parent-directory component\n' \
      "$plist_path" "$domain" >&2
    return 1
  fi
}

resolve_system_plist_path() {
  local domain="$1" plist_path="$2"
  validate_system_domain "$domain" || return 1
  if [[ -z $plist_path ]]; then
    printf '/Library/Preferences/%s\n' "$domain"
    return 0
  fi
  validate_explicit_plist_path "$plist_path" "$domain" || return 1
  printf '%s\n' "$plist_path"
}

MACOS_DEFAULTS_PLIST_PATH_ALLOWED_DIRECTORIES=("/Library/Objective-See/LuLu/" "/Library/Preferences/")

require_system_plist_path_permitted() {
  local plist_path="$1" allowed_directory
  for allowed_directory in "${MACOS_DEFAULTS_PLIST_PATH_ALLOWED_DIRECTORIES[@]}"; do
    if [[ $plist_path == "$allowed_directory"* ]]; then
      return 0
    fi
  done
  printf 'error: plist_path %q is outside every permitted plist directory (%s); grant the directory deliberately in BOTH the Tier 1 template and defaults-records.sh, or use the default /Library/Preferences form\n' \
    "$plist_path" "${MACOS_DEFAULTS_PLIST_PATH_ALLOWED_DIRECTORIES[*]}" >&2
  return 1
}

validate_defaults_record() {
  local domain="$1" key="$2" value_type="$3" host="$5"
  local scope="$6" plist_path="$7" tier="$8"
  validate_record_identity "$domain" "$key" || return 1
  case "$tier" in
    manual) return 0 ;;
    enforce | verify) ;;
    *)
      printf 'error: record %s %s has an unrecognized tier %q; declare tier: enforce, verify, or manual\n' \
        "$domain" "$key" "$tier" >&2
      return 1
      ;;
  esac
  validate_record_type "$value_type" "$domain" "$key" || return 1
  if ! validate_record_scope "$scope" "$host" "$plist_path" >/dev/null; then
    print_offending_record_reference "$domain" "$key"
    return 1
  fi
  if [[ $scope == system ]]; then
    if ! resolve_system_plist_path "$domain" "$plist_path" >/dev/null; then
      print_offending_record_reference "$domain" "$key"
      return 1
    fi
  fi
}

SYSTEM_READ_OK=0
SYSTEM_READ_UNSET=1
SYSTEM_READ_UNREADABLE=2

system_defaults_write() {
  local plist_path="$1" key="$2" value_type="$3" value="$4"
  local write_status=0 written_file="$plist_path"
  [[ $written_file == *.plist ]] || written_file="$written_file.plist"
  sudo defaults write "$plist_path" "$key" "-$value_type" "$value" || write_status=$?
  if [[ $write_status -eq 0 || -e $written_file ]]; then
    sudo chown root:wheel "$written_file"
    sudo chmod 644 "$written_file"
  fi
  return "$write_status"
}

system_defaults_read_actual() {
  local plist_path="$1" key="$2"
  local file_candidate
  for file_candidate in "$plist_path" "$plist_path.plist"; do
    if [[ -e $file_candidate && ! -r $file_candidate ]]; then
      return "$SYSTEM_READ_UNREADABLE"
    fi
  done
  local value read_error_file read_status=0
  if ! read_error_file="$(mktemp)"; then
    printf 'error: cannot classify the system read of %s %s; mktemp failed\n' \
      "$plist_path" "$key" >&2
    return "$SYSTEM_READ_UNREADABLE"
  fi
  value="$(defaults read "$plist_path" "$key" 2>"$read_error_file")" || read_status=$?
  if [[ $read_status -eq 0 ]]; then
    rm -f "$read_error_file"
    printf '%s' "$value"
    return "$SYSTEM_READ_OK"
  fi
  if grep -q 'does not exist' "$read_error_file"; then
    rm -f "$read_error_file"
    return "$SYSTEM_READ_UNSET"
  fi
  rm -f "$read_error_file"
  return "$SYSTEM_READ_UNREADABLE"
}
