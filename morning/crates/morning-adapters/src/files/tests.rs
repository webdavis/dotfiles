use super::*;

#[test]
fn a_missing_file_reports_its_path_and_the_reason() {
    let error = read(Path::new("/nonexistent/morning.log")).unwrap_err();
    assert!(error.starts_with("/nonexistent/morning.log:"), "{error}");
}

#[test]
fn a_directory_with_no_files_says_so() {
    let error = newest(Path::new("/nonexistent/recaps")).unwrap_err();
    assert!(error.contains("/nonexistent/recaps"), "{error}");
}

#[test]
fn head_cuts_a_long_text_and_says_how_much_it_cut() {
    let text = "a\nb\nc\nd\n";
    assert_eq!(
        head(text, 2),
        vec!["a".to_string(), "b".into(), "... 2 more lines".into()]
    );
}

#[test]
fn head_leaves_a_short_text_whole() {
    assert_eq!(head("a\nb\n", 5), vec!["a".to_string(), "b".into()]);
}
