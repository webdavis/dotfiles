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
        &world.output.borrow()[2..],
        &[
            "pns setup: kept the old config at private backup",
            "pns setup: wrote private config"
        ]
    );
    assert_eq!(*world.observed.borrow(), Some(Answers::default()));
    assert!(world.answers.borrow().is_empty());
}
