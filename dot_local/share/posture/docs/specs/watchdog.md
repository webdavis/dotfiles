# Watchdog decisions and manifest audit

Given probe readings, the domain produces problems and proposed next state. It does not read files,
launch processes, publish state or submit a page. The watchdog cutover owns those operations and their
ordering. The statements below retain the behavior of the Bash watchdog and audit in plan row 2.6.

- Given no trustworthy clock, report that scheduled results cannot be verified. Otherwise require a
  running osqueryd and a scheduled canary within the freshness bound in either direction. Missing, stale
  and implausibly future canaries have distinct messages (S208, S211).
- Given any of the six watched agents is unloaded, report its fixed label and omit its next state. For a
  loaded agent, zero or the exact whitespace-tolerant `(never exited)` sentinel resets its streak.
  Missing and malformed exit fields report distinct unknown states. Only the leading signed decimal part
  of a numeric exit reaches a page (S212 to S214).
- Given a nonzero exit, a changed or unreadable runs counter increments the failure streak. An equal
  counter preserves it. A streak of two or more reports a crash loop, including a frozen streak already
  at two. A counter reset counts as a changed run (S213).
- Given route status 2xx or 405, report no route problem. Every other status is unhealthy; failed or
  nonnumeric readings display 000. An unwritable watchdog state is itself a problem (S210, S215).
- Given manifest readings, inspect the pipeline manifest before the managed-bin manifest, preserving line
  order. A refusal retains every earlier finding. Missing, empty, untrusted and malformed manifests
  refuse distinctly. An unavailable audit seam refuses without an all-clear (S217, S227, S230 to S235).
- Given one path, report each drifting column separately. Symlinks and other irregular files are not
  followed. An `unbuilt` regular artifact is content drift. Oversize and unreadable hashes still allow
  mode and owner drift to be reported; unreadable attributes produce one unreadable finding (S228, S229,
  S238, S239).
- Given configured bounds, accept canonical decimal entries 1 to 100000, bytes 1 to 1073741824 and
  seconds 0 to 300. Invalid values use defaults 500, 8388608 and 60. The entry ceiling applies to each
  manifest, the time budget is one deadline from the supplied start, and a size exactly at the byte
  ceiling remains hashable. Refuse at the deadline, including a zero budget (S236, S237).
- Given an audit report, hash its byte-sorted lines with duplicates retained. Only its validated
  lowercase fingerprint enters proposed state. Two consecutive identical fingerprints confirm the
  condition; an already paged fingerprint suppresses repeats. A changed fingerprint restarts at one, and
  a clean audit clears both memories. Invalid fingerprints page every tick. Audit streaks clamp to 99 on
  both read and write (S221 to S223).
- Given a confirmed report, count columns rather than paths, accepting counts 1 to 999999. Render only
  the seven fixed kind labels and static refusal text. Mixed findings followed by a refusal use the
  unknown message. Report paths and hostile tokens never enter the problem text (S218 to S220).
- Given problems, preserve their order in one critical page with Sosumi sound, the issue count when
  greater than one, and the existing diagnosis and restart instructions (S225).

The next application owner must validate the persisted document as exactly one object (S209), persist
healthy baselines with a warning on failure (S224), and persist unhealthy baselines only after durable
page acceptance (S226). A refused submission must preserve the prior baselines and return failure. Those
effects are not simulated by this domain. Probe 4's queue policy (S216), independent pns health
inspection, process deadlines, filesystem readers and the producer cutover remain in the watchdog
application and adapter rows. The operator's priority-route configuration is still required.
