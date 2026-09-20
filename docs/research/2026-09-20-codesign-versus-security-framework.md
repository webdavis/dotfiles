# codesign against Security.framework

Status: measurement, written 2026-09-20. No Rust changed in the product, no dependency added, no apply
ran and no notification was delivered to produce it. This is slice 7 of
`docs/research/2026-09-20-native-call-sweep.md`, the one row that document deferred rather than
verdicted: the framework calls were known to exist, but the enrichment stores `codesign`'s own text and a
signing-information dictionary is a different record, so the sweep asked for a parity measurement and a
record-shape decision before any code.

Every number below was taken on dresden (Apple M1, macOS 27.0) with a throwaway Rust program in the
session scratchpad, since deleted. Every claim about a framework API cites a header under
`$(xcrun --show-sdk-path)/System/Library/Frameworks/Security.framework/Headers` by file and line, or the
shipped source of a crate in the local cargo registry.

Finding in one line: **the framework returns every field the current parse uses, 10 to 20 times faster,
and it returns them as values rather than as text to grep.** Only one of the five branches the classifier
has could not be reproduced on this machine, and it is the rarest one.

## What the text parse actually extracts

`posture-adapters/src/codesign.rs:73` spawns `/usr/bin/codesign -dv --verbose=2 <path>` with stderr
merged into stdout, because `codesign` writes its whole report to stderr. The bytes go through `sanitize`
(null bytes dropped with a diagnostic, trailing newlines trimmed) and reach
`posture_domain::classify_signing` in `posture-domain/src/enrich.rs:22`, which is the only reader of
them. `run` in `posture-adapters/src/command.rs:42` turns a non-zero exit into
`InspectionFailure::Failed`, so an unsigned path arrives at the classifier as `None` rather than as text.

The classifier reads exactly three things out of the report, in this order, and nothing else:

| Read                                                     | Source in the report                          | What it produces                                   |
| -------------------------------------------------------- | --------------------------------------------- | -------------------------------------------------- |
| the substring `not signed`, ASCII case-insensitive       | `code object is not signed at all`            | `UNSIGNED`, untrusted                              |
| the substring `adhoc`, ASCII case-insensitive            | the `flags=0x2(adhoc)` and `Signature=` lines | `ad-hoc signature (untrusted)`                     |
| the first `Authority=` line, up to its first further `=` | the leaf certificate's common name            | `signed: <authority>`, or the no-authority verdict |

The authority is then normalized: `Software Signing` or anything starting with `Apple` collapses to
`Apple`, and a `Developer ID Application: ` prefix is stripped. An empty authority on a signed path is
`signed, no authority (untrusted)`. The verdict string is the whole record: `Enrichment { fact, trust }`
in `posture-domain/src/enrich.rs`, one byte string plus one trust bit. The trust bit becomes the exit
code of `posture enrich` (0 or 10, `enrich.rs:14`), and inside the alerter it becomes
`OwnedSigning { text, untrusted }` in `posture-adapters/src/judge_batch.rs:39`, which promotes a Notice
to a Critical at `posture-domain/src/gate.rs:84` and renders as one page line at
`posture-domain/src/page/fields.rs:131`:

```
- **Signing:** ⚠️ **ad-hoc signature (untrusted)**
```

No other field of the report is read anywhere. `Format`, `CodeDirectory`, `Signature size`,
`Signed Time`, `TeamIdentifier`, `Sealed Resources`, `Internal requirements` and the two lower
`Authority` lines are all parsed away.

## What the framework returns for the same paths

`SecStaticCodeCreateWithPath` (`SecStaticCode.h:79`) takes a `CFURL` and yields a `SecStaticCodeRef`;
`SecCodeCopySigningInformation` (`SecCode.h:528`) with `kSecCSSigningInformation` (`SecCode.h:470`, value
`1 << 1`) fills a `CFDictionary`. The keys used below are `kSecCodeInfoIdentifier` (`SecCode.h:503`),
`kSecCodeInfoTeamIdentifier` (`SecCode.h:512`), `kSecCodeInfoFlags` (`SecCode.h:498`),
`kSecCodeInfoCertificates` (`SecCode.h:478`), `kSecCodeInfoTimestamp` (`SecCode.h:514`) and
`kSecCodeInfoUnique` (`SecCode.h:516`). Each certificate's display name comes from
`SecCertificateCopySubjectSummary` (`SecCertificate.h:101`), which the header recommends over
`SecCertificateCopyCommonName` (`SecCertificate.h:112`).

The measurement program linked `Security.framework` through raw FFI over the `core-foundation` 0.10.1
crate, NOT through `security-framework`. That crate does bind `SecStaticCodeCreateWithPath`
(`security-framework-sys-2.17.0/src/code_signing.rs:89`) and `SecStaticCodeCheckValidity`, but a grep for
`CopySigningInformation` across `security-framework` 3.7.0 and `security-framework-sys` 2.17.0 returns
nothing: the dictionary call has no binding in either crate. Its flag constants are there
(`code_signing.rs:16` onwards), so an adoption can take the crate for the constants and declare the one
missing function itself, or declare both.

Six paths, covering the five kinds the slice asked for plus a Developer ID bundle, which is the case the
authority normalization exists for. `status` is the `OSStatus` of the dictionary call.

| Path                                                 | status | identifier               | team         | flags     | first authority                                             | timestamp | unique |
| ---------------------------------------------------- | ------ | ------------------------ | ------------ | --------- | ----------------------------------------------------------- | --------- | ------ |
| `/bin/ls` (Apple system)                             | 0      | `com.apple.ls`           | absent       | `0x0`     | `macOS Software Signing`                                    | absent    | yes    |
| `/opt/homebrew/bin/jq` (Homebrew)                    | 0      | `jq-5555494488bc3fb8...` | absent       | `0x2`     | none, empty array                                           | absent    | yes    |
| scratchpad `clang` output (ad-hoc)                   | 0      | `adhoc`                  | absent       | `0x20002` | none, empty array                                           | absent    | yes    |
| the same file, signature removed (unsigned)          | 0      | absent                   | absent       | absent    | none, empty array                                           | absent    | no     |
| `/System/Applications/Calculator.app` (Apple bundle) | 0      | `com.apple.calculator`   | absent       | `0x0`     | `macOS Software Signing`                                    | absent    | yes    |
| `/Applications/Ghostty.app` (Developer ID bundle)    | 0      | `com.mitchellh.ghostty`  | `24VZTF6M5V` | `0x10000` | `Developer ID Application: Mitchell Hashimoto (24VZTF6M5V)` | present   | yes    |

The same six paths under `codesign -dv --verbose=2` in the same minute reported
`Authority=macOS Software Signing`, `flags=0x2(adhoc)`, `flags=0x20002(adhoc,linker-signed)`,
`code object is not signed at all`, `Authority=macOS Software Signing`, and
`Authority=Developer ID Application: Mitchell Hashimoto (24VZTF6M5V)` with `TeamIdentifier=24VZTF6M5V`.
The certificate array's subject summaries are the `Authority` lines in order, so the first element is the
line the classifier takes.

### Per field, what replaces what

| Field the classifier uses | Text today                                | Framework equivalent                                                     | Verified                    |
| ------------------------- | ----------------------------------------- | ------------------------------------------------------------------------ | --------------------------- |
| unsigned                  | non-zero exit, or `not signed` in stderr  | the dictionary succeeds with NO `kSecCodeInfoIdentifier` key             | yes, the unsigned row above |
| ad-hoc                    | the substring `adhoc` anywhere            | `kSecCodeInfoFlags & kSecCodeSignatureAdhoc` (`CSCommon.h:337`, `0x2`)   | yes, two rows above         |
| first authority           | first `Authority=` line to its next `=`   | `kSecCodeInfoCertificates[0]` through `SecCertificateCopySubjectSummary` | yes, three rows above       |
| signed with no authority  | a signed report with no `Authority=` line | identifier present, ad-hoc flag clear, certificate array empty or absent | NO, see below               |

The header states the unsigned rule outright rather than leaving it to be inferred: "If the code exists
but is not signed at all, this call will succeed and return a dictionary that does NOT contain the
kSecCodeInfoIdentifier key. This is the recommended way to check quickly whether a code is signed"
(`SecCode.h:341`). `errSecCSUnsigned` (`CSCommon.h:84`) is therefore not the unsigned signal for this
call.

UNVERIFIED: the fourth branch, `signed, no authority (untrusted)`. Producing it needs a signature made
with an identity whose certificate chain the copy call returns empty, and no such identity exists in this
machine's keychain to sign a scratch binary with. Its framework mapping above is read off the structure
of the other three rows, not measured. It is also the branch least likely to be reached: the only signed
path on dresden that took it in the sweep period was none.

Two differences that do NOT affect the classifier but would affect a richer record. `codesign` prints
`Signed Time=` for the Apple system binaries while `kSecCodeInfoTimestamp` is absent for them and present
only for the Developer ID bundle, so the two are not the same field: the dictionary key is the secure
timestamp, not the certificate's own signing date. And `kSecCodeInfoUnique` (the cdhash) has no text
equivalent in `--verbose=2` at all; it appears only at higher verbosity.

## Cost

Both measured in the same process, same program, 3 warmup iterations then 20 timed iterations per path
per method, ambient machine, milliseconds. The native column creates a fresh `SecStaticCodeRef` and
copies a fresh dictionary each iteration, so nothing is cached in process across iterations. The
`codesign` column is a full `std::process::Command` spawn with output captured, which is what the adapter
does today minus the bounded runner's second fork for its cleanup child.

| Path                                  |   n | native median | native p95 | codesign median | codesign p95 | ratio at the median |
| ------------------------------------- | --: | ------------: | ---------: | --------------: | -----------: | ------------------: |
| `/bin/ls`                             |  20 |         1.463 |      3.237 |           35.55 |        53.83 |                 24x |
| `/opt/homebrew/bin/jq`                |  20 |         1.350 |      2.347 |           14.37 |        33.38 |                 11x |
| scratchpad ad-hoc binary              |  20 |         1.673 |      3.941 |           17.28 |        25.95 |                 10x |
| the same file unsigned                |  20 |         0.833 |      1.343 |           17.48 |        20.93 |                 21x |
| `/System/Applications/Calculator.app` |  20 |         1.762 |      2.742 |           33.14 |        58.74 |                 19x |
| `/Applications/Ghostty.app`           |  20 |         4.496 |      8.887 |           40.81 |        59.54 |                  9x |

The saving is 13 ms to 36 ms per enriched path at the median, and the p95 gap is wider than the median
gap on every row. The sweep's estimate for this row was "~50 ms or more per path, unmeasured"; the real
figure is lower than that and still the largest single saving in the posture table.

The bundle is the slowest native case at 4.5 ms because the certificate chain is bigger and the bundle's
`Info.plist` is read to find the main executable. It is not resource validation:
`kSecCSSigningInformation` does not validate anything, which is the same reason it stays cheap on a
536-file bundle.

Enrichment is per finding, not per tick, so the saving lands on the alerter's judge pass: one path per
result row that names one (`posture-adapters/src/judge_batch.rs:74`). A quiet tick enriches nothing. A
tick carrying twenty rows saves roughly half a second.

## Record shape

The dictionary is richer than the text, but the record the enrichment stores is NOT the dictionary and
should not become it. What `posture enrich` writes is one byte string and one trust bit, and four readers
depend on that shape: the exit code (`posture-domain/src/enrich.rs:14`), the gate's Notice promotion
(`posture-domain/src/gate.rs:84`), the page line (`posture-domain/src/page/fields.rs:131`) and the digest
and integrity-page variants that carry the same `Option<&str>` (`posture-domain/src/gate.rs:67`). None of
them reads a field; all of them read the sentence.

**Recommendation on shape: keep `Enrichment` exactly as it is, and move the seam.** Today
`EnrichmentInspection::signing` returns `Result<Vec<u8>, InspectionFailure>` of `codesign`'s text and
`classify_signing` greps it. The adoption changes that one method to return the three facts the
classifier actually needs, and `classify_signing` stops being a parser:

```rust
pub struct SigningFacts {
    pub signed: bool,          // kSecCodeInfoIdentifier present
    pub adhoc: bool,           // kSecCodeInfoFlags & 0x2
    pub authority: Vec<u8>,    // first certificate's subject summary, empty when there is none
}
```

Nothing on the page changes: the same four verdict strings come out of the same normalization, which
stays in the domain crate where it is now. No reader changes. `sanitize`'s null-byte branch stops being
reachable for this field, since a `CFString` converted to UTF-8 carries no interior null, but it stays
for the other inspections that share the helper. The one behavioural change worth naming is that the
domain crate's classifier no longer depends on the exact wording of another program's report, which is
the same improvement the `plutil` and `file` rows of the sweep already took.

A new struct with the whole dictionary (team identifier, cdhash, timestamp, runtime flag) is a different
feature, not this one. It would be cheap to collect, but every field of it would be unread on arrival,
and the page has no line for any of them. YAGNI: take the three, leave the dictionary call able to hand
over more on the day a control asks for it.

**On the deadline wrapper: it is needed, and not for the reason the sweep guessed.** The sweep's Deadline
column said "yes, a signature check touches disk", which is right, and its worry about a notarization
check is not: `SecCodeCopySigningInformation` performs no validation at all. The header says so
(`SecCode.h:345`: "If the signing data for the code is corrupt or invalid, this call may fail or it may
return partial data. To ensure that only valid data is returned ... you must successfully call one of the
CheckValidity functions"), and network access is an opt-in flag on the OTHER call:
`kSecCSAllowNetworkAccess` (`SecStaticCode.h:191`, available since macOS 11.3) "Enables network access
for certificate trust evaluation performed while validating the bundle or its contents"
(`SecStaticCode.h:161`), with `kSecCSEnforceRevocationChecks` and `kSecCSNoNetworkAccess`
(`CSCommon.h:256`, `CSCommon.h:257`) as its companions. All three are `SecStaticCodeCheckValidity` flags
(`SecStaticCode.h:195`), which the adoption does not call. So no notarization check and no revocation
fetch happens on this path.

What remains is ordinary file input and output, and that is enough: the call reads the binary and, for a
bundle, its `Info.plist`, so a path on a stalled network volume or a wedged `osxfs` mount blocks the
calling thread with no bound of its own. Today the spawn is bounded from outside by the ten second budget
in `posture/src/lib.rs:64`, and a native call in process loses that. posture has no bounded-call wrapper
yet and may not borrow pns's, since no workspace may depend on another, so this slice is the one that
carries posture's own copy: a thread plus `recv_timeout`, the shape of
`pns/crates/pns-adapters/src/process/bounded.rs`. A timeout maps to `InspectionFailure::TimedOut`, which
the classifier already turns into `UNSIGNED` by way of the `None` arm, matching what a timed-out
`codesign` does today.

## Recommendation

Build it as a slice. The framework answers every question the text parse asks, does it 9 to 24 times
faster at the median with a wider margin at p95, and the header states the one rule the parse gets from a
string match ("not signed") as an explicit contract. The slice should pin five behaviours: an Apple
system binary classifies as `signed: Apple` through the certificate array; a Homebrew ad-hoc binary
classifies as `ad-hoc signature (untrusted)` through the `0x2` flag and not through a substring; a binary
with its signature removed classifies as `UNSIGNED` through the absent identifier key rather than through
an exit code; a Developer ID bundle classifies as `signed: <team name>` with the
`Developer ID Application: ` prefix stripped; and a path whose read exceeds the new in-process deadline
classifies as `UNSIGNED` rather than hanging the alerter. The fifth is the one that needs the wrapper,
and the wrapper is the only new machinery the slice adds. The record stays `Enrichment`, the page line
stays byte for byte what it is, and `codesign` leaves `posture-adapters` as the last of the nine
replaceable commands in that crate.
