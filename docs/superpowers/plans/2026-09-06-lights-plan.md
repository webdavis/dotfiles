# lights: implementation plan

Companion to `docs/superpowers/specs/2026-09-06-lights-design.md`. Every name below is proposed and needs
confirmation before anything is created.

## The crate

Source at `dot_local/share/lights`, deployed to `~/.local/share/lights`, built at apply time. It is
designed as a standalone package from the first commit: nothing outside its own folder dictates its
shape, and no path inside it reaches outside the folder. That is what lets it move to its own repository
later without a rewrite.

```
dot_local/share/lights/
  Cargo.toml                  workspace root plus the `lights` binary package
  Cargo.lock                  committed; the builder builds --locked
  src/main.rs                 composition root only, under 100 lines
  crates/
    lights-domain/            rotation, brightness, aliases, the Action value. No I/O.
    lights-application/       the five use cases, over the LightController and Notifier ports
    lights-adapters/          HueLightController, PnsNotifier, the settings parser
    lights-cli/               argv parsing, rendering, exit codes
  tests/                      acceptance tests driving the binary against recorded responses
  docs/specs/                 the behavioral specification, travelling with the crate
  docs/decisions/             decision records, likewise
```

The dependency edges, declared in the member manifests and nowhere else, so the compiler enforces them
rather than a review:

```
lights-application -> lights-domain
lights-adapters    -> lights-application, lights-domain
lights-cli         -> lights-adapters, lights-application, lights-domain
```

There is no protocol crate. pns has one because it publishes a wire contract that outside callers depend
on. `lights` publishes nothing: its only outbound contract is the pns producer argv, which belongs to
pns.

`main.rs` reads the settings, constructs the `LightController` the `type` key selects and the `Notifier`
beside it, hands them to the CLI and returns its exit code, and it does nothing else.

### Dependencies

`ureq` with rustls for the bridge calls, `serde_json` for the CLIP payloads, and `toml` with parsing only
for the config. Taken independently of pns, which happens to take the same three, because the tools share
no crate.

### The file-size rule

No handwritten `.rs` file exceeds 500 total lines including its unit tests, with 200 implementation lines
and 300 total as the targets. Production code contains no `unwrap`, `expect` or `panic!`, and
`cargo clippy --workspace --all-targets -- -D warnings` is a gate on every pull request.

## The domain

Four pure pieces, each independently testable with no bridge, no config file and no clock.

**`Rotation`** holds the ordered scene list and the fallback. `next(current)` and `previous(current)`
return the scene to activate. `current` is an `Option<&str>`, and both `None` and a name the rotation
does not hold return the fallback. The backward step is `(index + len - 1) % len`; writing it as
`(index - 1) % len` underflows a `usize`, which is the trap the bash avoids only because bash indexes
arrays from the end on a negative subscript.

**`Brightness`** is a newtype over `u8` with a private field and a smart constructor that clamps to 1
through 100. There is no way to build one outside the range.

**`Aliases`** maps a short name to a room name and passes an unlisted name through unchanged.

**`Action`** is the typed outcome a use case returns: `PowerSet { room, on }`,
`BrightnessSet { room, level }`, `BrightnessStepped { room, direction }`, `SceneSet { room, scene }`,
`Reported { room, on, brightness, scene }`. The CLI renders it for the terminal and the notifier renders
it for pns. Neither the use cases nor the domain build a display string, which is what lets a test assert
what happened without matching on prose.

## The application

Five use cases, each a small struct holding a `&dyn LightController`, a `&dyn Notifier` and the settings,
each returning `Result<Action, LightsError>`:

`TogglePower`, `SetPower`, `AdjustBrightness`, `SetScene`, `ReportStatus`.

They are the only code that sequences a read against a write, and they are tested entirely against the
two recording implementers. `LightsError` maps one to one onto the exit codes in the design.

The application crate owns both trait definitions, because the consumer owns the port. The adapters crate
implements them and depends on the application, never the reverse.

## The adapters

**`HueLightController`** implements `LightController`. It owns one `ureq::Agent` for the process,
memoizes the single `GET /clip/v2/resource` behind the trait's read methods, builds the four PUT bodies,
and turns a transport failure or a non-success status into a `LightControlError`. Certificate
verification is disabled for the bridge's self-signed certificate, with a comment saying why at the line
that does it.

**`PnsNotifier`** implements `Notifier`. It renders an `Action` into a detail string and spawns
`~/.local/libexec/pns/pns` detached with both streams discarded. A missing binary is silence.

**The settings parser** is a free function rather than a type. `settings::parse` reads a TOML string and
refuses an unknown key by name, refuses a `[controller]` table with no `type` or with a `type` nothing
implements, and refuses a missing address or key; `settings::load` is the thin file read above it. There
is no trait and no `ConfigLoader`, because settings are read once in `main.rs` before any use case exists
and are handed down as plain values. Nothing above ever asks where they came from, so there is no seam
for a trait to sit in.

## Everything outside the crate

### The builder

`.chezmoiscripts/run_onchange_after_54-build-lights.sh.tmpl`, modeled on the uu builder at 59 rather than
the pns builder at 58: `lights` has no daemon, so there is no `launchctl kickstart`, no pending marker
and no restart logic. Slot 54 is free, and it sits just ahead of the other build scripts, which hold
slots 55, 57, 58 and 59.

It hashes every `.rs` file, both manifests and the lock through globs, so a new module cannot silently
miss the trigger. It defers with a retry marker when cargo or the deployed source is missing, because
chezmoi records a `run_onchange` script as done on any zero exit and a skipped build has to change the
rendered script to fire again. It builds `--release --locked --quiet --bin lights` and installs with
`/usr/bin/install -m 755`, which replaces through a temporary file and a rename.

Install path: `~/.local/libexec/lights`, settled. The repository rule puts everything a keybinding,
launchd, a hook or a `just` recipe invokes under `libexec`, and pns already sits there despite being
typed by hand too.

### The config template

`dot_config/lights/private_config.toml.tmpl`, deploying to `~/.config/lights/config.toml`. It is
`private_` because it holds the bridge key. Its two secret lines:

```
address = {{ (keepassxc "OpenHue :: API Key (hue-bridge-pro)").UserName | toToml }}
key = {{ (keepassxc "OpenHue :: API Key (hue-bridge-pro)").Password | toToml }}
```

Reusing that entry is settled. It already exists and already carries both halves, and the pns config
template reads the same two fields from it, so a second entry would mean one bridge credential in two
places to rotate. This adds a fifteenth target to the set that needs KeePassXC unlocked at apply time,
and `CLAUDE.md` names that set, so it is updated in the same pull request.

Unlike the pns template, this one is handwritten. pns generates its template from a committed values file
because its config has grown five plugin tables with argued prose in the comments; `lights` has five
short tables and no renderer, and building one would be scaffolding for a second config that does not
exist.

### `.chezmoiignore`

Three source-only paths: `.local/share/lights/target`, `.local/share/lights/docs` and the committed test
fixtures. Then three entries in the darwin-conditional block, because the whole tool is macOS only:
`.local/libexec/lights`, `.local/share/lights` and `.config/lights`.

### The justfile

`test-rust` gains the lights manifest, alongside the fmt and clippy lines pns and uu already get:

```
cargo test --locked --workspace --manifest-path dot_local/share/lights/Cargo.toml
cargo fmt --all --check --manifest-path dot_local/share/lights/Cargo.toml
cargo clippy --locked --workspace --all-targets --manifest-path dot_local/share/lights/Cargo.toml -- -D warnings
```

### The aerospace bindings

All seven keys point at one tool, and the two that call `openhue` directly stop doing so:

```
f4  = 'exec-and-forget ~/.local/libexec/lights scene "CC Halo Daylight"'
f5  = 'exec-and-forget ~/.local/libexec/lights scene previous'
f6  = 'exec-and-forget ~/.local/libexec/lights scene next'
f7  = 'exec-and-forget ~/.local/libexec/lights scene "CC Halo Amber"'
f8  = 'exec-and-forget ~/.local/libexec/lights brightness down'
f9  = 'exec-and-forget ~/.local/libexec/lights toggle'
f10 = 'exec-and-forget ~/.local/libexec/lights brightness up'
```

`dot_aerospace.toml` is excluded from taplo, so the file's existing visual alignment is preserved by
hand.

### The deletion, and what the operator has to finish by hand

`dot_local/libexec/executable_control-hue-lights.sh` is deleted from the source tree in the last pull
request. Deleting a chezmoi source entry does not delete the deployed file, and this repository builds no
removal mechanisms, so two things survive the apply and the operator removes them:

- `~/.local/libexec/control-hue-lights.sh`
- `~/Library/Logs/smart-lights.log`

Both are listed in the final pull request body as manual steps rather than left to be discovered.

The `openhue` formula stays declared in `.chezmoidata/system_packages_autoinstall.yaml`, settled. After
the cutover nothing in the tree calls it, but removal here is manual by standing rule, and the cutover is
not the moment to also uninstall the fallback.

The scene rotation is the one open operator decision, and no pull request waits on it. The rotation is a
list in the config template, so whichever way it settles the change is one line, landable in any pull
request or after all four.

## The test plan

Everything below runs in under a second and touches no bridge.

The domain tests cover the rotation forward and backward from every index, both wraps, and the fallback
for an unknown current scene and for no current scene. They cover the brightness clamp at 1 and at 100
with one step either side of each, alias resolution for each of the three aliases, and pass-through for a
name the table does not list.

The use cases are tested against `RecordingLightController` and `RecordingNotifier`, in-memory
implementers that record every call and return scripted state. They pin that toggle reads before it
writes and writes the opposite of what it read, that `on` and `off` write without reading, that an
unknown room fails before any write is attempted, and that the notifier runs after the write rather than
before it.

The Hue adapter is tested against recorded bridge responses, JSON captured from the operator's own bridge
with read-only GETs and committed under `tests/fixtures/`. They pin the parsing: a room resolved to its
grouped light, a room whose services hold no grouped light, a scene matched on name and room together,
two rooms holding a scene of the same name, the three `status.active` values, a room with every light off
carrying no dimming, and malformed JSON. The request bodies are asserted separately, as exact JSON
against the four PUT shapes, with no network involved.

One live smoke run belongs to the operator. Agents never write to the bridge, so the first real PUT is
theirs. It has two named steps: walk the seven keys, then time `GET /clip/v2/resource`. That timing is
the one measurement the design is waiting on. Under 150 milliseconds the memoized bulk read stands as
designed; over it, the fallback is two targeted listings, `GET .../room` and `GET .../scene`, which costs
a second round trip on the scene paths and changes no trait and no test. Either way the number goes into
a decision record inside the crate.

Every new assertion is mutation verified by hand: break the subject, watch it go red, restore it, watch
it go green.

## The pull request ladder

There are four. Each one leaves `main` deployable, and each is gated on `just lint-check`,
`just test-rust` and `just ship`.

**PR 1: the crate skeleton, the domain and the settings parser.** The workspace, the four member crates
with their edges, `Rotation`, `Brightness`, `Aliases`, `Action`, and `settings::parse` with its refusals.
No binary is installed and no keybinding moves. Evidence: the domain and settings tests, and the
file-size command.

**PR 2: the two traits, the five use cases and the command line.** `LightController` and `Notifier`, the
five use cases over them, `RecordingLightController` and `RecordingNotifier`, argv parsing, rendering and
the exit-code mapping. The binary builds and runs against the recording implementers, and it is still not
installed anywhere. Evidence: the use case tests, and a usage-error walk over every bad argument shape.

**PR 3: the Hue adapter, the notifier, the builder and the config template.** The bridge client against
the recorded fixtures, the pns producer, the chezmoi builder, the config template with its KeePassXC
lines, the `.chezmoiignore` entries and the justfile recipe. The binary installs on the operator's next
apply. Aerospace still calls the bash script, so nothing the operator uses changes until they say so.
Evidence: the fixture tests, the exact PUT bodies, and the builder's own cargo line run by hand.

**PR 4: the cutover.** The seven aerospace bindings flip, the bash script is deleted from the source
tree, and `CLAUDE.md` is updated for the new tool, the new KeePassXC target and the retired script.
Evidence: the operator's live smoke, which is the only step that writes to the bridge.

PR 3 and PR 4 are deliberately separate. The binary being installed and the keys being repointed are two
different risks, and splitting them means the operator can run `lights status` by hand for as long as
they want before a single key changes behavior.
