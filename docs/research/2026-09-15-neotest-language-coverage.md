# Reaching Rust, Java, Elixir and Zig from neotest, 2026-09-15

Ledger task: "Preserve B95's Rust neotest discovery and duplicate-client follow-up"
(`docs/remaining-work.md:2700`). Builds on `docs/research/2026-09-rust-neotest-disposition.md`, which
root-caused the Rust readiness race and recorded Java and Elixir as withheld because this machine had no
toolchain to test either against.

The operator confirmed on 2026-09-15 that they write Rust, Java, Elixir and Zig, so plan 46b step 2's bar
applies to all four: prove each against a real scratch project or withhold it with the reason. This
session installed the missing toolchains (`openjdk`, `maven`, `elixir`, both declared in
`.chezmoidata/system_packages_autoinstall.yaml`; `zig` and `zls` were already present at 0.16.0) and ran
all four proofs headless, under `nvim --headless --clean -u NONE`, against throwaway scratch projects in
the session scratchpad.

## Verdicts

**Rust: adopted**, per the prior document's Option 1. `mrcjkb/rustaceanvim`'s neotest adapter is wired in
`neotest.lua`, wrapped in a bounded readiness retry, with `lsp.lua`'s `automatic_enable` now excluding
`rust_analyzer` so mason-lspconfig stands down for it. Re-verified this session at
`a968f5133b8b24f481de12f08cd79420d1ace559`: discovery on a scratch two-test crate took 4.94s to become
ready and returned the correct tree (file, `tests` namespace, both tests), and a full `build_spec` / run
/ `results` round trip against one of those tests passed.

**Java: adopted.** `rcasia/neotest-java` at `71354dd2c3f59bcc2301528dfccbbfa2b85bb870`, proven against a
scratch Maven project with one JUnit 5 test (`CalcTest#addsTwoNumbers`). Discovery found the file,
namespace and test node. `build_spec` compiled the classpath through a real `jdtls` LSP client (started
headless via `mfussenegger/nvim-jdtls`'s native `vim.lsp.config` shape, matching what `lsp.lua` now
enables through mason-lspconfig) and ran the JUnit Platform Console Standalone jar; the test passed.
`lsp.lua` gains `jdtls` in `ensure_installed` (mason installs both `jdtls` and the JDK-resolution
wrapper) and a `JAVA_HOME` set pointing at Homebrew's keg-only `openjdk`, since without it jdtls falls
back to macOS's `java` stub. First use on a machine still needs one manual `:NeotestJava setup`, which
downloads and checksums the JUnit Platform Console Standalone jar; that step cannot run unattended.

**Elixir: adopted**, unchanged disposition from the prior document except that the toolchain now exists.
`jfpedroza/neotest-elixir` at `a242aebeaa6997c1c149138ff77f6cacbe33b6fc`, still its newest commit, dated
2025-01-19 (twenty months stale as of this reading, same as the prior document found). Proven against a
scratch `mix new` project: discovery found the doctest and the generated test, and a full `build_spec` /
run / `results` round trip against `elixir -S mix test` passed for both. No LSP or classpath dependency,
so `lsp.lua` gains `elixirls` in `ensure_installed` only because the operator writes Elixir and wants a
language server, not because the adapter needs one.

**Zig: withheld.** `lawrence-laz/neotest-zig` at `de63f3b9a182d374d2e71cf44385326682ec90e7` discovers
positions correctly against a `zig init` scratch project's generated test (`root.zig`,
`basic add functionality`). Running it does not work: the adapter ships its own Zig-side test runner
(`zig/neotest_runner.zig`), and that file is written against a `std` shape Zig 0.16.0 (this machine's
toolchain, and the version `zls` is pinned to) has since changed. `zig test` fails to compile it:

```
/tmp/neotest-zig-probe/zig/neotest_runner.zig:31:24: error: root source file struct 'std' has no member named 'io'
    const stderr = std.io.getStdErr().writer();
/tmp/neotest-zig-probe/zig/neotest_runner.zig:40:18: error: root source file struct 'debug' has no member named 'getStderrMutex'
        std.debug.getStderrMutex().lock();
/tmp/neotest-zig-probe/zig/neotest_runner.zig:151:23: error: root source file struct 'heap' has no member named 'GeneralPurposeAllocator'
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
```

`std.io` was replaced by `std.Io`, `GeneralPurposeAllocator` was renamed, and `getStderrMutex` was
removed, all upstream Zig standard-library changes the adapter's pinned commit predates. Its README
already warns it targets Zig v0.14 and offers no v0.16 branch or tag. Discovery-only is not a usable test
runner, so this stays unpinned in `neotest.lua`, with a comment recording the same failure. Per the
operator's 2026-09-15 ruling on repositories, a `webdavis/` fork or rewrite is a follow-up the operator
approves separately, not part of this PR: rebuilding the runner against current `std` is a small,
well-scoped starting point for it, and the three failing calls above are exactly the ones to fix.

**Update, same day:** the follow-up was taken this session as `webdavis/neotest-zig`, pinned in
`neotest.lua` at `0deabad8bc9d08c7e70a6a3b7c0153ad76a52006`. It is not the runner-rewrite starting point
recorded above: the adapter owns no Zig code at all, so there was no `neotest_runner.zig` to fix.

## What changed on the machine, for the record

- `openjdk`, `maven` and `elixir` installed via Homebrew (elixir pulls `erlang`, `unixodbc` and
  `wxwidgets@3.2` as dependencies), all now declared in `.chezmoidata/system_packages_autoinstall.yaml`.
- Mason installed `jdtls` and `elixir-ls`, both now in `lsp.lua`'s `ensure_installed`.
- `zig` (0.16.0) and Mason's `zls` (0.16.0) were already present and already declared; nothing changed
  for them beyond the failed adapter proof above.
- Nothing else on the machine was changed. The JUnit Platform Console Standalone jar downloaded during
  the Java proof was a manual, one-time step this document already described; the shipped config does not
  automate it, matching `neotest-java`'s own design.

## An unrelated environment defect, found while proving Java

Compiling a fresh tree-sitter parser (the `java` grammar, needed for this proof) failed with
`ld: tapi error: malformed file` against `/Library/Developer/CommandLineTools/SDKs/MacOSX27.0.sdk`, a
system-wide C toolchain break unrelated to any of the four adapters: a bare `clang -o` of a one-line C
file fails the same way, and building against the older `MacOSX26.5.sdk` (also present on disk) succeeds.
This is an environmental blocker, not a neotest defect, and it is outside this PR's scope to fix; it
would affect compiling ANY new tree-sitter parser on this machine until Xcode's Command Line Tools are
repaired or `SDKROOT` is pinned to the working SDK.
