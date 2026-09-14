use super::*;

#[test]
fn tree_comparison_reports_all_dimensions_and_case_only_renames() {
    let before = vec![record(b"A", 1, 0o644), record(b"same", 2, 0o644)];
    let after = vec![record(b"a", 1, 0o644), record(b"same", 3, 0o600)];
    assert_eq!(compare_ssh_trees(&before, &before), []);
    assert_eq!(
        compare_ssh_trees(&before, &after),
        [
            SshTreeChange::Disappeared(b"A".to_vec()),
            SshTreeChange::Content(b"same".to_vec()),
            SshTreeChange::Attributes(b"same".to_vec(), attrs(0o644), attrs(0o600)),
            SshTreeChange::Appeared(b"a".to_vec())
        ]
    );
}

#[test]
fn tree_roots_are_sorted_unique_and_listing_failure_is_not_an_empty_scan() {
    assert_eq!(
        ssh_roots(
            Some(b"main".to_vec()),
            Ok(vec![b"z".to_vec(), b"main".to_vec(), b"A".to_vec()])
        ),
        Ok(vec![b"A".to_vec(), b"main".to_vec(), b"z".to_vec()])
    );
    assert_eq!(
        ssh_roots(None, Err(SshTreeRefusal::Unreadable(b"dropins".to_vec()))),
        Err(SshTreeRefusal::Unreadable(b"dropins".to_vec()))
    );
}

#[test]
fn walk_refuses_ambiguous_paths_cycles_depth_and_the_513th_visit() {
    for path in [b"line\nbreak".as_slice(), b"unit\x1fseparator"] {
        assert_eq!(
            SshWalkBudget::default().enter(path, &[]),
            Err(SshTreeRefusal::Path(path.to_vec()))
        );
    }
    assert_eq!(
        SshWalkBudget::default().enter(b"same", &[b"same".to_vec()]),
        Err(SshTreeRefusal::Cycle(b"same".to_vec()))
    );
    let mut budget = SshWalkBudget::default();
    assert!(
        budget
            .enter(b"child", &vec![b"parent".to_vec(); 15])
            .is_ok()
    );
    assert_eq!(
        budget.enter(b"child", &vec![b"parent".to_vec(); 16]),
        Err(SshTreeRefusal::Depth)
    );
    let mut budget = SshWalkBudget::default();
    for _ in 0..512 {
        assert_eq!(budget.enter(b"duplicate", &[]), Ok(()));
    }
    assert_eq!(budget.enter(b"duplicate", &[]), Err(SshTreeRefusal::Visits));
}

#[test]
fn bytes_are_charged_before_parse_including_repeated_reads() {
    let mut budget = SshWalkBudget::default();
    assert_eq!(budget.read(262143), Ok(()));
    assert_eq!(budget.read(1), Ok(()));
    assert_eq!(budget.read(1), Err(SshTreeRefusal::Bytes));
    assert_eq!(
        SshWalkBudget::default().read(usize::MAX),
        Err(SshTreeRefusal::Bytes)
    );
}

fn attrs(mode: u32) -> SshAttributes {
    SshAttributes {
        mode,
        uid: 501,
        gid: 20,
    }
}
fn record(path: &[u8], checksum: u8, mode: u32) -> SshRecord {
    SshRecord {
        path: path.to_vec(),
        attributes: attrs(mode),
        checksum: [checksum; 32],
    }
}
