# 0011: The shipped configuration template is pinned from outside the crate, and that pin has to leave

Status: accepted as a known debt, with the move scheduled into the configuration step of the refactor.

## The generation chain

`dot_config/pns/private_config.toml.tmpl` in the dotfiles repository is a GENERATED file. It is
`render`'s own output over the committed `dot_config/pns/config-values.toml`, produced by
`just pns-config-render`. A hand edit to the template is a defect, and it is caught by test.

## The one place pns reaches outside its own folder

Two reaches, both into the dotfiles checkout four directories above the crate.

**At compile time**, in the unit-test module of `src/config.rs`:

```rust
const SHIPPED_TEMPLATE: &str =
    include_str!("../../../../dot_config/pns/private_config.toml.tmpl");
const CONFIG_VALUES: &str = include_str!("../../../../dot_config/pns/config-values.toml");
```

Four tests read them: `the_committed_template_is_render_over_the_committed_values_file`,
`every_table_the_operator_runs_is_still_live_in_the_shipped_template`,
`the_resolved_configuration_over_the_committed_values_file_matches_its_snapshot`, and
`the_shipped_template_names_the_entry_and_field_of_every_secret`.

**At run time**, in
`tests/config_render.rs:the_binary_over_the_committed_values_file_writes_the_committed_template_exactly`,
which builds the same paths from `CARGO_MANIFEST_DIR` joined with `../../..`.

The source already records the cost, at the `SHIPPED_TEMPLATE` definition: `cargo test` and
`cargo clippy --all-targets` therefore only work from inside this repository. Run either in the deployed
`~/.local/share/pns` and the error is a "couldn't read" naming a path, which says nothing about why.

## Why the pin exists at all

It is not redundant with the renderer's own tests, and the reason is written into
`tests/config_render.rs`: the wrapper test only checks the banner and the suffix, and the idempotence
test only checks that two runs agree with each other, so a binary that always wrote one fixed body would
pass both. Comparing the binary's actual output against an independently known answer is what proves the
body came from the given input rather than from nowhere.

The same independence argument governs the banner text inside
`the_committed_template_is_render_over_the_committed_values_file`, which is duplicated by hand rather
than imported from `pns-config-render`: importing it would make both sides agree on nothing if that
binary's copy were ever gutted, and the test would still pass. The hand-kept second copy is what turns
that mutant red.

## The move

This is a dotfiles concern, not a pns concern, and a standalone crate cannot carry it.

- pns tests its renderer against fixtures it OWNS, inside the crate.
- The dotfiles repository pins `template == render(values)` with a test under `test/` that runs the BUILT
  renderer, which is the same independence argument satisfied from the other side.
- After the move, pns contains no path that reaches outside its own folder, and `cargo test` works in the
  deployed copy.

Both properties above must survive the move, or the move has traded a real check for a tidier tree:

1. The comparison is against an independently produced answer, never the renderer compared with itself.
1. The expected wrapper text is kept by hand, separately from the binary that emits it.

## Related

The template's five secret actions are a contract in their own right: each is exactly
`{{ (keepassxc "<entry>").<Field> | toToml }}` with no author quotes, and the test stub refuses any
action but that one. That pin is about the template's CONTENT rather than about where the template lives,
so it survives the move unchanged and is documented in `docs/specs/configuration.md`.
