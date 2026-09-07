mod tests {
    use super::super::*;

    #[test]
    fn a_glob_matches_only_what_its_own_two_ends_bracket() {
        // A STAR STANDS FOR ANYTHING INCLUDING NOTHING, and the ends are ends
        // rather than anywhere in the name.
        for (name, matches) in [
            ("checklist-s17.md", true),
            ("checklist-.md", true),
            ("checklist.md", false),
            ("checklist-s17.txt", false),
            ("other-s17.md", false),
        ] {
            assert_eq!(
                matches_glob(name, "checklist-*.md"),
                matches,
                "{name} against checklist-*.md"
            );
        }
        // AND THE TWO ENDS MAY NOT CLAIM THE SAME CHARACTERS. `notes-notes.md`
        // both starts with `notes-` and ends with `-notes.md`, sharing the one
        // hyphen between them, so a matcher asking only those two questions
        // would match a name too short to hold both ends at once.
        assert!(!matches_glob("notes-notes.md", "notes-*-notes.md"));
        assert!(matches_glob("notes--notes.md", "notes-*-notes.md"));
        // AND A PATTERN WITH NO `*` NAMES ONE FILE, which is the ordinary case
        // of an operator pointing at a single note.
        assert!(matches_glob("notes.md", "notes.md"));
        assert!(!matches_glob("notes.md.bak", "notes.md"));
    }

    #[test]
    fn a_note_is_judged_by_the_handle_it_was_opened_on_rather_than_by_its_name() {
        // THE SCAN AND THE READ ARE TWO MOMENTS, and this is the second one.
        // The directory belongs to the operator's other tools, so a name that
        // was a regular file inside the window when it was listed can be a
        // symlink out of that directory, or a file rewritten since, by the time
        // it is opened. CONSTRUCTED BY CALLING THE READ ITSELF, which is that
        // ordering: whatever the scan believed, this is what the read is handed.
        let directory = std::env::temp_dir().join(format!(
            "pns-note-read-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos())
        ));
        std::fs::create_dir_all(&directory).expect("the scratch directory");
        let planted = |name: &str, at: Duration| {
            let path = directory.join(name);
            std::fs::write(&path, "# a finding\n").expect("the note");
            std::fs::File::options()
                .write(true)
                .open(&path)
                .expect("the note")
                .set_modified(std::time::UNIX_EPOCH + at)
                .expect("the note's clock");
            path
        };

        let inside = planted("checklist-inside.md", Duration::from_secs(1500));
        assert_eq!(
            read_note(&inside, 1000, 2000).as_deref(),
            Some("# a finding\n")
        );
        // THE WINDOW IS HALF-OPEN AT FULL PRECISION, on `activity_in`'s own
        // rule. Truncating to whole seconds put a note written half a second
        // after the marker outside the window and one written half a second
        // after it closed inside.
        let edge = planted("checklist-edge.md", Duration::from_millis(1_000_500));
        assert!(
            read_note(&edge, 1000, 2000).is_some(),
            "half a second past the near edge is inside the window"
        );
        let past = planted("checklist-past.md", Duration::from_millis(2_000_500));
        assert!(
            read_note(&past, 1000, 2000).is_none(),
            "half a second past the far edge is outside the window"
        );
        // A SYMLINK IS REFUSED RATHER THAN FOLLOWED, so a name swapped after
        // the scan cannot read a file the glob never named. The scan skips
        // links itself; this is what stops the one planted between the two.
        let swapped = directory.join("checklist-swapped.md");
        let _ = std::os::unix::fs::symlink(&inside, &swapped);
        assert!(
            read_note(&swapped, 1000, 2000).is_none(),
            "a symlink planted at a matched name was followed"
        );
        // AND A FILE REWRITTEN SINCE THE SCAN IS REFUSED for the same reason:
        // the clock on the handle is what decides, not the one the scan saw.
        std::fs::write(&inside, "# rewritten after the scan\n").expect("the rewrite");
        std::fs::File::options()
            .write(true)
            .open(&inside)
            .expect("the note")
            .set_modified(std::time::UNIX_EPOCH + Duration::from_secs(9000))
            .expect("the note's clock");
        assert!(
            read_note(&inside, 1000, 2000).is_none(),
            "a note rewritten after the scan was read into the window anyway"
        );
    }
}
