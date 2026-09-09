# 0011: The dotfiles repository pins its generated configuration outside the pns package

Status: implemented by the configuration-boundary step of the refactor.

The dotfiles repository owns `dot_config/pns/config-values.toml` and its generated
`private_config.toml.tmpl`. `test/unit/pns-config-template.test.sh` runs the actual
`just pns-config-render` recipe into scratch space and compares every byte against the committed
template. The expected file is independent of the renderer being tested. A fixed-body renderer fails this
comparison even if its own wrapper and repeatability checks pass.

The same outer test runs `pns-config-render --check <values-file>`. The command reuses rendering,
secret-action substitution and configuration parsing, validates the registered plugin selection, and
compares the resolved configuration with `tests/fixtures/resolved-config.snapshot`, which stays inside
the pns package. Matching input succeeds without output or writes; a mismatch is refused without changing
the input. Deliberate values changes require reviewing and updating that owned snapshot.

The package's tests no longer include the outer template or values file, and no test constructs an outer
repository path. The renderer's independent banner and footer assertions remain in its binary acceptance
tests. The secret-action grammar test still rejects actions missing `| toToml`, and the renderer tests
retain secret entry/field rendering and literal-secret refusal coverage.

The complete output comparison covers headings and secret-action bytes. The resolved snapshot also
preserves their parsed effects, including each secret's entry and field identity. A separate list of live
headings is declaration consistency and has been retired under the repository's test-scope rule.
