use super::*;

fn document() -> Node {
    Node::map([
        ("schema", Node::Number(SCHEMA)),
        (
            "window",
            Node::map([
                ("name", Node::text("morning")),
                ("previous", Node::Flag(false)),
            ]),
        ),
        (
            "sections",
            Node::map([
                (
                    "agents",
                    Node::List(vec![Node::map([
                        ("project", Node::text("dotfiles")),
                        (
                            "sessions",
                            Node::List(vec![Node::map([
                                ("title", Node::text("the recap engine")),
                                ("branch", Node::text("main")),
                            ])]),
                        ),
                    ])]),
                ),
                (
                    "tasks",
                    Node::map([
                        ("rows", Node::rows(&["one".to_string()])),
                        ("more", Node::Number(0)),
                    ]),
                ),
            ]),
        ),
    ])
}

fn mask(fields: Vec<(&str, Mask)>) -> Mask {
    Mask {
        fields: fields
            .into_iter()
            .map(|(key, nested)| (key.to_string(), nested))
            .collect(),
    }
}

#[test]
fn a_mask_keeps_only_what_it_names_and_a_leaf_takes_its_value_whole() {
    let narrowed = apply(
        &document(),
        &mask(vec![
            ("schema", Mask::default()),
            (
                "sections",
                mask(vec![("tasks", mask(vec![("rows", Mask::default())]))]),
            ),
        ]),
    );
    assert_eq!(
        narrowed,
        Node::map([
            ("schema", Node::Number(SCHEMA)),
            (
                "sections",
                Node::map([(
                    "tasks",
                    Node::map([("rows", Node::rows(&["one".to_string()]))])
                )])
            ),
        ])
    );
}

#[test]
fn a_mask_over_a_list_narrows_every_element() {
    let narrowed = apply(
        &document(),
        &mask(vec![(
            "sections",
            mask(vec![("agents", mask(vec![("project", Mask::default())]))]),
        )]),
    );
    let agents = match &narrowed {
        Node::Map(pairs) => pairs[0].1.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        agents,
        Node::map([(
            "agents",
            Node::List(vec![Node::map([("project", Node::text("dotfiles"))])])
        )])
    );
}

#[test]
fn a_key_that_names_nothing_is_refused_by_its_whole_path() {
    assert_eq!(
        unknown_key(
            &document(),
            &mask(vec![("sections", mask(vec![("task", Mask::default())]))]),
            ""
        ),
        Some("sections.task".to_string())
    );
}

#[test]
fn a_key_inside_a_list_element_is_found_rather_than_refused() {
    assert_eq!(
        unknown_key(
            &document(),
            &mask(vec![(
                "sections",
                mask(vec![(
                    "agents",
                    mask(vec![("sessions", mask(vec![("title", Mask::default())]))])
                )])
            )]),
            ""
        ),
        None
    );
}

#[test]
fn every_key_of_the_document_itself_passes_the_check() {
    let whole = mask(vec![
        ("schema", Mask::default()),
        ("window", Mask::default()),
        ("sections", Mask::default()),
    ]);
    assert_eq!(unknown_key(&document(), &whole, ""), None);
}
