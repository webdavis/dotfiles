# Allowlist and known-good policy

These scenarios cover posture plan row 2.3, statements S069 to S097, S234, S301 to S303, S305 and S306.
The allowlist reader remains in Bash; curation now runs through `posture allowlist`.

| Given                                                              | When                                                                                   | Then                                                                                                               |
| ------------------------------------------------------------------ | -------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| An unreadable allowlist or no matching label                       | A user agent is judged                                                                 | Return NotAllowlisted without requesting a vouch.                                                                  |
| Several entries with the same label                                | An identity is judged                                                                  | The first matching entry decides, including degraded and reused entries.                                           |
| An entry with either identity column empty                         | A finding has that label                                                               | Return NotAllowlisted.                                                                                             |
| A matching label with a different path or program                  | The finding is judged                                                                  | Return ReusedLabel before requesting a vouch.                                                                      |
| A pinned matching identity                                         | Current plist digest differs or cannot be read                                         | Return ReusedLabel. Digest comparison remains case-sensitive here.                                                 |
| An unpinned matching identity                                      | Its current plist cannot be vouched for                                                | Return NotAllowlisted without checking the list itself.                                                            |
| A pinned or unpinned identity otherwise eligible for suppression   | The list file cannot be vouched for                                                    | Return NotAllowlisted.                                                                                             |
| A fully matched and vouched identity                               | The list also has a vouch                                                              | Suppress. Untrusted signing still pages through the existing gate.                                                 |
| A captured identity                                                | Its paths are made portable                                                            | Replace leading HOME in the plist path and every HOME occurrence in the program; both require the following slash. |
| A valid allow or deny request                                      | Source lines include an invalid parse result                                           | Refuse the whole curation and identify the one-based line.                                                         |
| An allow request                                                   | Source has repeated entries for that label                                             | Drop every match, preserve other raw lines in order, append the fresh entry.                                       |
| A deny request                                                     | Source lines are valid                                                                 | Drop matching entries and preserve all other lines without appending.                                              |
| A protected manifest                                               | Owner is not exactly 0, mode is unreadable, or group/world write is set                | Refuse its authority. The explicit fixture override retains its existing exception.                                |
| A manifest line for audit                                          | Digest, four-octal mode, one-to-ten-digit owner or absolute non-root path is malformed | Refuse the line. Single spaces separate columns; spaces inside the final path survive.                             |
| An unbuilt record or an unreadable/empty observed column           | A tuple is compared                                                                    | It cannot vouch for content, even when the supplied digest is forged as unbuilt.                                   |
| Current file state                                                 | A trusted manifest is selected by path                                                 | Match hash case-insensitively and match mode, owner and path verbatim against that one manifest.                   |
| A missing, unreadable, empty or untrustworthy managed-bin manifest | A path under local bin or libexec changes                                              | Track the path so the absent authority cannot silence monitoring.                                                  |
| A healthy managed-bin manifest                                     | A neighboring path is absent from it                                                   | Leave that neighbor untracked.                                                                                     |
| An untracked path                                                  | Any event arrives                                                                      | Remain log-only without reading current state.                                                                     |
| A tracked deletion, symlink or non-regular target                  | An event arrives                                                                       | Page before any rehash or vouch.                                                                                   |
| A tracked regular target with an empty event hash                  | Its current state is requested                                                         | Request the atomic-rename delay before the vouch. Only this event shape requests it.                               |
| A tracked regular target                                           | Current state differs despite a matching old event hash                                | Page. The event hash is never content authority.                                                                   |

Labels require at least two ASCII characters, start alphanumeric, and thereafter accept only
alphanumerics, period, underscore, at-sign and hyphen. The case-insensitive exact label com.apple and its
dot-prefixed descendants are refused.

The supplied vouch function is the shared authority used by allowlist and integrity policy. The
application and adapters own elapsed time, the shared settle budget, retries, filesystem reads,
serialization and publication. Row 2.3 added pure policy. The curation command below now owns the writer
effects.

## Curation command cutover

Given `allowlist add <label>` or `deny <label>`, the writer takes the blocking kernel lock before
validation and keeps it through capture, source rewrite, apply and manifest refresh. Lock setup fails
closed. Children cannot inherit its descriptor. List reads deployed bytes without taking the lock,
skipping only empty lines and lines whose first byte is `#`, and adds a final newline to entry lines.

Add captures the first launchd row and a complete lowercase SHA-256 (Secure Hash Algorithm 256-bit) plist
digest before resolving source. It removes matching source objects, preserves every other raw line, then
appends the new tuple. Both reordered and byte-identical refreshes publish. Deny first checks the literal
compact label substring; a miss succeeds without parsing or publication. A hit uses the existing
whole-source curation refusal. Invalid text bytes in an otherwise accepted object remain raw.

Publication writes source, applies one target, then refreshes the manifest. Child input, output and
errors stay inherited where the Bash publisher inherited them. One fifteen-minute total publication
budget covers apply and manifest location/refresh; tests inject a short budget. There are no retries.
Apply failure or timeout restores source and reports that deployment may be partial. A missing or failed
manifest keeps the new source/deployed state and reports stale integrity data with nonzero exit.

The new spelling selects one action and ignores trailing operands. Missing verbs or labels print usage
and exit 2; validation and operational refusals exit 1. Success and absent-deny notes remain on stdout.
List preserves invalid text bytes, removes NUL bytes as Bash read did, and does not validate JSON.
Mutations reject indented comments, whitespace-only lines and multiple values on one line. Their label
projection uses the last duplicate label value without rewriting the original object.

Source labels retain the existing decimal spelling and special-number coercion: a NaN label projects to
`null`, while a NaN query field becomes empty after the selected row is serialized. The query selects the
first launchd row, retains structured field formatting and preserves duplicate object-key position.
Unpaired low surrogates become replacement characters for projection; unpaired high surrogates refuse the
source even in an unused field. Original source bytes remain untouched. The legacy container-stack
boundary also remains: a source object accepts 9,998 nested arrays or 4,999 nested objects, with scalar
leaves, and refuses the next depth. Parsing inert fields does not allocate a recursive value tree.

Inspection and source lookup each use a ten-second command budget. Lock waiting precedes these budgets.
Publication uses mode-600 staging and backup files. On failed apply, source rollback preserves the old
bytes; if rollback itself fails, the error reports that failure. Manifest failure retains the new bytes.
No curation command sends a notification or updates an unrelated managed target.
