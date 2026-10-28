# Design: making the page-launchd allowlist a real boundary

Branch: `feat/osquery-allowlist-boundary`. Base: `51539ea` (`origin/main`, fetched over HTTPS; SSH to
GitHub times out in this environment). Status: design only, no implementation.

## Context

The osquery alerting pipeline pages the operator when a new user LaunchAgent appears. Under the
slice-6 default-deny ruling, an agent that is not in the page-launchd allowlist pages, and one that is
in it stays quiet. That single file therefore decides whether persistence alerts fire at all.

It is deployed at `~/.config/osquery/page-launchd-allowlist.txt`, owned and writable by the operator.
It is not listed in the root-owned pipeline-integrity manifest, so neither of the two manifest layers
ever looks at it. A change to it is routed to the daily digest, never to a page.

The attack: a process running as the operator appends a tuple naming its own LaunchAgent with an empty
`sha256` (the own-agent convention that skips the hash check), then installs that agent.
`allowlist_verdict` returns 0, the persistence finding is dropped, and the allowlist edit surfaces at
most once, the next day, inside a grouped digest. The component that decides whether to alert is
neither protected nor watched.

## What I verified before designing

Everything below was checked against the tree or run on this host, not recalled.

1. **The allowlist is user-writable and unmanifested.** The manifest runner
   (`.chezmoiscripts/run_after_05-osquery-pipeline-manifest.sh`) filters `chezmoi managed` down to two
   patterns: `$HOME/.local/libexec/osquery/*` and
   `$HOME/Library/LaunchAgents/com.webdavis.osquery-*.plist`. The allowlist matches neither.
   `_pipeline_is_tracked` in `results-alerter/pipeline-verdict.sh` uses the identical two patterns, and
   `.chezmoitemplates/osquery/osquery.conf` watches the same two path sets under the
   `pipeline_integrity` and `launch_agents` categories. All three agree, and all three exclude the
   allowlist.

1. **Its mutation only digests.** `results-alerter/route.sh` line 283 onward: the `allowlist_file`
   category matches basename `page-launchd-allowlist.txt` and calls `digest_append`, then `continue`.
   The digest is a once-daily grouped summary
   (`Library/LaunchAgents/com.webdavis.osquery-digest.plist.tmpl` uses `StartCalendarInterval`), so
   worst-case notice is roughly 24 hours, in a low-salience channel.

1. **`file_paths_hashes` does not cover `~/.config/osquery`.** The `allowlist_file` category appears in
   `file_paths` but not in `file_paths_hashes`, so allowlist events carry no `sha256` column at all.

1. **Passwordless sudo is real on this host.** `sudo -n true` succeeds.

1. **`chezmoi apply` destroys out-of-band writes to this file.** Verified empirically with a throwaway
   source, destination and persistent state on chezmoi v2.71.0: a plain (not `modify_`, not `create_`)
   managed file is rewritten from source on every apply. An appended line was gone after the next
   apply.

1. **A targeted apply does not run `run_after` scripts.** Same throwaway setup: a full
   `chezmoi apply --force` fired `run_after_05`; `chezmoi apply --force <one-target>` restored the file
   and fired nothing.

1. **No third-party seed has ever been added.** Across the full history of both
   `dot_config/osquery/private_page-launchd-allowlist.txt` and its predecessor
   `dot_config/osquery/launch-allowlist.txt`, every entry ever added is a `com.webdavis.osquery-*`
   own-agent, and every one landed as a source commit (`46a99cb`, `d0d4dae`, `1320d3c`, `a121b65`).
   `allowlist.sh -a` has produced zero committed entries.

## The reframe that facts 5 and 7 force

The brief describes the blocker as: the slice-5 writer updates the deployed allowlist outside a chezmoi
apply, so manifesting the file would page on every legitimate seed.

That workflow does not survive contact with chezmoi. A tuple written by `allowlist.sh -a` lives until
the next `chezmoi apply` and is then silently erased, along with the suppression it bought. The operator
would find the agent paging again with no explanation. So the out-of-band seed is not a working
workflow that a manifest would break. It is a latent bug, and the history confirms nobody has relied on
it: every entry that exists got there by editing the source and committing.

This changes the shape of the decision. Option B is not a workflow migration with a cost. It is the
repair of a workflow that is already broken, and the boundary fix falls out of it.

## Options

Each option is judged on five questions: does it stop the attack, does it survive passwordless sudo,
what happens to the seed workflow, what machinery it costs, and how it interacts with the existing two
layers (event-time verdict, 15-minute scheduled audit).

### A. Root-own the allowlist with a privileged update path

Deploy the file `root:wheel 0644` and give the writer a `sudo` update path, mirroring how the manifest
itself is installed.

- **Stops the attack?** Only the literal write. A user process can no longer append directly.
- **Survives passwordless sudo?** No. `sudo tee` is one command away, with no prompt. This is the same
  limit both `_pipeline_manifest_is_trustworthy` and the manifest runner already record about the
  manifest itself. Say it plainly: on this host root ownership is a raised bar and a loud failure mode,
  not a boundary.
- **Seed workflow?** Worse. Seeding now needs a privileged write, and the file still gets clobbered by
  the next apply, because chezmoi writes every target as the invoking user and would fight the root
  ownership on every run. Option A and chezmoi management are close to incompatible.
- **Machinery?** A privileged write path in a user-facing curation tool, plus an exception to chezmoi's
  ownership model.
- **Two-layer interaction?** None gained. A root-owned file that is still unmanifested is still invisible
  to both layers. The edit would go on digesting.

**Verdict: weak.** It buys the least of any option here and costs the most in ownership complexity. It
is also the only option that makes the file harder to manage rather than easier. Reject.

### B. Manifest the allowlist and route the writer through chezmoi

Add `~/.config/osquery/page-launchd-allowlist.txt` to the manifest's path filter, to
`_pipeline_is_tracked`, and to the `allowlist_file` routing arm. Change `allowlist.sh` so `-a` and `-d`
edit the chezmoi **source** file, apply that one target, and then refresh the manifest.

- **Stops the attack?** It detects it, within seconds. The deployed bytes no longer match the manifest
  tuple derived from chezmoi intent, so `pipeline_verdict` pages a CRIT for the allowlist file. The
  suppression of the persistence finding still happens on that pass, so this is detection, not
  prevention, unless it is paired with the verdict hardening described below.
- **Survives passwordless sudo?** No, and nothing does. An attacker who escalates rewrites the manifest
  and both layers then bless whatever they wrote. What B buys is that the allowlist stops being a
  *softer* target than the alerter's own scripts: it inherits exactly their trust properties, no better
  and no worse.
- **Seed workflow?** It gets fixed. A seed becomes a source edit plus an apply, which is what every
  existing entry already is, and the tuple now survives future applies instead of being erased. The
  manifest regenerates from the same source in the same flow, so a legitimate seed is silent by
  construction. Zero false pages.
- **Machinery?** Moderate and mostly reuse. Three path filters gain one entry each, one routing arm
  changes from `digest_append` to `pipeline_verdict`, and `allowlist.sh` gains source resolution
  (`chezmoi source-path`), a targeted apply and a manifest refresh. Because a targeted apply does not
  run `run_after` scripts (fact 6), the writer must invoke
  `.chezmoiscripts/run_after_05-osquery-pipeline-manifest.sh` itself; that script is deliberately not a
  template and already handles a direct invocation with `CHEZMOI_SOURCE_DIR` unset.
- **Two-layer interaction?** Both layers pick it up for free. Layer 1 judges any event on the path
  against the full content, mode and owner tuple. Layer 2 re-reads it every 15 minutes, which covers the
  hard-link and symlink-referent shapes that generate no event on the watched path. Binding mode and
  owner also means a `chmod 0666` on the allowlist now pages, which nothing catches today.

### C. Page on any allowlist mutation

Leave the file where it is and change the `allowlist_file` arm from `digest_append` to `sev="CRIT"`.

- **Stops the attack?** It makes it loud within seconds. It does not stop the suppression.
- **Survives passwordless sudo?** No. The decision lives in `route.sh`, which an escalated attacker can
  rewrite; they would have to also rewrite the manifest to stay quiet about it, which passwordless sudo
  permits.
- **Seed workflow?** Every legitimate seed pages and needs an operator dismissal. The noise cost is
  empirically near zero right now (fact 7: no third-party seed has ever happened), but it is noise on
  exactly the workflow that is meant to become easier, and it trains the operator to dismiss
  allowlist pages, which is the failure mode this whole subsystem exists to avoid.
- **Machinery?** Trivial. One line.
- **Two-layer interaction?** Layer 1 only, and only on the event path. An attacker who edits the
  allowlist through a hard link outside `~/.config/osquery` fires no event on the watched path and pages
  nothing. Layer 2 cannot help, because layer 2 walks the manifest and the file is not in it.

**Verdict: strictly dominated by B.** B is louder (both layers, including the no-event shapes), quieter
on legitimate work (zero false pages instead of one per seed), and fixes the clobber bug as a side
effect. C survives only as a fallback if B is judged too much machinery.

### D. Bind the allowlist's content to something the attacker cannot also edit

The brief asks whether the manifest's intent-derived trick applies here. It does, and that is precisely
what B does: the manifest never reads the protected tree, it derives content from `chezmoi cat`, mode
from `chezmoi dump` and owner from the running uid, then installs the result root-owned. Applying that
to the allowlist requires the source to be the authority for the file's content, which is the writer
change in B. So D is not a separate option. It is B's mechanism, and B is D's prerequisite.

There is one genuinely separate idea in this family, and it is worth calling out because it converts B
from detection into prevention:

**D-prime, the verdict consults the manifest for its own input.** Make `allowlist_verdict` refuse to
suppress anything when the deployed allowlist's current content, mode and owner are not the manifest's
tuple for that path. It already has the seam: `results-alerter.sh` sources both
`allowlist-verdict.sh` and `pipeline-verdict.sh` into the same shell, so
`_pipeline_deployed_state_is_known_good` is callable, guarded by a `declare -F` check the way
`pipeline-audit.sh` guards its own reuse.

- **Stops the attack?** Yes, in the sense that matters: the appended tuple buys no suppression. The
  attacker's LaunchAgent pages on the persistence detector, and the allowlist tamper pages separately.
  Two independent signals from one edit.
- **Survives passwordless sudo?** No. Same manifest, same escalation.
- **Seed workflow?** Unaffected once B is in place, because after a seed the deployed file matches the
  manifest again.
- **Machinery?** Small, and no new file or dependency. The real cost is the failure mode: during the
  sub-second window between an apply writing the file and `run_after_05` reinstalling the manifest, an
  unbound allowlist suppresses nothing, so a persistence finding judged in that window pages for an own
  agent. `_pipeline_tuple_settles` already implements exactly the bounded wait for that shape and should
  be reused rather than reinvented.
- **Two-layer interaction?** It adds a third consumer of the same manifest, which is the point: one
  root-owned known-good list, three enforcers.

### E. Suppress own agents by manifest membership instead of by allowlist entry

All seven current entries are `com.webdavis.osquery-*` agents whose plists are already in the manifest.
They carry an empty `sha256` only because their plists change with the dotfiles. If the verdict
suppressed a user LaunchAgent whose plist path is manifested and currently matches its tuple, those
seven entries would not need to exist, and the empty-`sha256` convention could be deleted outright.
`allowlist.sh` already refuses to write an unpinned tuple, so after E every allowlist entry would carry
a real hash.

- **Stops the attack?** It kills the attack *as described*, because there is no longer an
  empty-`sha256` convention to abuse. It does not stop the general attack: the attacker pins their own
  plist's hash instead, which costs them one `shasum`. Do not let this option's elegance be mistaken for
  a fix.
- **Survives passwordless sudo?** No.
- **Seed workflow?** Improves it. The seven own-agent entries stop needing hand maintenance, and the
  completeness guard in `test/integration/osquery-page-allowlist-seed.sh` (which already caught one
  missing tuple) becomes unnecessary for own agents.
- **Machinery?** Moderate, and it touches the detector's semantics rather than only its inputs.
- **Two-layer interaction?** It routes own-agent trust through the manifest, which is the same
  consolidation D-prime performs.

**Verdict: a good follow-on, not the fix.** It removes a convention that exists only as an exception,
and it composes cleanly with B. It should not be bundled into the same change as the boundary work.

## Recommendation

**Do B, with D-prime, as one change. Defer E to a follow-up. Reject A. Keep C only as the fallback if
the writer change is judged too invasive.**

The reasoning in three lines:

1. B is the only option that makes a legitimate seed *cheaper* rather than more expensive, because the
   workflow it supposedly breaks is already broken and B repairs it.
1. B reuses the manifest machinery whole. No new trust root, no new file format, no new privileged
   path. The allowlist simply joins the set of files the pipeline already protects, and inherits both
   enforcement layers, including the scheduled audit that covers the no-event shapes.
1. D-prime is what turns B from "you find out afterwards" into "the tampered entry does not work," and
   it costs one guarded function call because both helpers are already sourced into the same shell.

Sequencing, so no step leaves the host in a paging state:

1. Add the allowlist path to the manifest filter, `_pipeline_is_tracked`, and the routing arm, in one
   change. Match the **exact path**, never the directory: `~/.config/osquery` also holds
   `webhook-secret`, `osquery.conf`, `packs/` and the writer's `.lock` file, and every one of those must
   stay a log-only neighbor.
1. Change `allowlist.sh` to write the source, apply that one target, then invoke the manifest runner.
   Apply first and refresh the manifest second, matching the order a real apply uses, so the brief
   mismatch is the shape `_pipeline_tuple_settles` already waits out. The write lock moves to the source
   file, which also gets the `.lock` out of the watched directory.
1. Add D-prime to `allowlist_verdict`, guarded by `declare -F` and reusing `_pipeline_tuple_settles`.
1. Fail closed everywhere a new failure is possible: a writer that cannot resolve the chezmoi source, or
   cannot refresh the manifest, must refuse to claim it seeded anything.

Optional, low value: adding the allowlist to `file_paths_hashes` in `osquery.conf`. `pipeline_verdict`
re-hashes from disk and uses the event digest only to recognize the atomic-rename shape, so without it
every allowlist event simply takes the 0.3 second rename path. If it is added, scope it to the single
file, not to `~/.config/osquery/%%`, so `webhook-secret` does not get its digest recorded into
`file_events`.

## What this does not defend against

In the style of the coverage map at the top of `pipeline-verdict.sh`.

- **A USER-LEVEL ATTACKER ON A PASSWORDLESS-SUDO HOST. No option in this document closes that, and
  none can.** Every proposal here roots trust in the manifest at
  `/var/osquery/pipeline-known-good.sha256`, which is root-owned `0644`. A process running as the
  operator escalates with no prompt, rewrites the manifest to bless whatever it just wrote, and both
  layers go quiet. That is the same limit already recorded in `_pipeline_manifest_is_trustworthy` and in
  the manifest runner's docblock, and it applies unchanged to the allowlist once the allowlist joins the
  manifest. The honest claim for the recommendation is: the allowlist stops being the softest component
  in the pipeline and becomes exactly as hard as the alerter's own scripts. It is not a boundary that
  survives sudo. Requiring a sudo password is the only thing that would change that, and every
  unattended chezmoi script on this host depends on the current configuration.

- **SOURCE COMPROMISE.** Under the recommendation the chezmoi source becomes the authority for the
  allowlist's content, and the source is user-writable. An attacker who edits
  `dot_config/osquery/private_page-launchd-allowlist.txt` and then waits for, or races, a legitimate
  apply gets their tuple both deployed and manifested, signed as known-good by all three enforcers. This
  is not a regression; it is the identical exposure the alerter's own scripts already carry, and the
  manifest runner's docblock already records it. Worth stating explicitly here so the allowlist's
  posture is not read as stronger than the scripts it governs.

- **PREVENTION AGAINST AN ATTACKER WHO PINS A HASH.** Option E deletes the empty-`sha256` convention,
  which is the specific mechanism this report's attack uses. It does not stop an attacker from computing
  the hash of their own plist and writing a fully pinned tuple. Any allowlist whose contents an attacker
  can influence remains an allowlist an attacker can extend. The value of the recommendation is that the
  influence now requires either a source edit or a manifest rewrite, both of which have their own
  recorded exposures, rather than a bare append to a user-writable file that nothing reads.

- **THE WINDOW BETWEEN APPLY AND MANIFEST REFRESH.** D-prime makes an unbound allowlist vouch for
  nothing, so during the sub-second gap between the file landing and the manifest being reinstalled,
  a persistence finding judged in that window pages for an agent that is in fact known-good. This fails
  in the safe direction and `_pipeline_tuple_settles` bounds it, but it is a real false-page shape and
  should be tested rather than assumed away.

- **RE-TAMPERING WITHIN THE SAME DIVERGENCE SET.** Inherited unchanged from layer 2. Once the allowlist
  is reported as a content divergence, a second edit to it does not page again until the divergence kind
  set changes. Documented in `pipeline-verdict.sh`; it now applies to the allowlist too.

## Open question for adjudication

One, and it is the only thing I would hold implementation on: **should `allowlist.sh -a` run
`chezmoi apply` on the operator's behalf, or stop after editing the source and print the apply command?**

Auto-applying makes the tool a one-step seed and keeps the manifest consistent without operator
discipline, at the cost of a curation tool that mutates the home directory and calls `sudo` through the
manifest runner. Printing the command keeps the tool inert and makes the operator the one who deploys,
at the cost of a window in which the source and the deployed file disagree and the seed silently does
not work yet. My recommendation is auto-apply, because the alternative reintroduces exactly the
"seed does not take effect" confusion that the current clobber bug already produces. Confirm before
implementation.
