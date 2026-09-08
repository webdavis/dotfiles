use super::*;

#[test]
fn the_doctor_tells_the_truth_about_a_named_focus_in_every_state() {
    // THE DOCTOR'S OTHER THREE FOCUS SENTENCES, pinned. The off state is
    // asserted by the census test through FOCUS_OFF_LINE; these three lived
    // only in a hand-run drill script until a review probe showed the ON
    // sentence could lie without anything going red: a doctor claiming no
    // named Focus is active while one is ON is the exact wrong answer an
    // operator debugging silence would be handed.
    let sandbox = Sandbox::new("doctor-focus-on");
    sandbox.write_config("[focus]\nsilence = [\"Coding\"]\n");
    sandbox.write_focus_store("com.apple.donotdisturb.mode.curlybraces", "Coding");
    let output = doctor_command(&sandbox).output().expect("the engine runs");
    let printed = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(
        printed.contains("a macOS Focus you named is ON"),
        "the ON state never surfaced: {printed}"
    );

    let sandbox = Sandbox::new("doctor-focus-unnamed");
    sandbox.write_config("[focus]\nsilence = [\"Sleep\"]\n");
    sandbox.write_focus_store("com.apple.donotdisturb.mode.curlybraces", "Coding");
    let output = doctor_command(&sandbox).output().expect("the engine runs");
    let printed = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(
        printed.contains("no macOS Focus you named is active"),
        "the quiet state never surfaced: {printed}"
    );
    assert!(
        !printed.contains("is ON"),
        "an unnamed mode read as ON: {printed}"
    );

    // UNREADABLE means the FILE, not its contents: the parser is total, so
    // garbage bytes read as "no mode asserted" (fail-open, the quiet
    // sentence). Only a file the read itself refuses reaches the ignored
    // sentence, so that is what this block builds.
    let sandbox = Sandbox::new("doctor-focus-unreadable");
    sandbox.write_config("[focus]\nsilence = [\"Coding\"]\n");
    let dir = sandbox.path("Library/DoNotDisturb/DB");
    std::fs::create_dir_all(&dir).expect("focus db dir");
    std::fs::write(dir.join("Assertions.json"), b"{}").expect("store");
    let mut forbidden = std::fs::metadata(dir.join("Assertions.json"))
        .expect("meta")
        .permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut forbidden, 0o000);
    std::fs::set_permissions(dir.join("Assertions.json"), forbidden).expect("chmod");
    let output = doctor_command(&sandbox).output().expect("the engine runs");
    let printed = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(
        printed.contains("could not be read, so Focus is being ignored (permission denied)."),
        "the unreadable state never surfaced, or dropped the kind: {printed}"
    );

    // ABSENT IS A DIFFERENT SENTENCE, and this is the machine that has one: a
    // fresh account, or a second Mac, that has never asserted a Focus has no
    // store for macOS to have written. Told "could not be read", that operator
    // goes after a Full Disk Access grant that was never the problem, which is
    // exactly the reading the slice's own drill puts on that line.
    let sandbox = Sandbox::new("doctor-focus-absent");
    sandbox.write_config("[focus]\nsilence = [\"Coding\"]\n");
    let output = doctor_command(&sandbox).output().expect("the engine runs");
    let printed = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(
        printed.contains(
            "no Focus database was found on this machine, so no Focus is being respected"
        ),
        "the absent state never surfaced: {printed}"
    );
    assert!(
        !printed.contains("could not be read"),
        "absent was reported as a store that could not be read: {printed}"
    );
}

#[test]
fn a_mode_catalog_the_doctor_cannot_read_is_said_and_never_reported_as_health() {
    // NAME MATCHING GOES INERT WITH NO CATALOG. The assertion store decides
    // the verdict and the catalog only resolves names, so a catalog that
    // cannot be read leaves a config written the way the template shows it
    // (display names) matching nothing at all. Said with the healthy sentence
    // alone, that state is indistinguishable from being right.
    let sandbox = Sandbox::new("doctor-focus-no-catalog");
    sandbox.write_config("[focus]\nsilence = [\"Coding\"]\n");
    sandbox.write_focus_store("com.apple.donotdisturb.mode.curlybraces", "Coding");
    let catalog = sandbox.path("Library/DoNotDisturb/DB/ModeConfigurations.json");
    let mut forbidden = std::fs::metadata(&catalog).expect("meta").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut forbidden, 0o000);
    std::fs::set_permissions(&catalog, forbidden).expect("chmod");

    let output = doctor_command(&sandbox).output().expect("the engine runs");

    let printed = String::from_utf8_lossy(&output.stdout).to_string();
    let said = "the mode catalog could not be read (permission denied), so no Focus NAME can \
                match and only a raw modeIdentifier still would";
    assert!(
        printed.contains(said),
        "the inert name matching was never said: {printed}"
    );
    // AND THE VERDICT IS STILL THE HONEST ONE: the mode really is not silenced,
    // because the name it was named by resolved to nothing.
    assert!(
        printed.contains("no macOS Focus you named is active"),
        "the state sentence was replaced rather than extended: {printed}"
    );
}
