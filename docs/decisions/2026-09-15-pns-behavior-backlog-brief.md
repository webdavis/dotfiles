# pns behavior backlog decision brief

Three pieces of carried-forward pns work are stuck on operator decisions. Two have finished designs; the
third has a finished technical answer. None of them can move to a pull request until someone answers the
questions below, which today are spread across three long ledger bullets.

## Questions at a glance

| #   | Item                              | Question                                                                                |
| --- | --------------------------------- | --------------------------------------------------------------------------------------- |
| 1   | Hue bridge certificate pinning    | Pin the bridge's own certificate, the Hue root plus bridge id, or keep unverified?      |
| 2   | Hue pinning: fail mode            | Refuse to start with no pin, or warn for one release first?                             |
| 3   | Hue pinning: where the pin lives  | Committed config file, or a KeePassXC attribute?                                        |
| 4   | Hue pinning: enrollment identity  | Should `pns lights enroll` take the bridge id out of band?                              |
| 5   | Hue pinning: scope this wave      | Does `lights` get the same fix now, and does the router client get its own design task? |
| 6   | Hue pinning: command name         | Keep `pns lights enroll`, or fold it into `pns doctor`?                                 |
| 7   | Hook wait events: denied          | Does `PermissionDenied` still arm the blocked lamp?                                     |
| 8   | Hook wait events: subagent end    | Should `SubagentStop` end a subagent's own wait?                                        |
| 9   | Hook wait events: sandbox alert   | Build the sandbox network approval alert now, or wait for a trigger?                    |
| 10  | Hook wait events: upstream report | File an issue asking Claude Code for a distinct sandbox notification type?              |
| 11  | Recap card image                  | Is a phone image on the recap card worth building?                                      |
| 12  | Recap card image: token placement | May the Moshi upload token ride an authorization header?                                |

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

## 3. Recap card image (task 78)

pns sends a recap card to the phone through Moshi's push notification service. Whether that card can
carry an image was blocked on "no known upload path" until `docs/research/2026-09-moshi-image-cards.md`
found a documented upload endpoint and confirmed, with the Moshi app's own test action, that a rich image
notification actually displays on the phone. The remaining work is entirely a value call plus one
refactor: the recap card is currently posted from inside `replay_missed`
(`pns/crates/pns/src/return_replay.rs:38`), and giving it an image would mean moving card ownership into
the detached `pns recap` child process instead.

**Decision owed:** is one saved tap into Discord worth two pull requests and that refactor (recommended:
no, this stays low priority and unbuilt, since the recap card already exists in Discord and the image
would only shave one tap). If the answer becomes yes later, a second question follows: may the Moshi
upload token ride an `Authorization: Bearer` header, or does it have to stay in the request body the way
`pns/crates/pns-adapters/src/destinations/moshi.rs:14` currently requires.

**What changes once answered:** a no closes task 78 outright as will-not-build. A yes opens the
card-ownership refactor as its own pull request, followed by the image-attaching change once the token
question is settled.
