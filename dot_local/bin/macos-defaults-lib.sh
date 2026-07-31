# shellcheck shell=bash
# macos-defaults-lib.sh, shared helpers for the macos-defaults-{apply,capture,
# drift} tools. Sourced, never executed, so it carries no shebang and no
# executable bit.
#
# Each tool sources it as
#   source "$(dirname "${BASH_SOURCE[0]}")/macos-defaults-lib.sh"
# which resolves in BOTH the chezmoi source tree (dot_local/bin/) and the applied
# ~/.local/bin/ layout: this file carries no executable_ or dot_ prefix, so chezmoi
# deploys it under the same basename its siblings are deployed beside.

# resolve_source_dir, print the chezmoi source directory for the CURRENT context.
#
# Resolution order, most specific first:
#   1. $MACOS_DEFAULTS_SOURCE_DIR when SET, an explicit caller override. Set but
#      empty is a caller error, not "unset", so it is rejected rather than skipped.
#   2. The chezmoi source tree containing the current directory, so a run from a
#      secondary worktree targets THAT worktree rather than the primary checkout.
#      It is routed through `chezmoi --source=<top> source-path` so chezmoi
#      normalizes the path.
#   3. Otherwise chezmoi's configured source directory.
#
# Every failure returns nonzero with a message rather than falling through to the
# next rule: falling back after a failed chezmoi call would silently retarget a
# different checkout, which is the class of bug this resolver exists to end.
#
# Two rules keep that promise, and both close a way an earlier version broke it:
#
#   The source tree is identified by its .chezmoiversion marker, NOT by the data
#   file. Those are different questions. A source tree whose macos_defaults.yaml is
#   absent is STILL this tree, and must report a missing data file for the tree the
#   caller is standing in. Keying on the data file made an absent file look like
#   "some unrelated directory" and silently resolved whichever other checkout did
#   have one, reintroducing the exact bug this resolver exists to end.
#
#   The worktree is resolved with git's context variables SCRUBBED. `git rev-parse`
#   honors $GIT_DIR and $GIT_WORK_TREE, so an exported value from a git hook or a
#   wrapper made the resolver describe a checkout the caller was not in. Unsetting
#   them inside the command substitution binds the answer to the physical directory.
#
# Residual, stated rather than hidden: if git fails for a reason OTHER than an
# inherited context variable, a corrupt repository being the realistic one, the
# tree cannot be identified and resolution falls to chezmoi's configured source.
# That case is not reproduced here and is left as the documented limit.
resolve_source_dir() {
  if [[ -n ${MACOS_DEFAULTS_SOURCE_DIR+x} ]]; then
    if [[ -z $MACOS_DEFAULTS_SOURCE_DIR ]]; then
      printf 'error: MACOS_DEFAULTS_SOURCE_DIR is set but empty; refusing to resolve another checkout\n' >&2
      return 1
    fi
    printf '%s\n' "$MACOS_DEFAULTS_SOURCE_DIR"
    return 0
  fi

  local worktree_top resolved
  worktree_top="$(
    unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE
    git rev-parse --show-toplevel 2>/dev/null
  )"
  if [[ -n $worktree_top && -f "$worktree_top/.chezmoiversion" ]]; then
    if ! resolved="$(chezmoi --source="$worktree_top" source-path)"; then
      printf 'error: chezmoi --source=%s source-path failed; refusing to fall back to another checkout\n' \
        "$worktree_top" >&2
      return 1
    fi
    printf '%s\n' "$resolved"
    return 0
  fi

  if ! resolved="$(chezmoi source-path)"; then
    printf 'error: chezmoi source-path failed; the chezmoi source directory is unknown\n' >&2
    return 1
  fi
  printf '%s\n' "$resolved"
}

# macos_defaults_data_file, print the resolved path to macos_defaults.yaml.
# Returns 2, the tools' shared "data file missing or unreadable" status, when the
# source directory cannot be resolved. An empty resolution is a failure too: it
# would otherwise compose into a plausible-looking /.chezmoidata/... path.
macos_defaults_data_file() {
  local source_dir
  source_dir="$(resolve_source_dir)" || return 2
  if [[ -z $source_dir ]]; then
    printf 'error: resolved an empty chezmoi source directory for macos_defaults.yaml\n' >&2
    return 2
  fi
  printf '%s/.chezmoidata/macos_defaults.yaml\n' "$source_dir"
}

# require_readable_data_file <path>, the shared readable-data-file guard. Returns
# 2 with a message naming the file, so the caller's exit status matches the "data
# file missing or unreadable" contract documented in each tool's header.
require_readable_data_file() { # <path>
  local data_file="$1"
  if [[ ! -r $data_file ]]; then
    printf 'error: cannot read %s\n' "$data_file" >&2
    return 2
  fi
}

# defaults_records_join_expression <record-selector>, the yq expression that
# joins one record selection into EIGHT fields separated by the ASCII unit
# separator (0x1f): domain, key, type, value, host, scope, plist_path, tier.
# Shared by the stream below and by the per-record locator, so the two can
# never drift into describing different records.
#
# type, value, and tier are BARE selectors, deliberately. join renders a null
# (absent on a manual-tier record) as the empty string already, and the
# tempting explicit spelling, `.value // ""`, is WRONG here: yq's alternative
# operator fires on false as well as null, so a legitimate `value: false`
# (most of the tracked records) would collapse to an empty write. The
# defaulted fields below are safe with // only because none of them can hold
# a legitimate false.
defaults_records_join_expression() { # <record-selector>
  local unit_separator=$'\x1f'
  printf '%s | [.domain, .key, .type, .value, (.host // ""), (.scope // "user"), (.plist_path // ""), .tier] | join("%s")' \
    "$1" "$unit_separator"
}

# defaults_records_field_count <line>, how many unit-separated fields one line
# carries. Pure bash: the substitution keeps only the separators, so the field
# count is one more than what is left.
defaults_records_field_count() { # <line>
  local unit_separator=$'\x1f'
  local separators_only="${1//[!$unit_separator]/}"
  printf '%s' "$((${#separators_only} + 1))"
}

# defaults_records_locate_malformed <path> <declared-count>, describe the FIRST
# record that does not render as exactly one eight-field line. Only ever called
# on the failure path, so its per-record yq calls cost nothing in the normal case.
defaults_records_locate_malformed() { # <path> <declared-count>
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

# THE RECORD STREAM. Four functions below share this contract, split so each has
# one job: defaults_records_declared_count validates the file's shape and size,
# defaults_records_raw_stream reads it, defaults_records_validate_stream is a
# PREDICATE over what was read, and defaults_records_unit_separated emits. They
# were one function, in which the validation loop accumulated the very lines it
# was checking and then printed them, so asking "is this file usable" was
# inseparable from producing its output.
#
# The contract: emit each tracked record as one line
# of EIGHT fields joined by the ASCII unit separator (0x1f):
#   domain, key, type, value, host, scope, plist_path, tier
# host, plist_path, and (on manual records) type and value are empty when
# absent; an ABSENT scope defaults to "user" here, so a scope that reaches a
# caller empty was explicitly empty in the record (a record error, rejected by
# validate_record_scope below). The unit separator is not IFS whitespace, so an
# empty INTERIOR field survives `IFS=$'\x1f' read` intact, unlike a
# tab-separated stream, whose collapse is exactly why the optional columns do
# not extend one.
#
# The TIER is validated here, for the whole file, before a single record is
# emitted: every record must declare enforce, verify, or manual, and a missing
# or blank tier arrives as the empty string and is refused like any other
# unrecognized value. Refusing in the stream is what makes every tool
# fail-closed at once: no caller can act on a record whose tier is unknown,
# because no such record ever reaches a caller.
#
# The stream is SELF-VALIDATING, because the separator on its own guarantees
# nothing. A field value carrying a literal 0x1f byte, a NEWLINE, or both is not
# a formatting nuisance, it is record forgery. One record whose value was
#   v<0x1f><0x1f>system<0x1f><0x1f>enforce<0x1f>\nEVIL.DOMAIN<0x1f>EVILKEY<0x1f>bool<0x1f>true
# emitted two well-formed-LOOKING lines and made apply perform TWO root writes,
# the second fully attacker-controlled, while the template rendered only one.
# Both halves of that payload carry exactly eight fields, so a per-line field
# count does not catch it on its own. Two checks together do:
#   - every line must carry exactly eight fields, which catches a separator
#     injected without a newline;
#   - the number of emitted lines must equal the number of DECLARED records,
#     which catches a newline whether or not the halves are balanced.
# A violation returns 2, the tools' shared "data file unusable" status, names the
# offending record, and emits NOTHING: a caller must not act on part of a stream
# it has just been told is malformed. A legitimate multi-line preference value is
# therefore refused loudly rather than silently corrupted.
# The yq answers this reader keys on. They are protocol with another program,
# not free text, which is why they are named here rather than spelled inline at
# the one site that compares them: a yq release that renamed a node kind would
# be a one-line change, and the name says which question is being asked.
#
# yq answers TWO questions about a node and NEITHER one answers this guard's
# question alone. `kind` reports what the node IS after parsing; `tag` reports
# its REPRESENTATION, which the document author writes. Each is defeated by
# exactly the input the other catches, and both halves were measured against
# yq v4.53.3 and chezmoi v2.71.1 rather than reasoned about:
#
#   A LYING TAG on a wrong-shaped node defeats `tag` alone. `!!seq` on a map
#   leaves the node a map (kind map, tag !!seq) while a tag check reads !!seq
#   and calls it a list.
#
#   A TRUTHFUL SHAPE wearing a wrong tag defeats `kind` alone. `!!str` on a
#   real sequence leaves the node a sequence (kind seq, tag !!str) while a kind
#   check reads seq and calls it a list. The runner template's Go YAML reader
#   REFUSES that file with a parse error, so a kind-only check made this library
#   the MORE PERMISSIVE of the two readers on eight measured tags
#   (!!map !!str !!int !!bool !!float !!binary !!set !!timestamp), which is the
#   exact asymmetry test/integration/macos-defaults-shape-agreement.sh exists to
#   end and the mirror image of the hole a tag-only check left.
#
# So the accepted answer is the CONJUNCTION, and it is expressed as one: the
# node must be a sequence AND its tag must say so. Every other pairing of the
# two answers is refused. That closes the tag space by construction rather than
# by listing the tags anybody happened to try, which matters because the tag
# space is open: a document may write any application-specific tag it likes.
#
# The measured residual, stated rather than hidden. Six tag spellings on a real
# sequence (!!omap, !!pairs, !!merge, a local !custom, an unknown !!foo, and a
# verbatim URI tag) are RENDERED by the runner template and REFUSED here, so on
# those this library is the STRICTER reader. That direction is a loud refusal
# naming the file, never a silent misapplication, and buying agreement on them
# would mean transcribing which tags one Go YAML release happens to decode as a
# slice, which the next release could change without notice. Refusing what
# cannot be proven common to both readers is the fail-closed choice.
#
# Plain assignment, not readonly, for the same reason as SYSTEM_READ_* further
# down: this file is a library, and sourcing it twice must be a no-op.
DEFAULTS_RECORDS_LIST_KIND='seq'
DEFAULTS_RECORDS_LIST_TAG='!!seq'
DEFAULTS_RECORDS_MAP_KIND='map'
DEFAULTS_RECORDS_ABSENT_TAG='!!null'

# The two yq expressions this reader asks the data file, named so the pair
# always describes the SAME node, and the shape one kept beside the classifier
# that parses it so their separator cannot drift apart.
#
# Neither carries a `// []` fallback, and that absence is the point. The
# fallback substituted an empty list for a MISSING node, which made "this file
# declares no record list" and "this file declares an empty one" the same
# answer: a clean count of zero and an exit status of 0. One of those states is
# an operator saying they track nothing; the rest are a file that lost its
# records while the tools applied nothing and reported success. Without the
# fallback a missing node arrives as `!!null` and gets its own verdict.
DEFAULTS_RECORDS_SHAPE_EXPRESSION='.macos.defaults | [kind, tag] | join(" ")'
DEFAULTS_RECORDS_COUNT_EXPRESSION='.macos.defaults | length'

# records_declaration_verdict <shape-answer>, classify what the data file
# declares at .macos.defaults. PURE: one string in, one verdict out, no file
# access and no globals beyond the four yq answers named above.
#
#   list       a plain YAML sequence: kind seq AND tag !!seq. The whole accept
#              set, and the only accepted verdict. An untagged sequence, an
#              explicitly `!!seq`-tagged one, and one wearing the non-specific
#              `!` tag all answer exactly this pair (measured), so the
#              conjunction admits every legitimate spelling and nothing else.
#   mistagged  a real sequence carrying any OTHER tag. Its own verdict rather
#              than a fold into "other", because the operator's fix is specific
#              (delete the tag) and differs from every other refusal here.
#   map        a mapping, in any spelling, including one wearing a `!!seq` tag.
#   absent     no node at all, or an explicitly null one. yq answers `!!null` to
#              both, and they mean the same thing to an operator: the file
#              declares no record list, and `defaults: []` declares an empty one.
#   other      anything else, including a scalar, an empty answer (yq found no
#              node to describe, which happens when `.macos` is not a mapping),
#              and an answer yq spread over several lines (one per document in a
#              multi-document file). Unrecognized resolves to a REFUSED verdict,
#              never an accepted one, so a shape nobody anticipated cannot arrive
#              as "list".
records_declaration_verdict() { # <shape-answer>
  local shape_answer="$1" node_kind node_tag
  # Exactly one line of two space-separated fields is the only answer this
  # classifier can read. Anything else, including yq's per-document answers
  # separated by `---`, falls through to "other" rather than letting the first
  # document answer for the whole file.
  if [[ ! $shape_answer =~ ^([[:alpha:]]+)' '([^[:space:]]+)$ ]]; then
    printf 'other\n'
    return 0
  fi
  node_kind="${BASH_REMATCH[1]}"
  node_tag="${BASH_REMATCH[2]}"
  case $node_kind in
    "$DEFAULTS_RECORDS_LIST_KIND")
      # BOTH answers, or neither. A sequence whose tag names something else is
      # still refused, because the runner template refuses several such files
      # outright and this reader must never be the permissive one.
      if [[ $node_tag == "$DEFAULTS_RECORDS_LIST_TAG" ]]; then
        printf 'list\n'
      else
        printf 'mistagged\n'
      fi
      ;;
    "$DEFAULTS_RECORDS_MAP_KIND") printf 'map\n' ;;
    *)
      if [[ $node_tag == "$DEFAULTS_RECORDS_ABSENT_TAG" ]]; then
        printf 'absent\n'
      else
        printf 'other\n'
      fi
      ;;
  esac
}

# A UTF-8 byte order mark, and its length in BYTES. The byte count is a named
# constant rather than `${#UTF8_BYTE_ORDER_MARK}` because that expansion counts
# CHARACTERS: the mark is three bytes but ONE character under a UTF-8 locale
# (measured), so the obvious spelling would compare a single byte and call any
# file starting with 0xEF a marked one.
UTF8_BYTE_ORDER_MARK=$'\xef\xbb\xbf'
UTF8_BYTE_ORDER_MARK_BYTE_COUNT=3

# data_file_begins_with_byte_order_mark <path>, 0 when the file's first bytes
# are a UTF-8 byte order mark, 1 otherwise. One input, one boolean answer, no
# parsing.
#
# The mark is REFUSED by the caller rather than stripped, for two reasons that
# are about the file's two readers rather than about tidiness. yq v4.53.3 strips
# a document-start mark and reads the file normally; chezmoi v2.71.1, whose Go
# YAML reader the runner template goes through, does NOT, and dies with `map has
# no entry for key "macos"` (both measured). Stripping here would leave this
# library happily reading a file that `chezmoi apply` refuses, which is the
# asymmetry this guard exists to close. And a mark is legitimate content
# anywhere but the first three bytes: one inside a record value survives into
# that value, so there is no safe general strip to fall back on.
#
# WHY THE FIRST THREE BYTES ARE THE WHOLE OF THIS GUARD'S JOB, since "byte 0
# only" reads like an arbitrary narrowing and is not one. The question this
# predicate answers is not "does the file contain a mark", it is "is there a
# mark the two readers TREAT DIFFERENTLY", and byte 0 is the only position where
# they do. Measured on this yq and this chezmoi, dumping each reader's key list
# rather than inferring it from whether the file was accepted:
#
#   at byte 0            yq strips it and answers the key `macos`; chezmoi keeps
#                        it and cannot find `macos` at all. DIVERGENT, refused
#                        here.
#   anywhere else        NEITHER reader strips it. A mark before a key binds
#                        into that key in BOTH, so yq answers the key
#                        "<mark>killall" exactly as chezmoi does. The readers
#                        AGREE, so there is nothing for this guard to close, and
#                        the file is judged on the keys it actually has: a mark
#                        that lands on `defaults` makes the record list absent
#                        and is refused by the declaration verdict below, and
#                        one inside a record value is legitimate content and is
#                        preserved byte for byte.
#
# test/unit/macos-defaults-declaration-guard.sh pins that table from the reader
# side, so a future yq that starts stripping a mid-file mark reopens a real
# divergence and fails there instead of passing silently here.
#
# The limit this predicate does NOT cover, stated because it is adjacent enough
# to look covered: it reads only `.macos.defaults`, so a mark bound into a
# DIFFERENT key the runner template needs (`.macos.killall`) leaves this reader
# accepting a file chezmoi refuses. That gap is not about marks at all, it is
# the general one that any malformed sibling key produces, it predates this
# guard, and it belongs to the two readers' disagreement over schema-invalid
# shapes rather than here.
#
# An unreadable or absent file answers 1 here, the same as a clean one, because
# a predicate that cannot read the bytes cannot claim to have found a mark. That
# answer is NOT this guard's last word on such a file: measured on a mode-000
# file and on a missing path, the caller's next step (the yq shape read) fails
# and the file is refused with status 2 and a message naming it. The read's own
# diagnostic is discarded rather than relayed, because it would name `head` for
# a failure the caller is about to report against yq.
data_file_begins_with_byte_order_mark() { # <path>
  local leading_bytes
  leading_bytes="$(LC_ALL=C head -c "$UTF8_BYTE_ORDER_MARK_BYTE_COUNT" -- "$1" 2>/dev/null)" || return 1
  [[ $leading_bytes == "$UTF8_BYTE_ORDER_MARK" ]]
}

# declared_record_count_is_usable <count>, a PURE predicate: 0 when yq's answer
# is a single plain non-negative integer this library can safely do arithmetic
# on, 1 otherwise.
#
# The count drives both `((...))` loop bounds and the `-ne` comparison that
# catches a forged record, so three separate shapes have to be refused, and the
# comment matters because only one of them is reachable from today's producer.
#
# A NON-NUMERIC count and a LEADING ZERO (which bash reads as octal: $((0010))
# is 8) are defence in depth. `length` on a sequence answers with an integer
# node, and this predicate now runs only after the shape verdict is `list`, so
# neither shape reaches it from this producer.
#
# What a MULTI-LINE count would do if it got past here is worth recording
# correctly, because the obvious guess is wrong and an earlier version of this
# comment asserted it: bash raises no syntax error. It reads the whole value as
# ONE arithmetic expression and the `---` separator as a run of unary minus
# signs, so $'1\n---\n0' evaluates to 1 and $'0\n---\n9' to -9 (measured, bash
# 5.3.15). The `-ne` comparison would then answer on the DIFFERENCE of the
# per-document counts, silently, and on $'1\n---\n0' it answers "no mismatch"
# and emits the very stream it exists to reject. The shape check refuses a
# multi-document file first, so this predicate is the second barrier there, not
# the first.
#
# The SEVEN-DIGIT CEILING bounds a count no other check bounds. It is defence in
# depth against this producer as well, and honestly so: a sequence's `length` is
# its element count, so reaching eight digits needs ten million real records.
# The route that made it cheap is closed, and closing it is why the ceiling now
# has to be pinned by calling this predicate directly. An explicit `!!seq` on a
# SCALAR used to satisfy the old tag-based shape check while yq's `length`
# reported the scalar's length in BYTES, so a 10 MB file published a count of
# 10000000 (measured, yq v4.53.3) and a 1 GB file a billion. That count is the
# upper bound of the `defaults_records_locate_malformed` loop, which forks yq
# per iteration (measured at about 11 ms each).
#
# Bounding by DIGIT COUNT rather than by comparison is deliberate: it refuses an
# oversized value BEFORE any arithmetic touches it. A comparison has to evaluate
# the value in order to reject it, and bash's integer arithmetic WRAPS without
# complaint, so `^[0-9]+$` plus `-gt` ACCEPTS 18446744073709551616, which
# evaluates to 0 (measured). The bound stays in the quantifier rather than in a
# named constant because `^(0|[1-9][0-9]{0,N})$` is the form four other guards
# in this repo already use, and naming it at this one site alone would leave
# five spellings of one idiom.
#
# test/unit/macos-defaults-count-guard.sh pins both boundaries of the ceiling.
declared_record_count_is_usable() { # <count>
  [[ $1 =~ ^(0|[1-9][0-9]{0,6})$ ]]
}

defaults_records_declared_count() { # <path>, print the validated record count
  local data_file="$1"
  local shape_answer declaration_verdict declared_record_count
  # The BYTES, before the parse. A document-start byte order mark is the one
  # defect in this file that neither reader reports usefully: yq strips it and
  # answers as if the file were clean, while the runner template's Go YAML
  # reader keeps it bound into the first key and dies naming a key the operator
  # can see perfectly well in their editor. Three invisible bytes are not
  # something to leave an operator to find, and a reader that accepts a file its
  # sibling refuses is the asymmetry this whole guard exists to close.
  if data_file_begins_with_byte_order_mark "$data_file"; then
    printf 'error: %s begins with a UTF-8 byte order mark; yq strips it and reads the file, but the runner template does not and cannot then find .macos at all, so the two readers disagree about this file; remove the first three bytes\n' \
      "$data_file" >&2
    return 2
  fi
  # The SHAPE, before the count. `.macos.defaults[]` yields values from a map
  # just as happily as from a list, and `length` counts a map's keys, so a map
  # answers the count question without complaint and the stream comes out in
  # document order. The runner template reads the same file with Go's `range`,
  # which iterates a map in sorted KEY order. Two readers, two orders, neither
  # complaining, and order decides which write lands last when records share a
  # domain and key.
  #
  # Refused rather than reconciled: a map is not the declared schema, and
  # standardizing on one order would leave the other reader's agreement a
  # coincidence instead of a guarantee. The template refuses the same shape.
  #
  # Asking the shape FIRST is what lets the count check stop carrying shapes it
  # was never the right guard for. A count is only meaningful once the node is
  # known to be a list, and "declares .macos.defaults as a scalar" is a better
  # thing to tell an operator than "unusable record count 10000000".
  if ! shape_answer="$(yq eval -r "$DEFAULTS_RECORDS_SHAPE_EXPRESSION" "$data_file")"; then
    printf 'error: cannot determine the shape of .macos.defaults in %s\n' "$data_file" >&2
    return 2
  fi
  declaration_verdict="$(records_declaration_verdict "$shape_answer")"
  case $declaration_verdict in
    list) ;;
    mistagged)
      printf 'error: %s tags .macos.defaults as %q; the record list is a real sequence, so only its TAG is wrong, and the only tag accepted on it is %s, because the runner template refuses several of the others with a parse error while this reader would take the records, so the two readers would disagree about whether this file has any settings at all; delete the tag\n' \
        "$data_file" "${shape_answer#* }" "$DEFAULTS_RECORDS_LIST_TAG" >&2
      return 2
      ;;
    map)
      printf 'error: %s declares .macos.defaults as a map, but it must be a LIST of records; a map is read in sorted key order by the runner template and in document order here, so the two would apply records in different orders\n' \
        "$data_file" >&2
      return 2
      ;;
    absent)
      printf 'error: %s declares no .macos.defaults record list, so every tracked setting would be silently skipped and the run would still report success; to track no records, declare an explicitly empty list, defaults: []\n' \
        "$data_file" >&2
      return 2
      ;;
    *)
      printf 'error: %s does not declare .macos.defaults as a LIST of records; yq answered %q for its kind and tag\n' \
        "$data_file" "$shape_answer" >&2
      return 2
      ;;
  esac
  if ! declared_record_count="$(yq eval -r "$DEFAULTS_RECORDS_COUNT_EXPRESSION" "$data_file")"; then
    printf 'error: cannot count the records in %s\n' "$data_file" >&2
    return 2
  fi
  if ! declared_record_count_is_usable "$declared_record_count"; then
    printf 'error: %s produced an unusable record count %q; refusing to emit a stream that cannot be checked\n' \
      "$data_file" "$declared_record_count" >&2
    return 2
  fi
  printf '%s\n' "$declared_record_count"
}

# defaults_records_raw_stream <path>, the unvalidated joined records from yq.
# Separated so the reader can fail on its own terms; every check that follows
# assumes it has the whole stream, and a partial read must never reach them.
defaults_records_raw_stream() { # <path>
  local data_file="$1"
  if ! yq eval -r "$(defaults_records_join_expression '.macos.defaults[]')" "$data_file"; then
    printf 'error: cannot read the records in %s\n' "$data_file" >&2
    return 2
  fi
}

# defaults_records_validate_stream <path> <declared-count> <raw-stream>, a
# PREDICATE: 0 when every line is well-formed and the line count matches what the
# file declares, 2 otherwise, naming the offending record on stderr.
#
# It emits no records, deliberately. Validation used to accumulate the very lines
# it was checking and then print them, which meant the only way to ask "is this
# file usable" was to also produce its output. A caller that just wants to know
# now asks a question instead of running a producer, and the emission below has
# one job.
defaults_records_validate_stream() { # <path> <declared-count> <raw-stream>
  local data_file="$1" declared_record_count="$2" raw_records="$3"
  local line field_count checked_line_count=0
  local record_domain record_key record_type record_value
  local record_host record_scope record_plist_path record_tier
  while IFS= read -r line; do
    # yq prints a single empty line for an empty array; that is not a record.
    [[ -z $line ]] && continue
    field_count="$(defaults_records_field_count "$line")"
    if [[ $field_count -ne 8 ]]; then
      printf 'error: %s: %s renders %s fields, not 8; a field value contains a unit separator (0x1f) or a newline\n' \
        "$data_file" "$(defaults_records_locate_malformed "$data_file" "$declared_record_count")" \
        "$field_count" >&2
      return 2
    fi
    # The tier gate. Only the three declared tiers pass; a record whose tier
    # is missing or blank arrives here as the empty string and lands in the
    # same refusal, so absent, blank, and unrecognized all fail closed. The
    # refusal covers the WHOLE file, because a caller must not act on the
    # records beside one it cannot classify.
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
    # The rest of the record's rules, for the WHOLE file, before the emission
    # below prints a single line. This is the ordering the runner template has
    # always had (a complete validation pass, then a render) and the ordering the
    # tools lacked: apply validated scope and plist_path inside its own write
    # loop, so a file whose second record was malformed had its first record
    # written before anyone noticed, and a declarative settings file was left
    # half applied.
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

# defaults_records_declare_a_value <path>, a PREDICATE over the FILE: 0 when
# every enforce and verify record DECLARES a value, 2 otherwise, naming the first
# record that does not.
#
# Asked of the file rather than of the joined record line, because the line
# cannot answer it. join renders an ABSENT value and an explicitly EMPTY one
# identically, as the empty string, and the two are not the same record error:
#   value: ""   is a legitimate empty string, which the runner template renders
#               and which a `type: string` record may genuinely want;
#   value:      (or no value key at all) is a record that names no value, and it
#               reached `defaults write <domain> <key> -bool ''` here while the
#               template refused the same record outright.
# Reading presence from the file is what lets this reader refuse exactly what the
# template refuses instead of narrowing to "empty is fatal", which would refuse a
# file the template renders happily and teach the operator about it through a
# broken drift report.
#
# Only enforce and verify records are asked: a manual record carries a runbook
# pointer and no write payload, and the template forbids it a value entirely.
defaults_records_declare_a_value() { # <path>
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

defaults_records_unit_separated() { # <path>
  local data_file="$1"
  local declared_record_count raw_records line
  declared_record_count="$(defaults_records_declared_count "$data_file")" || return 2
  raw_records="$(defaults_records_raw_stream "$data_file")" || return 2
  defaults_records_validate_stream "$data_file" "$declared_record_count" "$raw_records" || return 2
  defaults_records_declare_a_value "$data_file" || return 2
  # Emission, and only emission, and only once the WHOLE file has passed. A
  # caller must never act on part of a stream it is about to be told is
  # malformed, which is why nothing is printed before the predicate returns.
  while IFS= read -r line; do
    [[ -z $line ]] && continue
    printf '%s\n' "$line"
  done <<<"$raw_records"
}

# validate_record_scope <scope> <host> <plist_path>, print the validated scope.
# Rejects, with a message and nonzero status, every combination that would
# otherwise be silently misapplied:
#   - a scope other than user/system, including the set-but-empty scope ""
#     (defaults_records_unit_separated already turned an ABSENT field into
#     "user", so an empty scope here was explicitly empty in the record);
#   - scope system with a host: ByHost storage is per-user, the pair is
#     meaningless;
#   - scope user with a plist_path: the path is only honored on system
#     records, and accepting it would silently write the user domain instead
#     of the named file.
validate_record_scope() { # <scope> <host> <plist_path>
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

# validate_record_identity <domain> <key>, a PREDICATE over the two fields that
# NAME a record: 0 when both carry text, 1 with a message otherwise. It applies
# to every record whatever its tier, because a record with no domain or no key
# names no control that any tool could act on or report.
#
# Refused, never skipped. apply and drift both used to `continue` past a record
# with an empty domain, which turned a malformed record into a silent no-op: a
# file whose second record had no domain applied its first record and exited 0,
# reporting success for a file it had only partly applied. The runner template
# refuses the same record outright.
validate_record_identity() { # <domain> <key>
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

# The value types a record may declare. The SAME closed set the Tier 1 runner
# template constrains .type to; test/integration/macos-defaults-validate-before-write.sh
# pins the two lists identical, the way the system-scope suite pins the two
# plist_path allowlists.
#
# Closed rather than free-form because the type is the one field BOTH readers put
# into a command as a BARE option word (-bool, -int): the template renders it
# into shell source unquoted, and the tools pass it to `defaults` as "-$type".
# Quoting is not available (`defaults` needs a bare option word), so constraining
# the set is what stands in for it. Plain assignment, not readonly, for the same
# re-source reason as the read-status constants below.
MACOS_DEFAULTS_SUPPORTED_TYPES=("array" "bool" "data" "date" "dict" "float" "int" "string")

# validate_record_type <type> <domain> <key>, a PREDICATE over the declared type:
# 0 when it names a supported type, 1 with a message otherwise. A blank type
# lands in the same refusal as an unrecognized one; both name no type, and a
# blank one reached `defaults write <domain> <key> - <value>` before this existed.
validate_record_type() { # <type> <domain> <key>
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

# print_offending_record_reference <domain> <key>, name the record a refusal
# belongs to. The scope and plist_path predicates judge ONE field each and are
# pure functions of it, so they cannot name the record the field came from;
# without this, a refusal reads "unknown scope bogus" and the operator has
# nothing to search the data file for. The predicates that already name the
# record do not get a second reference.
print_offending_record_reference() { # <domain> <key>
  printf 'error: the refusal above is on record (domain %s, key %s)\n' "$1" "$2" >&2
}

# resolve_system_plist_path <domain> <plist_path>, print the plist path a
# system-scope record writes to and reads from. An empty declared path means
# the default, /Library/Preferences/<domain>. A declared path must be
# ABSOLUTE: a relative path would resolve against whatever directory the tool
# happens to run from, so it is rejected, never resolved.
# validate_system_domain <domain>, a PREDICATE over the domain alone: 0 when it
# can name a plist under /Library/Preferences, 1 with a message otherwise.
#
# Separated from resolution because the two rules below are properties of the
# RECORD, true or false before anything is resolved, and they apply to every
# system-scope record whether or not it declares an explicit plist_path. Folded
# into the resolver they sat above an early return, which is the shape that once
# let them apply to only one of the two branches.
validate_system_domain() { # <domain>
  local domain="$1"
  # A defaults domain is reverse-DNS and never legitimately contains a slash.
  # Rejecting one keeps the default construction inside /Library/Preferences BY
  # CONSTRUCTION: without it, a domain of ../../tmp/owned resolves to
  # /Library/Preferences/../../tmp/owned, which is /tmp/owned, written as root.
  #
  # Checked for EVERY system-scope record, not only the ones that omit
  # plist_path. The domain rule is a property of the record, and the template
  # rejects such a record outright: a library that only checked it on the
  # default-path branch was the MORE PERMISSIVE of the two consumers, so the same
  # YAML rendered one way and applied another.
  if [[ $domain == */* ]]; then
    printf 'error: system-scope domain %q contains a slash; it would escape %s\n' \
      "$domain" '/Library/Preferences' >&2
    return 1
  fi
  # Degenerate domains: "", ".", "..", and any run of nothing but dots. None of
  # them escape a directory (`defaults` appends .plist), so this is hygiene
  # rather than containment, but none of them name a plist anybody meant to write
  # either, and the rule this function enforces is "does this resolve where I
  # intend to write".
  if [[ -z ${domain//./} ]]; then
    printf 'error: system-scope domain %q is empty or nothing but dots; it names no plist\n' \
      "$domain" >&2
    return 1
  fi
}

# validate_explicit_plist_path <plist_path> <domain>, a PREDICATE over a declared
# path: 0 when it names an absolute plist that cannot climb out of where it
# appears to sit, 1 with a message otherwise. The domain is carried for the
# messages only; it is validated separately, by validate_system_domain.
validate_explicit_plist_path() { # <plist_path> <domain>
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
  # Leading-slash is a PROXY for "resolves where I intend", and the two diverge
  # exactly where it matters: /Library/Preferences/../../etc/x passes the check
  # above and resolves to /etc/x. Reject parent-directory components outright
  # rather than canonicalizing, so the rejection does not depend on the path
  # existing yet.
  if [[ $plist_path == *"/../"* || $plist_path == *"/.." ]]; then
    printf 'error: plist_path %q (domain %s) contains a parent-directory component\n' \
      "$plist_path" "$domain" >&2
    return 1
  fi
}

# resolve_system_plist_path <domain> <plist_path>, print the plist a system-scope
# record writes to, or fail with a message.
#
# Resolution only. Each input is put to its own predicate first, and what remains
# here is the one decision this function actually makes: an absent plist_path
# means the default under /Library/Preferences, and a declared one is used as
# given. Previously those two lines sat among five validation blocks, with the
# default-path branch returning from the middle of them.
resolve_system_plist_path() { # <domain> <plist_path>
  local domain="$1" plist_path="$2"
  validate_system_domain "$domain" || return 1
  if [[ -z $plist_path ]]; then
    printf '/Library/Preferences/%s\n' "$domain"
    return 0
  fi
  validate_explicit_plist_path "$plist_path" "$domain" || return 1
  printf '%s\n' "$plist_path"
}

# The directories an EXPLICIT plist_path may name at WRITE time, grown
# deliberately, one product at a time. The render-time gate in
# run_onchange_after_30-macos-defaults.sh.tmpl carries the same list
# ($plistPathAllowedDirectories) and refuses the record before anything
# renders; this list closes the path AROUND the render: `just defaults-apply`
# reads the YAML directly, so without it a record the render would refuse was
# still handed to `sudo defaults write` (verified against
# /etc/example.evil.plist before the fix). The system-scope suite pins the
# two lists identical. Plain assignment, not readonly, for the same
# re-source reason as the read-status constants below.
MACOS_DEFAULTS_PLIST_PATH_ALLOWED_DIRECTORIES=("/Library/Objective-See/LuLu/" "/Library/Preferences/")

# require_system_plist_path_permitted <plist_path>, refuse (status 1, with a
# message naming the path and the rule) any WRITE target outside the
# permitted directories above. Called by apply for every system-scope record
# before anything is written. Reads are deliberately NOT gated: drift
# consulting an odd path mutates nothing, and refusing it would only hide
# the row from the report.
require_system_plist_path_permitted() { # <plist_path>
  local plist_path="$1" allowed_directory
  for allowed_directory in "${MACOS_DEFAULTS_PLIST_PATH_ALLOWED_DIRECTORIES[@]}"; do
    if [[ $plist_path == "$allowed_directory"* ]]; then
      return 0
    fi
  done
  printf 'error: plist_path %q is outside every permitted plist directory (%s); grant the directory deliberately in BOTH the Tier 1 template and macos-defaults-lib.sh, or use the default /Library/Preferences form\n' \
    "$plist_path" "${MACOS_DEFAULTS_PLIST_PATH_ALLOWED_DIRECTORIES[*]}" >&2
  return 1
}

# validate_defaults_record <domain> <key> <type> <value> <host> <scope>
# <plist_path> <tier>, THE per-record gate: 0 when the record is usable, 1 with a
# message on stderr otherwise. Its eight arguments are one record stream line, in
# stream order.
#
# A PREDICATE, and it prints nothing on stdout. That matters: the two functions
# it composes which DO print (validate_record_scope, resolve_system_plist_path)
# are called with their output discarded here, so no caller can mistake this for
# a producer and read a resolved value off a record that was just refused.
#
# Composing the record's rules in one place is what lets a caller ask the
# question ONCE PER FILE, before acting on any record. The tools used to ask it
# one record at a time inside their own consuming loops, which meant a file whose
# second record was malformed had its first record applied before the second was
# even looked at.
#
# The rules are grouped by what the DECLARED tier means, mirroring the runner
# template's validation pass:
#   - identity applies to every record; a record with no domain or no key names
#     no control, whatever its tier;
#   - enforce and verify records carry the read/write payload, so the type,
#     scope, host and plist_path rules apply to them. For enforce the payload is
#     the write; for verify it is the read the drift checker compares.
#   - manual records carry a runbook pointer and no payload, so only identity
#     applies. Their runbook rules (a runbook is REQUIRED, and no write field may
#     appear) live in the runner template alone: the runbook is not one of the
#     eight fields the record stream carries, so this gate cannot see it.
#
# Two rules are deliberately NOT here:
#   - the value rule, which needs to tell an absent value from an empty one and
#     so is asked of the FILE, by defaults_records_declare_a_value;
#   - the write-time plist_path allowlist. Reads are not gated (drift consulting
#     an odd path mutates nothing, and refusing it would hide the row instead of
#     reporting it), so that rule belongs to apply, ahead of apply's first write.
validate_defaults_record() { # <domain> <key> <type> <value> <host> <scope> <plist_path> <tier>
  # The fourth argument, the record's value, is deliberately not read: telling an
  # absent value from a legitimately empty one is impossible from the joined
  # line, so that rule is asked of the file instead. The argument stays in the
  # signature so callers pass a whole record rather than a subset of one.
  local domain="$1" key="$2" value_type="$3" host="$5"
  local scope="$6" plist_path="$7" tier="$8"
  validate_record_identity "$domain" "$key" || return 1
  case "$tier" in
    manual) return 0 ;;
    enforce | verify) ;;
    *)
      # Unreachable while the stream's tier gate holds, and fail-closed so that
      # if it ever stops holding this gate refuses rather than falling through
      # to "no payload rule applies to this tier".
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

# The three outcomes of reading a system-scope setting, carried as an exit STATUS
# rather than a marker string. A string sentinel is representable as a real value:
# a tracked setting whose live value happened to be the marker would be reported
# indeterminate, hiding both a match and a genuine drift. A status cannot be
# impersonated by any value, so the two channels stay separate.
#
# NOT readonly, deliberately. This file is a library: sourcing it twice must be a
# no-op, and `readonly` makes the second source's assignment fail. Every tool
# runs under `set -euo pipefail`, so that failure does not just skip the
# assignment, it kills the CALLER. Plain assignment also beats `readonly` on the
# other axis that matters here: it OVERWRITES an inherited value, so a hostile
# environment cannot hand the tools a different set of status codes.
SYSTEM_READ_OK=0
SYSTEM_READ_UNSET=1
SYSTEM_READ_UNREADABLE=2

# system_defaults_write <plist_path> <key> <type> <value>, one system-scope
# write. /Library plists are root-owned, so the write goes through sudo;
# keeping it here keeps apply and any future caller on one code path.
#
# After the write, the written file's ownership and mode are repaired to
# root:wheel 0644 IN THE SAME CALL: `defaults write` recreates its target as
# a root-owned 0600 binary plist (verified on a copy, 2026-07-27), and a 0600
# plist reads back SYSTEM_READ_UNREADABLE for the unprivileged drift checker
# on every later run, so an unrepaired write defeats the very drift gate that
# verifies it. The repair is PER WRITE, never a trailing cleanup in a caller:
# under set -e a failed later write ends the caller at that record, and a
# trailing cleanup would never run for the writes that DID land. The write's
# own failure is captured and re-raised AFTER the repair; a failed write that
# left no file behind skips the repair rather than failing on the missing
# path. The file repaired is the one `defaults` actually writes: the declared
# path when it already ends in .plist, the .plist beside it otherwise (an
# extensionless absolute path gets .plist appended by `defaults`, verified on
# a copy, 2026-07-27). The rendered Tier 1 runner carries the same function
# for the same reason; the render tests and the apply test pin both.
system_defaults_write() { # <plist_path> <key> <type> <value>
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

# system_defaults_read_actual <plist_path> <key>, the three-outcome system-scope
# read for drift. The outcome is the EXIT STATUS, named by the constants above,
# and only ONE of the three prints anything:
#   - SYSTEM_READ_OK (0): `defaults read` succeeded. The live value is printed on
#     stdout, with no trailing newline.
#   - SYSTEM_READ_UNSET (1): `defaults` itself reported the domain/default pair
#     does not exist, the one failure that genuinely means "not set". Prints
#     nothing.
#   - SYSTEM_READ_UNREADABLE (2): indeterminate. Returned up front when the plist
#     file exists but this user cannot read it (defaults would answer from a
#     stale cache or misreport), when the temp file that captures defaults'
#     stderr cannot be created, and for every OTHER read failure. Unknown
#     failures land here, never in unset: collapsing them would report drift
#     against a value nobody read, and skipping them would hide the record
#     entirely. Prints nothing.
# Turning the outcome into a display marker is the caller's job; that is the
# whole point of using a status. Documented limit: a plist whose PARENT directory
# blocks traversal cannot be file-checked, so that case rides on the stderr
# classification alone.
system_defaults_read_actual() { # <plist_path> <key>
  local plist_path="$1" key="$2"
  local file_candidate
  for file_candidate in "$plist_path" "$plist_path.plist"; do
    if [[ -e $file_candidate && ! -r $file_candidate ]]; then
      return "$SYSTEM_READ_UNREADABLE"
    fi
  done
  local value read_error_file read_status=0
  # A failed mktemp must not become an ambiguous redirect that silently loses the
  # classifier's only input. Refuse toward indeterminate, the safe direction, and
  # SAY SO: the ambiguous redirect ends at this same status by accident, so
  # without the message the deliberate refusal and the accident are
  # indistinguishable to a caller and to a test.
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
  # Only a genuinely absent domain or key is "unset". Every other failure, an
  # unparseable plist, a traversal-blocked parent, a `defaults` that is missing or
  # errors, or a message this does not recognize on a future macOS, stays
  # indeterminate rather than being reported as a known state.
  if grep -q 'does not exist' "$read_error_file"; then
    rm -f "$read_error_file"
    return "$SYSTEM_READ_UNSET"
  fi
  rm -f "$read_error_file"
  return "$SYSTEM_READ_UNREADABLE"
}
