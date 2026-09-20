# Whether the guest account and FileVault have public answers

Status: research, written 2026-09-20 on dresden (macOS 27.0, build 26A428, Apple M1). No Rust changed, no
dependency added, no system setting touched, no apply ran. Every command below is a read, and the two
throwaway C probes and one throwaway cargo project compiled for it were trashed afterwards.

This settles slice 8 of `docs/research/2026-09-20-native-call-sweep.md`. Two rows in that sweep were
decided on the weakest evidence in the document: `fdesetup status` was a "keep" and
`sysadminctl -guestAccount status` a "defer", both because a header search found nothing. A search
finding nothing is not the same claim as "there is no public API", so both were reopened here.

The two call sites are `posture/crates/posture-adapters/src/probes.rs`, in `command_control`, and the
output of each is classified in `posture/crates/posture-domain/src/poll/classify.rs`
(`classify_filevault`, and `classify_messages` over the two guest sentences).

Verdicts, one line each:

- **Guest account: replace.** The answer is a boolean in a property list this process already parses for
  another control, it agrees with `sysadminctl` on the live machine, and it costs 0.12 ms against 26.32
  ms.
- **FileVault: keep, and the keep is final.** Every interface that reports FileVault state is private.
  The two public encryption properties answer a different question, and on this machine both of them
  disagree with `fdesetup` in both directions.

## Measured spawn cost

`python3`, `subprocess.run` with the output captured, N=20 each, back to back on an otherwise loaded
machine.

| Command                            | Median     | p95        | Min        | Max        |
| ---------------------------------- | ---------- | ---------- | ---------- | ---------- |
| `sysadminctl -guestAccount status` | 26.32 ms   | 29.17 ms   | 19.46 ms   | 29.75 ms   |
| `fdesetup status`                  | 93.61 ms   | 112.91 ms  | 72.63 ms   | 119.86 ms  |
| `diskutil apfs list -plist`        | 2356.67 ms | 5563.03 ms | 1462.70 ms | 7131.07 ms |

`diskutil` is measured only because it was the obvious oracle for what the FileVault properties mean. It
is two orders of magnitude worse than the command it would replace, so it is not a candidate for
anything, and it is used below purely as a reference reading.

## Guest account

### What the command answers, and what the preference says

The live comparison, run within the same second:

```
$ /usr/sbin/sysadminctl -guestAccount status
sysadminctl[...] Guest account disabled.

$ defaults read /Library/Preferences/com.apple.loginwindow GuestEnabled
0
```

`plutil -p` of the same file shows `"GuestEnabled" => false` in the root dictionary, and the file is mode
0644 owned by `root:wheel`, so an unprivileged poller reads it without escalation.

### Which interface `sysadminctl` itself uses

Read-only tracing stopped at `otool` and `strings`, as briefed. `otool -L /usr/sbin/sysadminctl` links
`login.framework` and `SystemAdministration.framework` (both private), plus public Foundation,
OpenDirectory and Security. Its own string table contains `com.apple.loginwindow`, the selectors
`isGuestEnabled`, `setGuestEnabled:`, `createGuestAccount` and `isGuestForProtocolEnabled:`, and the
format string `Guest account %@.` that produces the sentence the current classifier matches. The literal
`GuestEnabled` does not appear in the binary (`strings -a | grep -cx GuestEnabled` returns 0), so the key
itself is read inside one of the two private frameworks, which ship only in the dyld shared cache and
have no file on disk to inspect. The evidence is therefore corroborating rather than conclusive: the tool
names the right preference domain and delegates the read, and its answer agrees with the key's value.

### OpenDirectory

OpenDirectory is a public framework, but it has no guest concept: `grep -rniE guest` over
`$(xcrun --show-sdk-path)/System/Library/Frameworks/OpenDirectory.framework` headers returns nothing, and
the record and attribute constants are all generic (`kODRecordTypeUsers` is declared at
`CFOpenDirectory.framework/Versions/A/Headers/CFOpenDirectoryConstants.h:640`). The record route also
fails empirically: with guest disabled, `dscl . -read /Users/Guest` returns
`DS Error: -14136 (eDSRecordNotFound)` and `dscl . -list /Users` lists no Guest at all. An absent record
cannot be told apart from a directory node that failed to answer, which is exactly the distinction a
posture control exists to make. OpenDirectory is out.

### CFPreferences

`CFPreferencesCopyValue` is declared at
`$(xcrun --show-sdk-path)/System/Library/Frameworks/CoreFoundation.framework/Headers/CFPreferences.h:89`,
with `kCFPreferencesAnyHost` at line 26, `kCFPreferencesCurrentHost` at 28 and `kCFPreferencesAnyUser` at
30\. It is a supported way to reach the same value, but it is not the one to take: posture already parses
this exact file in process for the automatic-login control, through
`posture-adapters/src/property_list.rs` and the `plist` crate, reached at `probes.rs:103`. The guest
reading is one more key out of a property list the tick already opens.

### Cost of the replacement

A throwaway cargo project using the same `plist` crate, reading the same path and pulling `GuestEnabled`
as a boolean, N=20:

```
GuestEnabled = Some(false)
median=0.119 ms p95=0.646 ms min=0.047 max=0.646
```

Against a 26.32 ms median spawn, that is the whole control for free, and posture already pays the parse
once per tick for automatic login, so a build slice that reads both keys off one parsed list pays it zero
extra times.

### Verdict: replace

The exact call, in `posture-adapters/src/probes.rs`, is the shape already at line 103:
`PropertyList::read(&self.login_window)` and then the value of `GuestEnabled`. `PropertyList` needs one
accessor for a boolean (`raw` at `property_list.rs:37` already reaches scalars and renders a boolean as
the text `true` or `false`, so a build slice either parses that text or adds a `boolean` accessor beside
it, which is the cleaner of the two). `GuestAccount` then leaves `command_control` entirely, the way
`AutoLogin` already has.

Behaviours a build slice pins:

1. `GuestEnabled` true reads as `Known(Enabled)`, false as `Known(Disabled)`. Verified against
   `sysadminctl` for the false case above; the true case is a fixture, because turning the guest account
   on is a system change this work is not allowed to make.
1. The key absent from an otherwise valid list reads as `Known(Disabled)`. This is the one judgement
   call: the macOS default is off and the key is written when the setting is toggled, but that default is
   not proven by anything measured here, so it is a fixture-pinned decision rather than an observation.
1. The file missing, unreadable, or not a property list reads as `Indeterminate`, matching
   `classify_autologin`'s `None` arm at `classify.rs:53`.
1. A value of the wrong type (a string, a number) reads as `Indeterminate` rather than being coerced.

## FileVault

### No public header reports FileVault state

`grep -rliE "filevault|fdesetup|libcsfde"` over `$(xcrun --show-sdk-path)/System/Library/Frameworks/`
matches six files, and every match is incidental: `IOKit.framework/Headers/pwr_mgt/IOPM.h:276` mentions
FileVault only in prose about whether the key may be kept across standby, and the other five hits are
`.tbd` link stubs, not declarations. `CoreStorage` matches only two Kernel framework headers. There is no
`FileVault.framework` and no `FDE` header in the public SDK; `FileVault.framework` exists on the machine
under `PrivateFrameworks`.

The private route is confirmed from the other end. `otool -L /usr/bin/fdesetup` links
`/usr/lib/libcsfde.dylib`, `/usr/lib/libCoreStorage.dylib`, `/usr/lib/libodfde.dylib` and the private
`APFS.framework`, `DiskManagement.framework`, `EFILogin.framework`, `MobileKeyBag.framework` and
`SystemAdministration.framework`. The SDK ships `usr/lib/libcsfde.tbd`, which exports `_CSFDE*` symbols
(`_CSFDECreateDefaultContext`, `_CSFDEAddVolumeEncryption`, and so on), but `find` over
`$(xcrun --show-sdk-path)/usr/include` turns up no `csfde`, `CoreStorage` or `fvde` header at all. A link
stub without a declaration is not a public API: there is no prototype, no availability annotation and no
contract, which is the same standard that made `csrutil` a verified keep in the sweep.

### The two public encryption properties answer a different question

Two public properties do report encryption, and both are the wrong question:

- `kDADiskDescriptionMediaEncryptedKey` and `kDADiskDescriptionMediaEncryptionDetailKey`, declared at
  `$(xcrun --show-sdk-path)/System/Library/Frameworks/DiskArbitration.framework/Headers/DADisk.h:64` and
  `:65`, both `API_AVAILABLE(macos(10.14.4))`. Apple's own page for the first carries a declaration and
  an availability table and no abstract at all
  (https://developer.apple.com/documentation/diskarbitration/kdadiskdescriptionmediaencryptedkey), so
  there is no documented statement of what it means, and the detail number is not enumerated anywhere in
  the header.
- `kCFURLVolumeIsEncryptedKey` at
  `$(xcrun --show-sdk-path)/System/Library/Frameworks/CoreFoundation.framework/Headers/CFURL.h:1102`, and
  its Foundation twin `NSURLVolumeIsEncryptedKey` at `Foundation.framework/Headers/NSURL.h:610`, both
  `API_AVAILABLE(macosx(10.12))`.

Both were measured against `fdesetup status`, which reports `FileVault is On.` on this machine, using
`diskutil apfs list` as the oracle for what each volume actually is. A C probe calling
`DADiskCopyDescription`, and a second calling `CFURLCopyResourcePropertyForKey`:

| Volume                     | `diskutil` FileVault line | DA `MediaEncrypted` | DA detail | `VolumeIsEncrypted` |
| -------------------------- | ------------------------- | ------------------- | --------- | ------------------- |
| `disk3s1` System           | `Yes (Unlocked)`          | false               | 0         | false               |
| `disk3s5` Data             | `Yes (Unlocked)`          | true                | 2         | false               |
| `disk3s7` (its own volume) | `No (Encrypted at rest)`  | true                | 0         | true                |
| `disk3s6` VM               | `No`                      | false               | 0         | not probed          |

That table contains a counterexample in each direction on one machine with one FileVault state. A volume
whose FileVault line says Yes reports `MediaEncrypted` false, and a volume whose FileVault line says No
reports it true. `VolumeIsEncrypted` is worse: it is false for the volume FileVault protects and true for
the one it does not, so a control built on it would report FileVault off while FileVault is on.

The reason is documented by Apple, not inferred: with FileVault off on an Apple silicon or T2 Mac "the
volume is still encrypted but the volume encryption key is protected only by the hardware UID in the
Secure Enclave", and turning FileVault on changes the key protection to the user's password combined with
that UID (https://support.apple.com/guide/security/volume-encryption-with-filevault-sec4c6dc1b6e/web).
The data is encrypted either way. "Is this media encrypted" and "is FileVault on" are two different
questions on every Mac posture cares about, and only the private interfaces answer the second one.

### What the current classifier would lose

`classify_filevault` at `classify.rs:32` recognises five sentences, three of which carry a transition: on
but needs a restart to finish, off but enabled after the next restart, off but needs a restart to finish.
No boolean property expresses a pending transition at all, so even a property that did correlate would
collapse three readings into two and silently lose the transitional states, which for a posture control
are the interesting ones.

`diskutil apfs list -plist` does report a per-volume `FileVault` boolean distinct from `Encryption`, and
is therefore the only non-private interface that answers the right question, but it is a spawn 25 times
more expensive than the spawn it would replace and it still has no transitional states. It changes
nothing.

### Verdict: keep, final

`fdesetup status` stays. The absence is no longer an unproven search result: the FileVault-specific
interfaces are private by construction (`FileVault.framework` under `PrivateFrameworks`, `libcsfde` with
link stubs and no header), the public encryption properties are measured to disagree with FileVault in
both directions on this machine, and Apple's own security documentation explains why they must. This is a
final keep, not a keep pending better evidence: reopening it needs Apple to publish a new API, not
another search of the same SDK.
