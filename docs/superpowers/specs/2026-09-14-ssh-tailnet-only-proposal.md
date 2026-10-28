# Restricting this Mac's SSH exposure to the tailnet (proposal)

Status: proposal. Nothing in here has been activated, no file on the machine was changed while it was
written, and nothing runs until the operator approves a mechanism and the steps below. Ledger task 75.

Ledger task 75 asks for this Mac's Secure Shell (SSH) exposure to be restricted to the tailnet with a
supported control, with recovery access preserved, and records why the earlier `ListenAddress` proposal
was wrong: launchd owns Remote Login's listening socket. This document establishes that ownership by
measurement, walks the controls that actually exist on this machine, recommends one, and writes out the
activation and rollback the operator would run, each with the check that proves it.

Everything below was measured on dresden on 2026-09-14: macOS 26.2 (Darwin 25.2.0), OpenSSH_10.0p2,
Tailscale 1.102.4. Nothing in the investigation ran with `sudo`, and no system file, service or setting
was changed. The only processes it started were two throwaway sshd instances bound to loopback on scratch
ports, run as the operator's own user against configuration files in a scratch directory, and both were
stopped when the measurements were taken.

## Current state, measured

### launchd owns the listening socket, and sshd never binds one

- `/System/Library/LaunchDaemons/ssh.plist` declares `inetdCompatibility` with `Wait = false` and
  `Instances = 42`, and a `Sockets.Listeners` dictionary whose only address key is
  `SockServiceName = ssh`. There is no `SockNodeName`, which is the launchd key that would bind the
  socket to one address (`launchd.plist(5)`), so the socket is a wildcard on every interface.
- `ls -lO /System/Library/LaunchDaemons/ssh.plist` reports `restricted,compressed`. The file is
  protected by System Integrity Protection, so it cannot be edited even with `sudo`.
- `launchctl print system/com.openssh.sshd` answers without `sudo` (exit 0) and reports
  `state = not running`, `active count = 0`, `runs = 0`, `properties = ... inetd-compatible ...`, and a
  `sockets` block holding two live descriptors (7 and 8) with `passive = 1` and `bonjour = 1`.
- `netstat -an -p tcp` shows `*.22 LISTEN` on `tcp4` and on `tcp6` at the same time, and `ps` shows no
  sshd process at all.
- One inbound connection to `127.0.0.1:22` produced exactly one process, `sshd-session: [accepted]`,
  whose parent process was 1 (launchd). It was gone as soon as the connection closed, and
  `launchctl print` still reported `state = not running` afterwards.
- `launchctl print-disabled system` reports `"com.openssh.sshd" => enabled`, which is Remote Login being
  on. The plist itself carries `Disabled = true`; the override database is what turns it on.

Three consequences follow, and they decide the rest of this document.

1. `ListenAddress` is inert here. sshd is handed an already-accepted socket on standard input and runs
   in the inetd mode `sshd(8)` documents as `-i`; it never calls `bind`, so a directive that names which
   address to bind cannot move anything. The main configuration on disk already has both `ListenAddress`
   lines commented out, and uncommenting them would change nothing a scanner could see. This is the
   error ledger task 75 names, and `ssh-hardening.sh` already documents the same ownership for the
   sibling case of the `Port` directive.
2. A configuration change lands on the next connection, with no restart of anything. There is no
   long-running daemon holding a parsed configuration: each connection gets its own process, and that
   process reads `/etc/ssh/sshd_config` and its includes itself. The existing `--reload` mode and the
   quickstart runbook speak of a running daemon keeping its configuration until sshd restarts, which is
   true of a conventional listener-style sshd and is the safe thing to assume, but on this machine the
   measured shape is per-connection. Activation should still be verified by making a real connection
   rather than by trusting either statement.
3. Remote Login's own toggle is the only switch on that socket. Turning Remote Login off removes the
   listener entirely; nothing short of that narrows the address it listens on.

### What the configuration tree holds today

`/etc/ssh/sshd_config` includes `/etc/ssh/sshd_config.d/*`, which expands to two files:

- `000-ssh-hardening.conf`, the managed public-key-only drop-in: `PasswordAuthentication no`,
  `KbdInteractiveAuthentication no`, `UsePAM yes`, `PubkeyAuthentication yes`, `PermitRootLogin no`,
  `GSSAPIAuthentication no`, `HostbasedAuthentication no`. Nothing else, and no `Match` block anywhere.
- Apple's `100-macos.conf`: `UsePAM yes`, `AcceptEnv LANG LC_*`, and the sftp `Subsystem`.

That drop-in has two producers in this repository and their content is byte-identical today (verified by
`diff`): the `print_config` heredoc in `dot_local/bin/executable_ssh-hardening.sh`, and
`posture/crates/posture-domain/src/ssh_policy/dropin.conf`, which `posture ssh print-config` emits.
`posture ssh install|verify|reload|rollback|print-config|print-path` is the Rust port of the same tool
and is installed at `~/.cargo/bin/posture`.

### What is reachable today

Port 22 is open on every interface this Mac has, over Internet Protocol version 4 (IPv4) and version 6
(IPv6): the local area network address `192.168.1.26`, the tailnet addresses `100.77.192.92` and
`fd7a:115c:a1e0::a738:c05e`, loopback, and any other interface that comes up. Bonjour advertises `ssh`
and `sftp-ssh` on the local network, which is a separate advertisement the plist declares and which no
sshd directive controls. Authentication is public-key-only already, so the exposure at stake is the
pre-authentication surface and the scan-visible open port, not a password-guessing risk.

### What must keep working after any change

- Interactive login from the operator's own devices, over the tailnet.
- The pns phone tap. `~/.ssh/authorized_keys` carries one key whose options are
  `command="/usr/bin/touch /Users/stephen/.local/state/pns/phone-attention.marker",restrict`, and the
  iOS Shortcut's "Run script over SSH" action connects to this Mac and triggers that forced command. The
  Shortcut's target is its `Hostname` global variable, so which network the tap arrives over is whatever
  that variable holds. `pns tap --info` reports the marker's age, which is how a tap is verified.
- `ssh-hardening.sh --reload`'s readiness probe, which is `ssh-keyscan -T <timeout> -p <port> 127.0.0.1`.
  Loopback has to keep answering.

### What already watches these files

`dot_local/libexec/posture/converge/desired/osquery.conf.tmpl` puts `/etc/ssh/sshd_config` and
`/etc/ssh/sshd_config.d/%%` under both file-integrity monitoring and file hashing. Any change to the
drop-in raises a file-integrity event that the alerter will page. That is correct behavior, and it means
activation produces an alert the operator should expect and recognize rather than investigate.

## The options

| Option                             | Removes listener | Address granularity | Recovery cost      |
| ---------------------------------- | ---------------- | ------------------- | ------------------ |
| A. `Match LocalAddress` refusal    | no               | per local address   | edit one file      |
| B. `Match Address` refusal         | no               | per client address  | edit one file      |
| C. macOS application firewall      | no               | none                | one toggle         |
| D. packet filter anchor            | packets only     | full                | root, system files |
| E. Tailscale SSH, Remote Login off | yes              | tailnet identity    | console only       |
| F. `tailscale serve --tcp`         | no               | not applicable      | not applicable     |
| G. Remote Login's own scope        | no               | none (per user)     | one toggle         |

### A. `Match LocalAddress` with `RefuseConnection` (recommended)

sshd 10.0 offers `RefuseConnection`, which `sshd_config(5)` describes as terminating the connection
unconditionally and calls useful only inside a `Match` block. `Match` accepts a `LocalAddress` criterion
with classless inter-domain routing (CIDR) patterns and negation, so one block expresses "refuse every
connection that did not arrive on a tailnet or loopback address":

```
Match LocalAddress "!127.0.0.0/8,!::1,!100.64.0.0/10,!fd7a:115c:a1e0::/48,*"
  RefuseConnection yes
```

`100.64.0.0/10` and `fd7a:115c:a1e0::/48` are the ranges Tailscale documents for every node's IPv4 and
IPv6 addresses, so the block names the tailnet by its address space rather than by this node's two
current addresses, and it keeps working if either address is ever reissued.

Measured with `sshd -G -T -C` against a copy of the real tree (the main configuration with an `Include`
glob over a copy of `100-macos.conf` plus the candidate drop-in):

| Connection sample                                       | `refuseconnection` |
| ------------------------------------------------------- | ------------------ |
| `laddr=127.0.0.1` (loopback)                            | no                 |
| `laddr=100.77.192.92` (tailnet IPv4)                    | no                 |
| `laddr=fd7a:115c:a1e0::a738:c05e` (tailnet IPv6)        | no                 |
| `laddr=192.168.1.26` (local network IPv4)               | yes                |
| `laddr=2600:1700::1` (a sample routable IPv6 address)   | yes                |
| `addr=100.100.1.1,laddr=192.168.1.26` (spoofed source)  | yes                |

The last row is why the criterion is `LocalAddress` and not `Address`: the address a connection arrived
ON is a property of this machine's own interfaces, so claiming a tailnet source address from the local
network does not buy anything.

Three more measurements, because each of them could have made this option wrong:

- The trailing `Match` block does not leak into Apple's file. For both a tailnet sample and a local
  network sample, `usepam yes`, both `AcceptEnv` lines and the sftp `Subsystem` resolved exactly as they
  do with no `Match` block present. Each file in the include glob gets its own `Match` state.
- With no `-C` connection specification at all, the negated single block resolves `refuseconnection no`.
  This matters because the existing verifiers sample with `user=...,host=...,addr=127.0.0.1` and no
  `laddr`, so they stay in the permissive branch and keep judging the seven protected directives the way
  they do today.
- The obvious alternative spelling, an allow block followed by `Match all` with `RefuseConnection yes`,
  resolves identically for every real connection but reports `refuseconnection yes` for a bare
  `sshd -G`. Anything reading the unconditioned resolve would then see a machine that refuses
  everything. The single negated block avoids that reporting artifact, which is why it is the
  recommended spelling.

Cost: the refusal happens inside sshd, so the port stays open and the connection is still accepted at
the transport layer. See "What this does not do" below.

### B. `Match Address` with `RefuseConnection`

Identical mechanism keyed on the client's address instead of the local one. It is weaker for no saving:
a client that claims a `100.64.0.0/10` source address from the local network lands in the allowed
branch, measured above. Rejected.

### C. The macOS application firewall

Enabled on this machine (`socketfilterfw --getglobalstate` reports `State = 1`, block-all disabled). Its
entire option surface is per-application paths, block-all, stealth mode and the two allow-signed
switches; its own usage text has no address, interface or port argument anywhere. It cannot express
"tailnet yes, local network no", and its block-all mode has no interface scope either, so it would take
tailnet SSH down with the rest. Rejected as unable to express the policy.

### D. An anchor in the built-in packet filter (`pf`)

pf is the one mechanism that would drop the packets rather than refuse the session, and it has full
address granularity. The cost is high and lands entirely outside what this repository manages well:
`/etc/pf.conf` is a system file that a macOS update can replace, loading a custom anchor needs an edit
to it plus a root LaunchDaemon to enable pf and load the ruleset, pf's enable state is reference-counted
across the system services that use it (`pfctl -E`/`-X`), and `pfctl -s info` is not even readable
without root (`/dev/pf: Permission denied`), so the state is invisible to the unprivileged posture
poller that watches everything else. Rejected for this task; worth revisiting only if the open port
itself, rather than who can log in, becomes the thing to remove.

### E. Tailscale SSH, with Remote Login off

Tailscale documents a macOS SSH server for the open-source `tailscale` plus `tailscaled` build, which is
exactly what runs here, and enabling it claims port 22 for the Tailscale address only while leaving
`/etc/ssh/sshd_config` and `~/.ssh/authorized_keys` untouched. On its own it therefore restricts
nothing: it adds a tailnet path beside the existing wildcard listener. It only becomes a restriction
when Remote Login is turned off as well, and that combination is the only option here that truly removes
the listening socket.

It also breaks the pns tap. Tailscale SSH authenticates by tailnet identity and never reads
`authorized_keys`, so the forced command that writes the attention marker would stop running, and the
tap is one of the things ledger task 75 requires to still work. Recovery would also narrow to the
physical console and Screen Sharing, since with Remote Login off there is no SSH fallback at all.
Rejected for now, and recorded as the option to revisit if the tap ever stops depending on a forced
command.

### F. `tailscale serve --tcp`

A tailnet-only forwarder to a local port. It cannot help while launchd holds the wildcard socket, since
the only thing it could forward to is the same port 22 that is already reachable everywhere. It would
matter only in a world where sshd listened on loopback alone, which is exactly what launchd's ownership
prevents. Rejected as not applicable.

### G. Remote Login's own scope

Remote Login's access control is the `com.apple.access_ssh` service access control list group (readable
with `dscl . -read /Groups/com.apple.access_ssh`), which scopes by user account. There is no network
scope in the Sharing pane at all. It answers a different question. Rejected.

## Recommendation

Option A. Add one `Match` block to the managed drop-in, keeping the seven existing directives exactly as
they are:

```
Match LocalAddress "!127.0.0.0/8,!::1,!100.64.0.0/10,!fd7a:115c:a1e0::/48,*"
  RefuseConnection yes
```

If approved, the change lands in three places in this repository, in one pull request:

1. `posture/crates/posture-domain/src/ssh_policy/dropin.conf`, the content `posture ssh` installs.
2. The `print_config` heredoc in `dot_local/bin/executable_ssh-hardening.sh`, which is byte-identical to
   that file today and must stay so.
3. `posture/crates/posture-domain/src/ssh_policy/directives/tests.rs`, whose
   `print_config_emits_every_accepted_directive_exactly_once_and_is_pure` asserts the non-comment lines
   are exactly the seven directives and would fail on the added block. The Bash-side guard,
   `test/unit/ssh-hardening-dropin.sh`, asserts each keyword appears exactly once and passes unchanged.

The seven protected directives keep their current values in both branches, so `posture ssh verify` and
`ssh-hardening.sh --verify` keep judging the same thing. `RefuseConnection` is not one of the keywords
their tree scan treats as moving a protected value, and their samples carry no `laddr`, so they resolve
in the permissive branch.

## What this does not do, said plainly

- The listener stays. Port 22 still answers on the local network, a port scan still shows it open, and
  the transport-layer connection is still accepted. This narrows who can log in, not what can connect.
- The refusal is not instant, and this changes how it must be tested. Measured against a throwaway sshd
  instance on a scratch port: a refused connection still gets the version banner, still completes key
  exchange, and is cut at the authentication request, logging
  `administratively prohibited connection for <user> from <address> port <n>`. `ssh-keyscan` against a
  refused address SUCCEEDS, returns a host key and exits 0. So `ssh-keyscan` is not a test of refusal; a
  real `ssh` attempt is, and it reports `Connection closed by <address> port 22`.
- Sessions already open are unaffected. The block is evaluated per connection.
- `PerSourcePenalties` is active with `refuseconnection:10` in its default policy, so a host that is
  refused repeatedly accumulates a penalty and may be dropped earlier and more quietly on later tries.
  Useful against a scanner, and a thing to remember when testing the same denial several times in a row.
- Bonjour keeps advertising `ssh` and `sftp-ssh` on the local network. That is the plist's declaration
  and no sshd directive touches it.
- It says nothing about outbound traffic, and LuLu is an outbound filter, so LuLu is not part of this.

## Activation

Nothing here runs until the operator approves. Every step is the operator's, in order, at the machine.

1. Before anything changes, confirm the pns tap's route. Open the Shortcut and read its `Hostname`
   global variable. If it holds a local network address or a `.local` name, change it to
   `dresden.tail2f2430.ts.net` or `100.77.192.92`, then make a Back Tap and confirm with
   `pns tap --info` that "Last tap" reads a few seconds. Verification: a tap that already arrives over
   the tailnet before the change is what makes a failing tap afterwards mean something.
2. Confirm the tailnet is healthy: `tailscale status --json` reports `BackendState: Running`, and
   `tailscale ip` still prints `100.77.192.92` and `fd7a:115c:a1e0::a738:c05e`. If either address has
   moved outside the two documented ranges, stop: the block would refuse the tailnet as well.
3. Confirm Screen Sharing over the tailnet works, and keep the current SSH session open for the whole
   procedure. The quickstart runbook's reload preamble applies unchanged.
4. Apply the approved change (the operator runs the apply; `posture ssh install` writes the drop-in and
   verifies it, and it never restarts anything).
5. Verify the resolved policy before making any connection:
   `/usr/sbin/sshd -G -T -C 'user=stephen,host=x,addr=203.0.113.1,laddr=192.168.1.26,lport=22'` must
   print `refuseconnection yes`, and the same command with `laddr=100.77.192.92` and with
   `laddr=fd7a:115c:a1e0::a738:c05e` must print `refuseconnection no`. Verification: this is the whole
   policy, resolved by the real binary against the real tree, before any door is closed.
6. Verify allowed reachability over IPv4: from another tailnet device,
   `ssh -4 stephen@100.77.192.92 true` succeeds. A real login, not a banner probe.
7. Verify allowed reachability over IPv6: from the same device,
   `ssh -6 stephen@fd7a:115c:a1e0::a738:c05e true` succeeds.
8. Verify disallowed reachability over IPv4: from a local network device that is NOT on the tailnet,
   `ssh -4 stephen@192.168.1.26 true` must fail with `Connection closed by 192.168.1.26 port 22`. Do
   not use `ssh-keyscan` for this: it succeeds against a refused address.
9. Verify disallowed reachability over IPv6: from the same non-tailnet device, `ssh -6` to this Mac's
   routable or link-local IPv6 address must fail the same way. If the device cannot reach a routable
   IPv6 address, use the link-local form with the interface suffix, and record which address was tried.
10. Verify loopback still answers: `ssh-keyscan -T 5 -p 22 127.0.0.1` returns a host key, and
    `ssh stephen@127.0.0.1 true` succeeds. This is the path `ssh-hardening.sh --reload` probes.
11. Verify the hardening verdict is unchanged: `posture ssh verify` (and `ssh-hardening.sh --verify`)
    exits 0.
12. Verify a real phone tap: make a Back Tap and confirm `pns tap --info` reports a "Last tap" of a few
    seconds and `Fresh: yes`.
13. Expect one file-integrity alert for `/etc/ssh/sshd_config.d/000-ssh-hardening.conf` and confirm it
    names that path and no other. An alert naming a different path is a real finding.

## Rollback

The way back is one file, and no restart is involved: there is no running daemon holding the old
policy, so the next connection reads whatever is on disk.

1. Restore the previous drop-in content: `posture ssh install` from a checkout without the block, or
   directly `sudo <editor> /etc/ssh/sshd_config.d/000-ssh-hardening.conf` and delete the two added
   lines. Deleting the whole file also works and is what the existing recovery text says, but it throws
   away the public-key-only policy with it, so it is the last resort rather than the first move.
2. Verify the policy is gone:
   `/usr/sbin/sshd -G -T -C 'user=stephen,host=x,addr=203.0.113.1,laddr=192.168.1.26,lport=22'` prints
   `refuseconnection no`.
3. Verify reachability is restored: from the local network device used in activation step 8,
   `ssh -4 stephen@192.168.1.26 true` succeeds again, and the IPv6 attempt from step 9 succeeds again.
4. Verify the hardening is still in place: `posture ssh verify` exits 0 and the seven directives are
   unchanged. If the whole file was deleted instead, reinstall it before anything else.
5. Verify the tap still works: one Back Tap, `pns tap --info` reports a fresh marker.
6. Expect a second file-integrity alert for the same path.

## Recovery access when the tailnet is down

The change is a file on disk, and it is evaluated per connection, so a tailnet outage does not lock
anything permanently. The ladder, in the order to try it:

1. Any SSH session already open stays open. This is why step 3 of activation keeps one.
2. The physical console. This Mac is carried, so this is usually available, and it is the only path that
   depends on nothing.
3. Screen Sharing over the tailnet, which is useless in a tailnet outage and is listed only because the
   quickstart runbook's existing recovery text names it for the sshd reload case.
4. If the tailnet is down and no console is available, there is no remote way back in, by design. That
   is the property being bought. The mitigation is that the tailnet daemon on this machine is a system
   daemon that starts before login and survives reboots, and that key expiry is disabled on the node, so
   the tailnet coming back does not need anyone to log in first.

A note on the failure direction: every mistake this block can make is a refusal, not an opening. A typo
in a negated pattern leaves that address unmatched by the negation, so it matches `*` and is refused.
The one spelling that would fail open is a pattern list with no positive `*` term, which activation step
5 catches before any connection is attempted.

## What must be approved before anything runs

1. The mechanism: option A, an sshd `Match LocalAddress` block with `RefuseConnection`, rather than
   turning Remote Login off (option E) or building a packet filter anchor (option D).
2. The scope of "tailnet": the documented Tailscale ranges `100.64.0.0/10` and `fd7a:115c:a1e0::/48`,
   rather than this node's two current addresses.
3. Loopback staying allowed, which `ssh-hardening.sh --reload`'s readiness probe needs.
4. Accepting that the port stays open on the local network and only the login is refused, and that
   Bonjour keeps advertising the service there.
5. The pns tap moving to (or already being on) a tailnet hostname in the Shortcut's `Hostname` global
   variable, since that is the Shortcut field that decides which network the tap arrives over.
6. That activation and rollback are the operator's to run, at the machine, in the order above.
