# 0009: A compiled-in destination beats an executable of the same name, unless the channels directory was overridden

Status: accepted. Implemented by `src/channels/mod.rs:native_first`.

## The rule

```rust
pub fn native_first(channels_dir_overridden: bool) -> bool {
    !channels_dir_overridden
}
```

- With the channels directory NOT overridden, compiled-in destinations take precedence, and an executable
  serves only a name that has no compiled-in implementation.
- With the channels directory overridden by a non-empty `PNS_CHANNELS_DIR`, executables win for every
  name.

A blank value does not count as an override.

## Why the switch exists

It is the test seam for the whole delivery path. Pointing the channels directory at a scripted directory
makes every destination observable and inert at once, which is what lets the dispatch acceptance suite
run against stubs while the compiled-in half is covered separately. Without it, testing delivery would
mean either reaching a real destination or threading a substitution through the whole call chain.

It is also the operator's escape hatch: an operator who needs a destination to behave differently on
their machine can shadow it without a rebuild.

## The risk being accepted

An environment variable changes which code delivers a notification. The mitigation is that it is a
single, total predicate with one call site, so what it decides is auditable, and that it takes effect
only when set to a non-empty value.

## Consequence for the refactor

Destination selection becomes a registry of real implementations rather than a switch on a destination
name. This predicate survives as the registry's precedence rule: the composition root registers
compiled-in destinations and discovers executable ones, and this is what decides which registration wins
a contested name. It must not be re-derived at each destination.
