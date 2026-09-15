# pns behavior backlog decision brief

Three pieces of carried-forward pns work are stuck on operator decisions. Two have finished designs; the
third has a finished technical answer. All twelve questions below were answered by the operator on
2026-09-15, unblocking three build ladders. Each answer is also recorded in its source design's own open
questions section, so neither document contradicts the other.

## Questions at a glance

| #   | Item                              | Question                                                                                | Decision (2026-09-15)                                                                                   |
| --- | --------------------------------- | --------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| 1   | Hue bridge certificate pinning    | Pin the bridge's own certificate, the Hue root plus bridge id, or keep unverified?      | Approach A: pin the bridge's own certificate fingerprint.                                               |
| 2   | Hue pinning: fail mode            | Refuse to start with no pin, or warn for one release first?                             | Refuse at config parse. No warn-then-refuse release.                                                    |
| 3   | Hue pinning: where the pin lives  | Committed config file, or a KeePassXC attribute?                                        | KeePassXC attribute beside the bridge address and key.                                                  |
| 4   | Hue pinning: enrollment identity  | Should `pns lights enroll` take the bridge id out of band?                              | Optional, with a warning when skipped, not required.                                                    |
| 5   | Hue pinning: scope this wave      | Does `lights` get the same fix now, and does the router client get its own design task? | Yes to both: `lights` this wave, UniFi router its own task.                                             |
| 6   | Hue pinning: command name         | Keep `pns lights enroll`, or fold it into `pns doctor`?                                 | Stays `pns lights enroll`.                                                                              |
| 7   | Hook wait events: denied          | Does `PermissionDenied` still arm the blocked lamp?                                     | No. It becomes an observation only.                                                                     |
| 8   | Hook wait events: subagent end    | Should `SubagentStop` end a subagent's own wait?                                        | Yes, add it as a fifth declaration routed to `resolved`.                                                |
| 9   | Hook wait events: sandbox alert   | Build the sandbox network approval alert now, or wait for a trigger?                    | Build B39 now.                                                                                          |
| 10  | Hook wait events: upstream report | File an issue asking Claude Code for a distinct sandbox notification type?              | No. The lack of distinction is fine.                                                                    |
| 11  | Recap card image                  | Is a phone image on the recap card worth building?                                      | Approved as a per-card-type opt-in covering every card type; the operator's own recap keeps images off. |
| 12  | Recap card image: token placement | May the Moshi upload token ride an authorization header?                                | Live again now that image cards are approved; left open for the build to decide with evidence.          |

## 1. Hue bridge certificate pinning

The Hue bridge adapter at `pns/crates/pns-adapters/src/hue/bridge.rs:151` calls
`disable_verification(true)`, so pns talks to the lamp bridge over Hypertext Transfer Protocol Secure
(HTTPS) with no certificate check at all. A design is written at
`docs/superpowers/specs/2026-09-14-hue-bridge-certificate-pinning-design.md`; it measured the live
bridge's certificate and built a working prototype, but no code has shipped and verification behavior is
unchanged.

**Decision owed:** which of three approaches to build: pin the bridge's own certificate fingerprint
(recommended, because the certificate carries no verifiable bridge name, so a fingerprint is the only
thing that actually identifies this bridge), pin the Hue root certificate plus the bridge id, or leave
verification disabled and record the risk instead. Two follow-up decisions ride on the same answer:
should a missing pin refuse to start pns (recommended) or only warn for one release, and should the pin
live in the committed `dot_config/pns/config-values.toml` (recommended) or as a KeePassXC attribute next
to the bridge's address and key.

Three smaller decisions close out the same design: should `pns lights enroll` require the bridge id off
the device's own label rather than trusting whatever answers at the configured address; does the `lights`
tool get the identical fix in this wave, given it disables verification against the same bridge with the
same key at `lights/crates/lights-adapters/src/hue.rs`; and is `pns lights enroll` the right command name
or should this live under `pns doctor` instead.

**What changes once answered:** the first pull request adds the pin type and config key with no behavior
change, the second ships the enrollment command, and the third turns certificate pinning on for real.
Until then the bridge connection stays unverified exactly as it is today.

**Decisions (2026-09-15):** approach A, pinning the bridge's own certificate fingerprint. The bridge
certificate carries no subjectAltName, so name verification is impossible, and approach B could only
answer "is this some Hue bridge" rather than "is this my bridge"; B would also require trusting a Philips
root certificate copied from a third-party mirror. A pins one specific device and is therefore the more
secure option, not merely the simpler one. A missing pin refuses pns at config parse, with no
warn-then-refuse release. The pin lives in the vault, as an attribute beside the bridge address and key
on the OpenHue entry, reversing the design's own recommendation: this repository is public (verified
2026-09-15 through the GitHub API), and its own convention already treats an id as a vault reference
rather than a committed literal, stated in `dot_config/pns/config-values.toml`'s own words about channel
ids. The cost is nothing, because the OpenHue entry already exists and the pin becomes one more attribute
on it; the fingerprint is a hash and publishing it would be low harm regardless, but consistency with the
repository's own rule and the public repository are what decided it. The bridge id check at enrollment is
optional with a warning when skipped, not required: `pns lights enroll` accepts the bridge id read off
the physical device's label and refuses when the certificate's common name disagrees, but a run without
it still enrolls and warns, because without it enrollment trusts whatever answers first, so an impostor
present at enrollment time gets pinned and everything afterward looks correct. `lights` gets the
identical pinning change in this wave; the UniFi router client becomes its own design task (task 89,
filed below). The command stays `pns lights enroll`, not a flag on `pns doctor`: doctor reports state,
enrolling performs an action and hands back a value to save, and a diagnostic that sometimes changes
things becomes one nobody runs. The design's seventh open question, about accepting a Philips root from a
third-party mirror, is moot now that A is chosen.

## 2. Hook wait events (B6, B20, B39)

pns lights up a "waiting on you" lamp state when Claude Code asks a question or requests a permission.
The wiring that arms and clears that state has three known defects, laid out in
`docs/superpowers/specs/2026-09-14-hook-wait-events-design.md`: the `asked` and `plan-ready` hook
declarations in `private_dot_claude/modify_settings.json:439` fire after the question is already answered
and can re-arm a wait that should be closing, and sandboxed network approval prompts have no
distinguishable hook event at all. The design's fix (route both declarations, plus a new
`ElicitationResult` event, to the existing `pns hook resolved`, and delete `plan-ready`) is written and
pinned by ten test behaviors, but nothing has been built.

**Decision owed:** three small calls on the main fix, plus whether to build the sandbox piece at all.
Should `PermissionDenied` keep arming the blocked lamp, given nobody is actually waiting on an answer
when it fires (recommended: no, route it as an observation instead). Should a subagent's own approval
wait end at `SubagentStop` rather than waiting for its parent session to stop (recommended: yes). Should
the sandbox network approval alert be built now with a text-matched allowlist, or wait until the sandbox
is switched on locally or Claude Code gives that dialog its own notification type (recommended: wait).
Separately, is a one-line upstream issue asking for that distinct notification type worth filing (the
operator's call either way).

**What changes once answered:** the three main-fix answers become one pull request editing the hook
declarations and the marker-clearing logic. The sandbox answer either schedules that alert as a second
small pull request or leaves it recorded as deliberately unbuilt.

**Decisions (2026-09-15):** `PermissionDenied` stops arming the waiting lamp and becomes an observation.
`pns/crates/pns/src/hook_dispatch.rs` already treats a denial as a decision the harness has taken on its
own, which is why it never forwards to the phone; the lamp had not caught up to that, and it was calling
the operator to a question that no longer exists. Yes, add a fifth declaration routing `SubagentStop` to
`pns hook resolved`, so a subagent's own wait ends when it finishes rather than being held until the
parent's Stop. Build B39, the sandbox network approval alert, now, so it is ready if sandboxing is ever
turned on. The alert stays approximate rather than specific, and that is a property of the platform, not
a defect in the build: a sandbox network dialog reaches the dialog host with no `PermissionRequest`, its
only hook-visible trace is a `Notification` whose type defaults to `permission_prompt`, and no matcher
can separate it from a tool approval. Exposure on this machine is currently zero, since no `sandbox`
block exists in the managed template or the live settings. Do not file an upstream request for a distinct
notification type; the operator's words are that the lack of distinction is fine.

## 3. Recap card image (task 78)

pns sends a recap card to the phone through Moshi's push notification service. Whether that card can
carry an image was blocked on "no known upload path" until `docs/research/2026-09-moshi-image-cards.md`
found a documented upload endpoint and confirmed, with the Moshi app's own test action, that a rich image
notification actually displays on the phone. The remaining work is entirely a value call plus one
refactor: the recap card is currently posted from inside `replay_missed`
(`pns/crates/pns/src/return_replay.rs:38`), and giving it an image would mean moving card ownership into
the detached `pns recap` child process instead.

**Decision owed:** whether the capability is worth building at all, and, if it is, how the Moshi upload
token is allowed to travel: may it ride an `Authorization: Bearer` header, or does it have to stay in the
request body the way `pns/crates/pns-adapters/src/destinations/moshi.rs:14` currently requires.

**Decisions (2026-09-15):** the feature is approved, and it covers every card type, the recap included.
What the operator declined is an image on their own recap card, which is a setting in their own config,
not a limit on the product; another user might want exactly that for their recap. This is the pattern the
repository already uses everywhere: build the capability, ship it off, and leave it off in the operator's
own configuration. The capability is opt-in per card type because of a real tradeoff, stated here rather
than only in a design document: a Moshi card's `data` carries one `type`, so turning images on for a card
type gives up the deep link that focuses the originating herdr pane when that card is tapped. The
operator's own configuration keeps the recap card's images off and keeps its deep link.

The token-placement question is live again now that image cards are being built. The research already
established that the documented upload interface requires the header form; it did not settle whether
`moshi.rs`'s stricter body-only rule should be amended to allow it. That question is left open for the
build to answer with evidence, rather than re-decided here.

**What changes once answered:** task 78 moves from a value question to an approved, not-yet-started
build: the card-ownership refactor that moves recap posting into the detached `pns recap` child, and the
per-card-type opt-in with its deep-link tradeoff stated at the point of the toggle.
