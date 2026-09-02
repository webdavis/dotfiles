//! Integration coverage for `pns-config-render`, run as the real compiled
//! binary rather than through its library functions: what matters here is
//! what the BINARY writes to disk (or refuses to), not what its internal
//! helpers return.

use std::path::PathBuf;
use std::process::Command;

const BINARY: &str = env!("CARGO_BIN_EXE_pns-config-render");

/// A private scratch directory, named for the test and the process, removed
/// on drop so a failure leaves an identifiable directory behind (the same
/// convention `tests/support/mod.rs`'s `Sandbox` uses).
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "pns-config-render-test-{name}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create scratch dir");
        Scratch { root }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn run(values_path: &std::path::Path, template_path: &std::path::Path) -> std::process::Output {
    Command::new(BINARY)
        .arg(values_path)
        .arg(template_path)
        .output()
        .expect("spawn pns-config-render")
}

/// THE MUTANT THIS PINS: the self-parse step skipped. `render` alone never
/// bounds an integer, so a values file naming an out-of-range `after_secs`
/// renders successfully and would reach disk if nothing parsed it back.
#[test]
fn a_values_file_that_renders_something_the_parser_rejects_is_refused_without_writing() {
    let scratch = Scratch::new("self-parse-refusal");
    let values_path = scratch.path("config-values.toml");
    let template_path = scratch.path("private_config.toml.tmpl");
    // 3601 is one past the nag ceiling `parse_config` enforces (an hour);
    // `render` itself has no notion of that ceiling and writes it live.
    std::fs::write(&values_path, "[nag]\nafter_secs = 3601\n").expect("write values");

    let output = run(&values_path, &template_path);
    assert!(
        !output.status.success(),
        "a self-parse failure must refuse rather than exit 0"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("does not self-parse"),
        "stderr should name the self-parse refusal: {stderr}"
    );
    assert!(
        !template_path.exists(),
        "nothing should be written once the self-parse fails"
    );
}

/// THE MUTANT THIS PINS: the literal-secret refusal check removed (or
/// narrowed to a scan of the rendered text, which `render` would happily
/// pass since it accepts a plain string for any key, secret-bearing or
/// not).
#[test]
fn a_literal_value_at_a_secret_bearing_key_is_refused_without_writing() {
    let scratch = Scratch::new("literal-secret-refusal");
    let values_path = scratch.path("config-values.toml");
    let template_path = scratch.path("private_config.toml.tmpl");
    std::fs::write(
        &values_path,
        "[plugins.hue]\nbridge = \"192.168.1.9\"\nkey = { keepassxc = \"Hue Bridge\", field = \"Password\" }\nrooms = [\"Studio\"]\n",
    )
    .expect("write values");

    let output = run(&values_path, &template_path);
    assert!(
        !output.status.success(),
        "a literal at a secret-bearing key must refuse rather than exit 0"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("plugins.hue.bridge"),
        "stderr should name the offending key: {stderr}"
    );
    assert!(
        !template_path.exists(),
        "nothing should be written once a literal secret is refused"
    );
}

/// THE MUTANT THIS PINS: the roster refusal in `render` disabled or
/// loosened, letting a values entry the layout does not know through.
#[test]
fn an_unknown_values_entry_is_refused_without_writing() {
    let scratch = Scratch::new("unknown-key-refusal");
    let values_path = scratch.path("config-values.toml");
    let template_path = scratch.path("private_config.toml.tmpl");
    std::fs::write(&values_path, "[plugins.mobile]\nnot_a_real_key = true\n")
        .expect("write values");

    let output = run(&values_path, &template_path);
    assert!(!output.status.success());
    assert!(!template_path.exists());
}

/// THE MUTANT THIS PINS: `GENERATED_BANNER` deleted or blanked in the
/// binary. The expected text is a SECOND, independent copy of the banner
/// (also duplicated in `config::tests::the_committed_template_is_render_over_the_committed_values_file`),
/// so a production banner gutted to nothing cannot make this agree with
/// itself.
#[test]
fn the_written_template_starts_with_the_generated_banner_and_the_darwin_wrapper() {
    let scratch = Scratch::new("banner");
    let values_path = scratch.path("config-values.toml");
    let template_path = scratch.path("private_config.toml.tmpl");
    std::fs::write(&values_path, "").expect("write values");

    let output = run(&values_path, &template_path);
    assert!(output.status.success(), "{output:?}");
    let written = std::fs::read_to_string(&template_path).expect("read written template");
    let expected_banner = "\
# GENERATED FILE: this is `render`'s own text over the committed
# `dot_config/pns/config-values.toml`, produced by `just pns-config-render`.
# EDIT THE VALUES FILE AND REGENERATE; a hand edit here fails this test.
{{- if eq .chezmoi.os \"darwin\" }}

";
    assert!(
        written.starts_with(expected_banner),
        "the written template must start with the generated banner: {written}"
    );
    assert!(
        written.ends_with("{{- end }}\n"),
        "the written template must close the darwin conditional: {written}"
    );
}

/// THE MUTANT THIS PINS: any source of nondeterminism in the walk (a
/// `HashMap` iteration order, a clock, a random id): running the binary
/// twice against the SAME values file must write byte-identical templates.
#[test]
fn running_the_binary_twice_against_the_same_values_file_writes_identical_bytes() {
    let scratch = Scratch::new("idempotence");
    let values_path = scratch.path("config-values.toml");
    let first_path = scratch.path("first.tmpl");
    let second_path = scratch.path("second.tmpl");
    std::fs::write(
        &values_path,
        "[plugins.hue]\nrooms = [\"Studio\", \"Kitchen\"]\n[nag]\n",
    )
    .expect("write values");

    let first = run(&values_path, &first_path);
    assert!(first.status.success(), "{first:?}");
    let second = run(&values_path, &second_path);
    assert!(second.status.success(), "{second:?}");

    let first_bytes = std::fs::read(&first_path).expect("read first output");
    let second_bytes = std::fs::read(&second_path).expect("read second output");
    assert_eq!(
        first_bytes, second_bytes,
        "two runs over the same input must match exactly"
    );
}

/// THE MUTANT THIS PINS: the argv usage guard removed, letting a missing
/// argument panic or silently no-op instead of a clean, documented exit.
#[test]
fn missing_arguments_print_usage_and_exit_2() {
    let output = Command::new(BINARY).output().expect("spawn with no args");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("usage:"), "{stderr}");
}
