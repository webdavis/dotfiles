# Controls, poller and Funnel policy

The domain evaluates supplied readings and returns pages and proposed state. These contracts cover plan
rows 2.7 and 2.8. The application must store each required page before publishing the proposed baseline.
The process and file adapters supply the readings; this policy does not perform those effects.

- Given a controls document, admit the whole nonempty array or return one refusal. Require a unique
  nonempty identifier made of lowercase ASCII (American Standard Code for Information Interchange)
  letters, digits and underscores, excluding `firewall`, `gatekeeper` and `screenlock`. Admit only tier
  `verify`, the eight declared readers and each reader's two exact values. Both rule readers require an
  absolute target; every other reader refuses one. Targets refuse newline and unit separator. Carriage
  return and tab remain literal target data, matching Bash (S254 to S256).
- Given declaration text, strip backslash, backtick, dollar and both quotes, flatten carriage return,
  newline and tab, and retain at most 160 characters. Refuse a description that becomes empty; an empty
  remedy is allowed. Render those admitted fields as inline-code spans (S254, S266).
- Given a failed probe, disregard its printed result. Message classification requires one consistent
  recognized value. Process-id output and status must agree: successful digit-only lines mean running,
  and exit 1 with no output means stopped. All five FileVault forms retain their on/off meaning,
  including deferred enablement as off. Automatic login is on whenever its declaration read succeeds;
  only the exact missing-key diagnostic means off after failure (S246 to S249).
- Given LuLu base preferences, rule readers are usable only after a successful nonempty read with no
  `currentProfile` key. Key presence or an unreadable preference makes only the rule controls
  indeterminate and gives the gap its fixed cause (S251).
- Given prior state, require mode 600, exactly one object and the exact trio domains: firewall 0/1/2,
  Gatekeeper 0/1 and screen lock 0/1. Each control independently requires an in-domain prior under the
  same expectation and target. A changed declaration rearms that control (S260, S261).
- Given unreadable members, report the trio as `posture_query`, a refused declaration as `controls_file`,
  and each indeterminate control by identifier. A new member pages during an existing gap; recovery
  removes that member so another failure can page. Clean members still compare and advance while gapped
  members retain only trusted priors. Refused controls preserve all prior fields under the new trio.
  Without a readable trio or trusted prior, stop after the gap decision (S257 to S262).
- Given a first observation, page each already-off protection and each already-deviant control. With
  trusted state, page only transitions to off or away from the declaration. Healthy seed, steady
  deviation and recovery are silent. Combine exposures in built-in then declaration order, with the
  existing critical title, count and Sosumi sound. A missing controls file and first firewall-off produce
  separate gap and exposure pages in that order (S263, S264, S267).
- Given the projected `AllowFunnel` values of exactly one document, omit absent, null and false. Boolean
  true entries at any depth make Funnel active; every other non-map value or nonboolean map entry makes
  it unreadable, even beside an active entry. A map containing only false entries is inactive.
  Deduplicate and sort active keys (S268, S269).
- Given Funnel state, distinguish absent from corrupt. Only one object with exact `active` or `inactive`
  is trusted; a failed-publication marker removes that trust. Page on opening or an untrusted active
  read. Corrupt state gets a gap on a tick with no exposure page, then a proposed repair. A failed read
  retains the prior state, and a valid read clears the read gap. Steady active and closing are silent
  (S272 to S275, S277).
- Given exposed keys, strip backticks, flatten carriage return/newline/tab, cap each at 200 characters
  with the captured truncation suffix, and wrap it in a code span. Preserve exact rendered page bytes,
  including the shell's trailing-newline removal at submission (S276).

The controls-file reader validates exactly one complete JSON (JavaScript Object Notation) array before
returning any controls (S254, S255). It follows a symlink to a regular file. A missing path, directory,
broken symlink or named pipe returns the missing-file refusal; a failed regular-file read returns the
malformed-file refusal. The reader leaves the file and symlink unchanged.

Field projection preserves the captured Bash bytes before domain admission: numeric identifiers, boolean
and compound text, object encounter order with last duplicate values, omitted false/null fields, leading
zeroes, non-finite numbers, byte-order marks and malformed character replacement. A high surrogate
without its required partner refuses the document; an isolated low surrogate becomes a replacement
character. Command substitution removes null characters and trailing newlines before the domain sanitizes
descriptions and remedies. A later invalid row refuses the whole set (S254 to S256).

The reader returns a typed refusal without printing. The future caller still owns Bash's additional
shell-redirection diagnostic for an unreadable file, alongside the malformed-file explanation. The
controls array's typed length replaces the intermediate count string. Process arguments, deadlines, LuLu
archive/path reads, state-file ownership, publication failures and durable acceptance remain in the
planned application and adapter rows (S250, S252, S253, S265, S270, S271, S278). The Bash callers remain
active until those cutovers.
