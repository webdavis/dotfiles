use super::*;

#[test]
fn setup_checks_the_existing_path_before_asking_whether_anyone_can_answer() {
    let mut world = World::new(&[]);
    world.check = Err("existing config refusal".into());
    let (code, errors) = world.run(false);
    assert_eq!(code, 2);
    assert_eq!(errors, ["existing config refusal"]);
    assert_eq!(&*world.trace.borrow(), &["check false"]);
    assert!(world.output.borrow().is_empty());
}

#[test]
fn setup_refuses_a_nonterminal_before_questions_or_rendering() {
    let mut world = World::new(&[]);
    world.terminal = false;
    let (code, errors) = world.run(false);
    assert_eq!(code, 2);
    assert!(errors[0].contains("stdin is not a terminal"));
    assert_eq!(&*world.trace.borrow(), &["check false", "terminal"]);
    assert!(world.output.borrow().is_empty());
}

#[test]
fn setup_abandoned_answers_are_never_composed_or_published() {
    let world = World::new(&[]);
    world
        .answers
        .borrow_mut()
        .push_back(Err("owned read refusal".into()));
    let (code, errors) = world.run(false);
    assert_eq!(code, 2);
    assert_eq!(
        errors,
        ["pns setup: owned read refusal; nothing was written"]
    );
    assert_eq!(world.trace.borrow().len(), 3);
    assert!(world.observed.borrow().is_none());
}

#[test]
fn setup_validates_the_composed_text_before_any_publication() {
    let mut world = World::new(&DECLINED);
    world.validate = Err("invalid fixture config".into());
    let (code, errors) = world.run(true);
    assert_eq!(code, 2);
    assert_eq!(
        errors,
        ["pns setup: what it composed does not load (invalid fixture config); nothing was written"]
    );
    assert_eq!(&world.trace.borrow()[8..], &["compose", "validate"]);
    assert!(
        !world
            .output
            .borrow()
            .iter()
            .any(|line| line.contains("pns setup: wrote"))
    );
}

#[test]
fn setup_publication_failure_is_exit_one_without_a_success_claim() {
    let mut world = World::new(&DECLINED);
    world.publish = Err("owned publication refusal".into());
    let (code, errors) = world.run(false);
    assert_eq!(code, 1);
    assert_eq!(errors, ["pns setup: owned publication refusal"]);
    assert_eq!(
        &world.trace.borrow()[8..],
        &["compose", "validate", "publish false"]
    );
    assert!(
        !world
            .output
            .borrow()
            .iter()
            .any(|line| line.contains("pns setup: wrote"))
    );
}

#[test]
fn setup_publishes_once_after_validation_and_names_the_exact_retained_backup() {
    let mut world = World::new(&DECLINED);
    world.publish = Ok(Some("private backup".into()));
    let (code, errors) = world.run(true);
    assert_eq!(code, 0);
    assert!(errors.is_empty());
    assert_eq!(
        &world.trace.borrow()[8..],
        &["compose", "validate", "publish true"]
    );
    assert_eq!(
        // THE WHOLE OF IT. This used to skip two lines, the preamble the walk
        // said through `say`; the preamble is now the header's own labelled
        // lines, so everything left here is the wizard reporting its own work.
        &*world.output.borrow(),
        &[
            "pns setup: kept the old config at private backup",
            "pns setup: wrote private config"
        ]
    );
    assert_eq!(*world.observed.borrow(), Some(Answers::default()));
    assert!(world.answers.borrow().is_empty());
}

#[test]
fn the_walk_opens_with_a_labelled_header_and_names_every_section_it_asks_under() {
    // WITHOUT SECTIONS THE WALK IS A WALL OF QUESTIONS. An operator part-way
    // through has no way to tell which feature the question in front of them
    // arms, and the credential questions in particular read as unexplained
    // demands for a secret.
    let world = World::new(&DECLINED);
    let _ = world.run(true);
    assert_eq!(
        &*world.furniture.borrow(),
        &[
            "pns setup",
            "label: About",
            "label: Already on",
            "label: Enter",
            "label: Careful",
            // A CONTINUATION ROW, which is what an empty label is for: it hangs
            // under the caveat above it rather than starting a new claim.
            "label: ",
            "section: Phone",
            "section: Hermes",
            "section: Lights",
            "section: Home probe",
            "section: Focus",
            "section: Nagging",
        ]
    );
}
