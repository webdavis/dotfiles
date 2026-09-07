# posture

`posture` ports the osquery security tools and ssh-hardening to Rust. The five crates establish the
boundaries for that work. Every command currently prints usage to stderr and exits 2; existing shell
entry points continue running.

| Crate                 | Responsibility                                    |
| --------------------- | ------------------------------------------------- |
| `posture-domain`      | Pure policy without dependencies                  |
| `posture-application` | Use cases and the ports they own                  |
| `posture-protocol`    | Existing cross-process digest record codec        |
| `posture-adapters`    | Concrete capabilities and consumed wire contracts |
| `posture-cli`         | Command decoding, composition, and exit codes     |

The member manifests enforce the inward dependencies. Domain and application depend on neither protocol
crate. Adapters alone consume posture-protocol and the sibling pns-protocol; the cli composes domain,
application and adapters. No lint allowances or serialization dependency are needed for the staged
protocol crate.

PR 2.4 implements posture-protocol's existing six-field digest record codec, preserving the current
unversioned format between alert and digest processes. It adds no queue or envelope. The sibling
pns-protocol retains notification requests and results. Filesystem operations remain adapters, and
derivation, grouping, sanitization and caps remain pure domain policy.

Rust work follows both `/Users/stephen/.agents/skills/clean-code/SKILL.md` and
`/Users/stephen/.agents/skills/clean-code-rust/SKILL.md`; the Rust binding wins all numbers and
mechanisms.

From the dotfiles checkout, build the installed target with:

```sh
cargo build --release --locked --quiet --bin posture --manifest-path dot_local/share/posture/Cargo.toml
```

`just test-rust` runs the workspace tests, formatting check, and clippy. The builder installs
`~/.local/libexec/posture/posture` after recording the authorized artifact and refreshing its governing
manifest through the runner's pipeline-only option. It refuses empty artifacts and artifacts over 8 MiB.
A pre-publication refusal preserves the prior record, binary and tuple. A failed installation keeps the
new record so the next apply can retry. The interval between manifest publication and binary installation
can produce a detectable mismatch; it has no fixed time bound or interruption rollback.

`test-baseline.tsv` records each retained Bash test by source path, leaf name, and observed result.
Subsequent port changes map those names to Rust successors or explain their retirement. The existing Bash
cases stay in place until their entry point is cut over.

The dotfiles plan is `docs/superpowers/plans/2026-09-05-posture-port-plan.md`; its source inventory is
`docs/superpowers/specs/2026-09-05-posture-behavioral-specification.md`. These package documents remain
in Git and are excluded from deployment.
