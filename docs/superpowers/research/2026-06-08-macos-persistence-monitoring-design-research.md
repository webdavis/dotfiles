# macOS Persistence-Monitoring: Trust & Noise Design Research

**Date:** 2026-06-08
**Question:** For a single-user personal Mac (osquery + custom bash alerter → Discord), how should
known-good software be allow-listed / suppressed so the system stays low-noise and low-maintenance —
and is the whole real-time-alert approach even right? Triggering pain: legit ad-hoc Homebrew daemons
(e.g. `atuin`) trip CRITICAL, and a per-binary hash allowlist would need re-vouching on every
`brew upgrade`.

**Method:** four parallel web-research passes (Google Santa; Objective-See BlockBlock/KnockKnock;
FIM/osquery detection-engineering practice; trust-strategy & paradigm), each citing primary sources.

______________________________________________________________________

## Verdict

The user's instinct is correct on both counts:

1. **Per-binary hash allow-listing is a recognized anti-pattern** for trusting frequently-updated
   software. It is high-maintenance by construction and the standards literature says so explicitly.
2. **"Alert in real time on every persistence change" is the wrong paradigm for a single-user Mac.**
   It duplicates a noisy signal macOS already ships, and practitioners reduce noise with
   baseline-diff + trust-on-first-use + suppression, not with a better allowlist key.

The fix is a **paradigm change**, not a better allowlist format.

______________________________________________________________________

## Finding 1 — Hash allow-listing is an anti-pattern for known-good

NIST SP 800-167 (Guide to Application Whitelisting) states a hash "is not helpful when a file is
updated, such as when an application is patched; the patched version will have a different hash," and
that the correct flow validates the new version via its **digital signature**, then records the hash —
i.e. signatures, not hashes, are the durable trust anchor [1]. CISA's allow-listing guidance echoes
that hash rules "require continuous updating as software changes" [2]. First-generation hash-only FIM
is widely described as "noisy, unmanageable, and not scalable" because a hash change says nothing about
whether the change was authorized [3].

**Implication:** keep hashes only for *blocklisting* a known-bad sample. Do not use them to allowlist
known-good.

## Finding 2 — Signing authority / Team ID is the durable key — but ad-hoc Homebrew has none

The canonical osquery launchd pattern allowlists by **signing authority**: legitimate `com.apple.*`
items are signed by Apple's `Software Signing` authority; third-party legit software is trusted by its
**Developer ID / Team ID** (e.g. `Developer ID Application: Microsoft Corporation`) rather than
per-binary [4][5]. A Team ID is roughly **update-invariant** — the same vendor keeps the same Team ID
across versions — so the entry survives upgrades [5]. Fleet exposes `team_identifier` and `authority`
on the `launchd`/`signature` tables for exactly this [6][7].

**The catch for THIS user:** Homebrew CLI **formulae** are ad-hoc signed (satisfies launchd, but no
Developer ID, no Team ID) [verified locally: atuin, jq, yq, starship, zoxide, sesh all ad-hoc]. So
Team-ID allow-listing works for casks / vendor apps but **not** for atuin-style ad-hoc daemons — the
exact binaries that trip the CRITICAL rule.

## Finding 3 — Google Santa is off-target

Santa evaluates rules most→least specific: **CDHash → Binary hash → Signing ID → Certificate →
Team ID** [8]. But TEAMID rules require a *production* certificate [9], and for unsigned/ad-hoc binaries
Santa skips all signature-derived properties so "only file hash … rules apply" [8] — reproducing the
re-vouching treadmill. North Pole confirms the churn: Homebrew bottles "produce different hashes for
every build," and the only "trust once" fix is their **hosted Workshop Package Rules** SaaS [10].
Santa is also an *execution-authorization* engine (blocking), which the user explicitly does not want;
Monitor mode logs but yields an event firehose, not curated alerts [11]. Community project **Santamon**
layers persistence detection on Santa telemetry for home labs, a better fit than Santa+Workshop if
richer detection is later wanted [12].

## Finding 4 — Objective-See (BlockBlock / KnockKnock) is a backstop, not a replacement

By design these tools **do not auto-classify**: KnockKnock "simply lists persistently installed
software … does not try to determine if something is malware" [13]; both surface signing info +
VirusTotal + process lineage and leave the verdict to the user. KnockKnock filters Apple-signed items
(third-party still shown) and is a *manual* periodic scan [13]; BlockBlock is real-time and converts
each user Allow/Block into a remembered rule (trust-on-first-use) [14]. BlockBlock's noise-cutting
allowlist is **Developer-ID-keyed**, so an ad-hoc Homebrew daemon still prompts [15], and there is no
headless/Discord path — unsuitable as multi-Mac, notify-not-prompt infrastructure. Wardle's own
methodology treats code-signing + notarization as the primary false-positive reducer [16][17].

## Finding 5 — The paradigm: stop paging on every event

- **Apple already fires a native real-time persistence notification** ("Background Items Added") for
  every login item / LaunchAgent / LaunchDaemon, and it is widely reported as too noisy; macOS's own
  mitigation is a 1-day / 1-week **snooze** [18]. A Discord alert on "new launchd item" substantially
  **duplicates a built-in signal** with little marginal value.
- **Detection-engineering noise control = baseline first, then tuning + exceptions + suppression +
  snooze.** "Establishing a one-week baseline before making changes is essential"; suppression
  "acknowledges that a signal is real but reduces repetitive noise" [19].
- **osquery is built for diffing, not paging:** differential logging only emits on change, with an
  `epoch`/`counter` to "skip the initial added records" — i.e. it is designed to feed a baseline-diff
  review [20][21].
- **Location/provenance is weak as a *trust gate*:** `com.apple.quarantine` "isn't protected … it's
  straightforward to remove," isn't set on locally-built or package-manager files, and provenance is a
  forensic tag — so path/quarantine rules rot and false-positive on a dev Mac [22]. Use location as a
  *weighting hint*, never as the CRITICAL gate.
- **Lean on built-ins:** Gatekeeper/XProtect/Background Task Management already cover the benign case
  with minimal interruption; add-on agents raise privileged attack surface [23][24].

______________________________________________________________________

## Recommendation

Re-anchor the system on a **batched baseline-diff + trust-on-first-use** model:

1. **Batched digest, not per-event pages.** Once a day (or week), one Discord message: "here's what's
   new in persistence since the baseline." osquery differential logs feed this natively. This batches
   decisions instead of interrupting — the lowest-cognitive-load option for an ADHD single user.
2. **Trust-on-first-use acknowledgment.** Approve `atuin` **once**, by its launch-item identity (label
   + program path). No hash, no re-vouch on `brew upgrade`. Only genuinely new items resurface.
3. **Narrow real-time CRITICAL only.** Reserve a real-time page for the high-fidelity case: a *new*
   persistence item that is unsigned/ad-hoc **and** from a publisher (Team ID) not on a short personal
   allowlist **and** not already acked. Everything else → the digest.
4. **Allowlist by Team ID where it exists; ack-by-identity where it doesn't** (ad-hoc Homebrew). Store
   the Team-ID list as a tiny version-controlled file shared across machines; it changes only when you
   adopt a genuinely new vendor, never on a routine upgrade.
5. **Don't alert on plain "new launchd item."** macOS's Background-Items notification already covers it.

This removes the maintenance treadmill entirely — there is no per-binary hash to keep fresh — and cuts
noise by moving from "prove every binary in real time" to "show me what changed; I ack it once."

______________________________________________________________________

## Limitations

- Sources include vendor blogs (Cimcor, Uptycs, North Pole) used for practitioner opinion, not as
  standards; standards claims rest on NIST/CISA/Apple/osquery primary docs.
- "Background Items Added covers all install methods" is from Apple deployment docs; exact coverage of
  every osquery-watched path was not independently re-tested.
- The recommendation is a design direction, not yet validated against the user's real alert volume
  (Finding-5 batching assumes persistence changes are infrequent in steady state — true for differential
  queries, which return zero rows once baselined).

______________________________________________________________________

## Bibliography

1. https://nvlpubs.nist.gov/nistpubs/specialpublications/nist.sp.800-167.pdf — NIST SP 800-167; hash
   whitelisting breaks on patches; validate updates via signature. (Authoritative standard.)
2. https://www.cisa.gov/sites/default/files/documents/Guidelines%20for%20Application%20Whitelisting%20in%20Industrial%20Control%20Systems_S508C.pdf — CISA; hash rules need continuous updating. (Gov.)
3. https://www.cimcor.com/blog/the-comprehensive-guide-to-file-integrity-monitoring — hash-only FIM is
   noisy/unscalable; change-validation fix. (Vendor opinion.)
4. https://medium.com/@rrcyrus/hunting-for-bad-apples-part-1-256c30912476 — osquery authority-based
   launchd allowlist (Apple `Software Signing`, no Team ID). (Practitioner.)
5. https://www.uptycs.com/blog/hunting-for-evil-launch-daemons-identifying-suspicious-behavior-with-osquery — baseline-then-watch-new; Developer ID / Team ID allowlisting. (osquery-EDR vendor.)
6. https://fleetdm.com/guides/mitigation-assets-and-detection-patterns-for-ai-agents-like-openclaw — multi-signal scoring to cut false positives. (Fleet.)
7. https://fleetdm.com/tables/launchd — `team_identifier`/`authority` fields. (Fleet docs.)
8. https://santa.dev/concepts/rules.html — Santa rule precedence; unsigned → file-hash-only fallback. (Official.)
9. https://northpole.dev/features/binary-authorization/ — TEAMID requires production certificate. (Official.)
10. https://northpole.security/blog/introducing-package-rules — brew-upgrade hash churn; Workshop
    Package Rules (hosted) is the fix. (Vendor blog, firsthand.)
11. https://santa.dev/concepts/mode.html — Monitor vs Lockdown. (Official.)
12. https://github.com/0x4D31/santamon — persistence detection on Santa telemetry, home-lab scoped. (Community.)
13. https://objective-see.org/products/knockknock.html — "lists … does not try to determine if malware"; Apple-signed filter; VirusTotal. (Vendor.)
14. https://objective-see.org/products/blockblock.html — real-time persistence alerts; remembered rules (TOFU). (Vendor.)
15. https://thumbtube.com/blog/how-objective-sees-blockblock-prevented-legitimate-auto-updaters-from-running-and-the-signed-updater-allowlist-that-restored-updates/ — signed-updater allowlist is Developer-ID-keyed. (Secondary, corroborated.)
16. https://nostarch.com/art-mac-malware-v2 — Wardle; signing/notarization as FP reducer. (Book.)
17. https://taomm.org/vol2/pdfs/CH%2011%20Persistence%20Monitor.pdf — Wardle; building a persistence monitor. (Primary.)
18. https://support.apple.com/guide/deployment/manage-login-items-background-tasks-mac-depdca572563/web — native Background Task Management persistence notification + snooze. (Apple.)
19. https://www.elastic.co/docs/solutions/security/detect-and-alert/reduce-noise-and-false-positives — baseline-first; tuning/exceptions/suppression/snooze. (Vendor-neutral DE.)
20. https://osquery.readthedocs.io/en/stable/deployment/logging/ — differential vs snapshot; epoch/counter to suppress initial noise. (Official.)
21. https://github.com/osquery/osquery/blob/master/docs/wiki/deployment/anomaly-detection.md — differential baselining over IOCs. (Official.)
22. https://eclecticlight.co/2025/12/05/quarantine-macl-and-provenance-what-are-they-up-to/ — Howard Oakley; quarantine/provenance limits. (Leading independent macOS-internals writer.)
23. https://eclecticlight.co/2022/08/30/macos-now-scans-for-malware-whenever-it-gets-a-chance/ — XProtect runs routinely with minimal interruption. (Same.)
24. https://github.com/drduh/macOS-Security-and-Privacy-Guide/blob/master/README.md — add-on agents raise attack surface; built-ins as baseline. (Widely-cited community guide.)
25. https://support.apple.com/guide/security/gatekeeper-and-runtime-protection-sec5599b66df/web — what Gatekeeper/notarization/Developer ID verify. (Apple Platform Security.)
