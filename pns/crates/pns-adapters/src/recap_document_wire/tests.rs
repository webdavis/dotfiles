use super::*;

fn document() -> Node {
    Node::map([
        ("schema", Node::Number(1)),
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
                        ("sessions", Node::rows(&["one".to_string()])),
                    ])]),
                ),
                (
                    "tasks",
                    Node::map([
                        ("rows", Node::rows(&["a: colon".to_string()])),
                        ("more", Node::Number(0)),
                    ]),
                ),
            ]),
        ),
    ])
}

#[test]
fn the_json_form_is_the_documents_own_shape_with_its_keys_in_place() {
    let written = Wire::Json.encode(&document());
    assert_eq!(
        written,
        r#"{"schema":1,"window":{"name":"morning","previous":false},"sections":{"agents":[{"project":"dotfiles","sessions":["one"]}],"tasks":{"rows":["a: colon"],"more":0}}}"#
    );
}

#[test]
fn the_toon_form_nests_by_indentation_and_declares_every_array_length() {
    let written = Wire::Toon.encode(&document());
    assert_eq!(
        written,
        "schema: 1\n\
         window:\n  \
           name: morning\n  \
           previous: false\n\
         sections:\n  \
           agents[1]:\n    \
             - project: dotfiles\n      \
               sessions[1]: one\n  \
           tasks:\n    \
             rows[1]: \"a: colon\"\n    \
             more: 0"
    );
}

#[test]
fn a_string_that_would_read_as_something_else_is_quoted() {
    for (text, written) in [
        ("plain", "plain"),
        ("", "\"\""),
        ("true", "\"true\""),
        ("12", "\"12\""),
        (" padded ", "\" padded \""),
        ("a,b", "\"a,b\""),
        ("says \"hi\"", "\"says \\\"hi\\\"\""),
    ] {
        assert_eq!(scalar_toon(&Node::text(text)), written, "{text:?}");
    }
}

#[test]
fn a_json_mask_and_a_toon_mask_of_the_same_fields_read_the_same() {
    let (json, json_wire) =
        read_mask("{\"schema\": true, \"sections\": {\"tasks\": {\"rows\": true}}}")
            .expect("a json mask");
    let (toon, toon_wire) =
        read_mask("schema: true\nsections:\n  tasks:\n    rows: true\n").expect("a toon mask");
    assert_eq!(json_wire, Wire::Json);
    assert_eq!(toon_wire, Wire::Toon);
    assert_eq!(json, toon);
}

#[test]
fn a_mask_that_is_not_json_after_an_opening_brace_is_refused_rather_than_read_as_empty() {
    assert!(read_mask("{not json").is_none());
}
