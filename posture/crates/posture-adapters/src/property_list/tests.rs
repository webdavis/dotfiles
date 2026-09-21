use super::*;
use crate::test_sandbox::Sandbox;

/// The same six keys as `binary.plist`, in the other wire format.
const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Program</key>
	<string>/bin/sh</string>
	<key>ProgramArguments</key>
	<array>
		<string>/bin/sh</string>
		<string>/Library/Scripts/run me.sh</string>
	</array>
	<key>Empty</key>
	<string></string>
	<key>KeepAlive</key>
	<true/>
	<key>Nice</key>
	<integer>-5</integer>
	<key>currentProfile</key>
	<string>work</string>
</dict>
</plist>
"#;

fn written(contents: &[u8]) -> (Sandbox, std::path::PathBuf) {
    let sandbox = Sandbox::new("property-list");
    let path = sandbox.join("subject.plist");
    std::fs::write(&path, contents).expect("fixture contents");
    (sandbox, path)
}

#[test]
fn both_wire_formats_answer_every_reading_identically() {
    let binary = written(include_bytes!("binary.plist"));
    for (format, path) in [
        ("binary", binary.1.clone()),
        ("xml", written(XML.as_bytes()).1),
    ] {
        // The sandbox of the XML case lives to the end of the iteration only,
        // so it is read inside the loop body it was made for.
        let list = PropertyList::read(&path).unwrap_or_else(|| panic!("{format} parses"));
        assert!(list.declares("currentProfile"), "{format}");
        assert!(!list.declares("autoLoginUser"), "{format}");
        assert!(
            list.contains_string("/Library/Scripts/run me.sh"),
            "{format}"
        );
        assert!(!list.contains_string("/Library/Scripts/run"), "{format}");
        assert!(!list.contains_string("Program"), "{format}");
        assert_eq!(
            list.raw("Program").as_deref(),
            Some(&b"/bin/sh"[..]),
            "{format}"
        );
        assert_eq!(
            list.raw("ProgramArguments.1").as_deref(),
            Some(&b"/Library/Scripts/run me.sh"[..]),
            "{format}"
        );
        assert_eq!(list.raw("Empty").as_deref(), Some(&b""[..]), "{format}");
        assert_eq!(
            list.raw("KeepAlive").as_deref(),
            Some(&b"true"[..]),
            "{format}"
        );
        assert_eq!(list.raw("Nice").as_deref(), Some(&b"-5"[..]), "{format}");
    }
}

#[test]
fn an_absent_key_path_and_a_container_at_its_end_have_no_raw_value() {
    let (_sandbox, path) = written(XML.as_bytes());
    let list = PropertyList::read(&path).expect("fixture parses");
    for key_path in [
        "Missing",
        "ProgramArguments.9",
        "ProgramArguments.first",
        "ProgramArguments",
        "Program.Nested",
    ] {
        assert_eq!(list.raw(key_path), None, "{key_path}");
    }
}

#[test]
fn a_malformed_or_absent_file_is_never_a_property_list() {
    let (sandbox, malformed) = written(b"not a property list");
    assert!(PropertyList::read(&malformed).is_none());
    let (_empty_sandbox, empty) = written(b"");
    assert!(PropertyList::read(&empty).is_none());
    assert!(PropertyList::read(&sandbox.join("absent.plist")).is_none());
}

#[test]
fn a_root_that_is_not_a_dictionary_declares_nothing_and_still_holds_strings() {
    let (_sandbox, path) =
        written(br#"<plist version="1.0"><array><string>/a</string></array></plist>"#);
    let list = PropertyList::read(&path).expect("an array root parses");
    assert!(!list.declares("currentProfile"));
    assert!(list.contains_string("/a"));
}
