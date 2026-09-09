use super::{Violation, check, check_bytes};
use serde_json::{Map, Value, json};

fn object_with_fields(count: usize) -> Value {
    let mut map = Map::new();
    for index in 0..count {
        map.insert(format!("k{index}"), Value::Null);
    }
    Value::Object(map)
}

/// Objects nested `depth` deep; depth 1 is a bare object.
fn nested_objects(depth: usize) -> Value {
    let mut value = json!({});
    for _ in 1..depth {
        value = json!({ "a": value });
    }
    value
}

/// Arrays nested `depth` deep; depth 1 is a bare array.
fn nested_arrays(depth: usize) -> Value {
    let mut value = json!([]);
    for _ in 1..depth {
        value = json!([value]);
    }
    value
}

#[test]
fn a_request_at_the_byte_cap_passes_and_one_byte_over_is_refused() {
    for bytes in [65_535, 65_536] {
        assert_eq!(check_bytes(&vec![b' '; bytes]), Ok(()), "{bytes} bytes");
    }
    assert_eq!(
        check_bytes(&vec![b' '; 65_536 + 1]),
        Err(Violation::Bytes { bytes: 65_536 + 1 })
    );
}

#[test]
fn an_object_at_the_field_cap_passes_and_one_field_over_is_refused() {
    for fields in [63, 64] {
        assert_eq!(
            check(&object_with_fields(fields)),
            Ok(()),
            "{fields} fields"
        );
    }
    assert_eq!(
        check(&object_with_fields(64 + 1)),
        Err(Violation::Fields { count: 64 + 1 })
    );
}

#[test]
fn text_at_the_character_cap_passes_and_one_character_over_is_refused() {
    // A multibyte character, so the cap is shown to count characters, not bytes.
    for chars in [7_999, 8_000] {
        assert_eq!(
            check(&json!({ "detail": "é".repeat(chars) })),
            Ok(()),
            "{chars} characters"
        );
    }
    let over = "é".repeat(8_000 + 1);
    assert_eq!(
        check(&json!({ "detail": over })),
        Err(Violation::Text { chars: 8_000 + 1 })
    );
}

#[test]
fn a_key_is_text_too_and_is_held_to_the_same_cap() {
    let mut map = Map::new();
    map.insert("k".repeat(8_000 + 1), Value::Null);
    assert_eq!(
        check(&Value::Object(map)),
        Err(Violation::Text { chars: 8_000 + 1 })
    );
}

#[test]
fn an_array_at_the_item_cap_passes_and_one_item_over_is_refused() {
    for items in [63, 64] {
        assert_eq!(
            check(&json!({ "list": vec![Value::Null; items] })),
            Ok(()),
            "{items} items"
        );
    }
    assert_eq!(
        check(&json!({ "list": vec![Value::Null; 64 + 1] })),
        Err(Violation::Items { count: 64 + 1 })
    );
}

#[test]
fn nesting_at_the_depth_cap_passes_and_one_level_deeper_is_refused() {
    for depth in [7, 8] {
        assert_eq!(check(&nested_objects(depth)), Ok(()), "{depth} objects");
    }
    assert_eq!(
        check(&nested_objects(8 + 1)),
        Err(Violation::Depth { depth: 8 + 1 })
    );
    // An array is a level too: a chain of arrays counts the same way.
    for depth in [7, 8] {
        assert_eq!(check(&nested_arrays(depth)), Ok(()), "{depth} arrays");
    }
    assert_eq!(
        check(&nested_arrays(8 + 1)),
        Err(Violation::Depth { depth: 8 + 1 })
    );
}

#[test]
fn the_caps_reach_inside_extensions_and_arrays_alike() {
    let buried = "x".repeat(8_000 + 1);
    let value = json!({ "extensions": { "vendor": [ { "note": buried } ] } });
    assert_eq!(check(&value), Err(Violation::Text { chars: 8_000 + 1 }));
    let mut too_many = Map::new();
    for index in 0..=64 {
        too_many.insert(format!("k{index}"), Value::Null);
    }
    let value = json!({ "extensions": [ Value::Object(too_many) ] });
    assert_eq!(check(&value), Err(Violation::Fields { count: 64 + 1 }));
}

#[test]
fn every_violation_has_a_stable_diagnostic_code() {
    assert_eq!(Violation::Bytes { bytes: 1 }.code(), "bytes_over_cap");
    assert_eq!(Violation::Fields { count: 1 }.code(), "fields_over_cap");
    assert_eq!(Violation::Text { chars: 1 }.code(), "text_over_cap");
    assert_eq!(Violation::Items { count: 1 }.code(), "items_over_cap");
    assert_eq!(Violation::Depth { depth: 1 }.code(), "depth_over_cap");
}
