use super::{InvalidIdentifier, Name, RequestId, SchemaId};

#[test]
fn a_request_id_at_the_cap_is_accepted_and_one_character_over_is_refused() {
    assert!(RequestId::new("a".repeat(127)).is_ok());
    let at = "a".repeat(128);
    assert_eq!(
        RequestId::new(at.clone()).map(|id| id.as_str().to_string()),
        Ok(at)
    );
    assert_eq!(
        RequestId::new("a".repeat(128 + 1)),
        Err(InvalidIdentifier::TooLong {
            chars: 128 + 1,
            max: 128,
        })
    );
}

#[test]
fn a_request_id_is_visible_ascii_because_it_rides_a_header_and_an_environment_variable() {
    assert_eq!(RequestId::new(""), Err(InvalidIdentifier::Empty));
    for hostile in [
        "with space",
        "tab\there",
        "nl\n",
        "é",
        "\u{7f}",
        "\u{1b}[31m",
    ] {
        assert_eq!(
            RequestId::new(hostile),
            Err(InvalidIdentifier::ForbiddenCharacter),
            "{hostile:?}"
        );
    }
    assert!(RequestId::new("nvim-7f3a9c2e-0001:~!").is_ok());
}

#[test]
fn a_name_at_the_cap_is_accepted_and_one_character_over_is_refused() {
    assert!(Name::new("é".repeat(63)).is_ok());
    let at = "é".repeat(64);
    assert_eq!(
        Name::new(at.clone()).map(|name| name.as_str().to_string()),
        Ok(at)
    );
    assert_eq!(
        Name::new("n".repeat(64 + 1)),
        Err(InvalidIdentifier::TooLong {
            chars: 64 + 1,
            max: 64,
        })
    );
}

#[test]
fn a_name_keeps_unicode_but_refuses_emptiness_and_control_characters() {
    assert!(Name::new("Übersicht·done").is_ok());
    assert_eq!(Name::new(""), Err(InvalidIdentifier::Empty));
    for hostile in ["a\nb", "a\tb", "\u{1b}x", "\u{7f}"] {
        assert_eq!(
            Name::new(hostile),
            Err(InvalidIdentifier::ForbiddenCharacter),
            "{hostile:?}"
        );
    }
}

#[test]
fn identifiers_are_plain_strings_on_the_wire_and_refuse_on_the_way_in() {
    let id: RequestId = serde_json::from_str("\"r-1\"").unwrap();
    assert_eq!(serde_json::to_string(&id).unwrap(), "\"r-1\"");
    assert!(serde_json::from_str::<RequestId>("\"bad id\"").is_err());
    let name: Name = serde_json::from_str("\"nvim\"").unwrap();
    assert_eq!(serde_json::to_string(&name).unwrap(), "\"nvim\"");
    assert!(serde_json::from_str::<Name>("\"\"").is_err());
}

#[test]
fn a_schema_id_is_a_name_and_a_major_version_and_nothing_else_parses() {
    assert_eq!(
        SchemaId::parse("pns.request/1"),
        Some(SchemaId {
            name: "pns.request".to_string(),
            major: 1,
        })
    );
    assert_eq!(
        SchemaId::parse("pns.result/12"),
        Some(SchemaId {
            name: "pns.result".to_string(),
            major: 12,
        })
    );
    for bad in [
        "pns.request",
        "pns.request/",
        "/1",
        "pns.request/one",
        "pns.request/1.2",
        "pns.request/-1",
        "pns.request/+1",
        "",
        "pns.request/1/2",
    ] {
        assert_eq!(SchemaId::parse(bad), None, "{bad:?}");
    }
    assert_eq!(
        SchemaId::parse("pns.request/1").unwrap().to_string(),
        "pns.request/1"
    );
}
