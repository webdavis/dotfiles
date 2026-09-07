# posture

`posture` ports the osquery security tools and ssh-hardening to Rust. The four crates establish the
boundaries for that work. Every command currently prints usage to stderr and exits 2; existing shell
entry points continue running.

| Crate                 | Responsibility                                      |
| --------------------- | --------------------------------------------------- |
| `posture-domain`      | Pure policy without dependencies                    |
| `posture-application` | Use cases and the ports they own                    |
| `posture-adapters`    | Concrete capabilities and the consumed pns protocol |
| `posture-cli`         | Command decoding, composition, and exit codes       |

The member manifests enforce the inward dependencies. The adapters consume the sibling `pns-protocol`
crate by path, which resolves in both the checkout and deployed source trees. Posture has no external
wire protocol of its own.

From the dotfiles checkout, build the installed target with:

```sh
cargo build --release --locked --quiet --bin posture --manifest-path dot_local/share/posture/Cargo.toml
```

`just test-rust` runs the workspace tests, formatting check, and clippy. The builder installs
`~/.local/libexec/posture/posture` after recording the authorized artifact and refreshing its governing
manifest. It refuses artifacts over 8 MiB. A failed refresh restores the prior build record; a failed
installation keeps the new record so the next apply can retry.

`test-baseline.tsv` records each retained Bash test by source path, leaf name, and observed result.
Subsequent port changes map those names to Rust successors or explain their retirement. The existing Bash
cases stay in place until their entry point is cut over.

The dotfiles plan is `docs/superpowers/plans/2026-09-05-posture-port-plan.md`; its source inventory is
`docs/superpowers/specs/2026-09-05-posture-behavioral-specification.md`. These package documents remain
in Git and are excluded from deployment.
