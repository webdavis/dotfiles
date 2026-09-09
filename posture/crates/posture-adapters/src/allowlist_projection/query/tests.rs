use super::identity as query_identity;

#[test]
fn query_decimal() {
    assert_eq!(
        query_identity(br#"[{"path":"/private/fixture","program":12.0}]"#),
        Some(("/private/fixture".into(), r#"12.0"#.into()))
    );
}

#[test]
fn query_nan() {
    assert_eq!(
        query_identity(br#"[{"path":"/private/fixture","program":NaN}]"#),
        Some(("/private/fixture".into(), r#""#.into()))
    );
}

#[test]
fn query_infinity() {
    assert_eq!(
        query_identity(br#"[{"path":"/private/fixture","program":Infinity}]"#),
        Some((
            "/private/fixture".into(),
            r#"1.7976931348623157E+308"#.into()
        ))
    );
}

#[test]
fn query_false() {
    assert_eq!(
        query_identity(br#"[{"path":"/private/fixture","program":false}]"#),
        Some(("/private/fixture".into(), r#""#.into()))
    );
}

#[test]
fn query_object() {
    assert_eq!(
        query_identity(
            br#"[{"path":"/private/fixture","program":{"z":NaN,"a":[12.0,true,"\udc00"]}}]"#
        ),
        Some((
            "/private/fixture".into(),
            r#"{
  "z": null,
  "a": [
    12.0,
    true,
    "�"
  ]
}"#
            .into()
        ))
    );
}

#[test]
fn query_array() {
    assert_eq!(
        query_identity(br#"[{"path":"/private/fixture","program":["x",{"z":1,"a":2}]}]"#),
        Some((
            "/private/fixture".into(),
            r#"[
  "x",
  {
    "z": 1,
    "a": 2
  }
]"#
            .into()
        ))
    );
}

#[test]
fn query_duplicate_object_key() {
    assert_eq!(
        query_identity(br#"[{"path":"/private/fixture","program":{"z":1,"a":2,"z":3}}]"#),
        Some((
            "/private/fixture".into(),
            r#"{
  "z": 3,
  "a": 2
}"#
            .into()
        ))
    );
}

#[test]
fn query_projection_uses_only_the_first_row_and_defaults_absent_fields() {
    assert_eq!(
        query_identity(br#"[{"path":"first","program":"one"},{"path":"second","program":"two"}]"#),
        Some(("first".into(), "one".into()))
    );
    assert_eq!(
        query_identity(br#"[{}]"#),
        Some((String::new(), String::new()))
    );
    for input in [b"[]".as_slice(), b"{bad", b"{}", b"[1]"] {
        assert_eq!(query_identity(input), None);
    }
}
