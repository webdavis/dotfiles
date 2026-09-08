use super::*;

#[test]
fn the_callback_result_is_one_json_line_with_its_delivery_and_commit_facts_unchanged() {
    for status in [Status::Accepted, Status::Degraded, Status::Rejected] {
        let mut expected = receipt(status);
        if status != Status::Accepted {
            expected.diagnostics = vec!["ledger_unavailable".into()];
        }
        let mut output = Vec::new();
        let returned = run(&args(), REQUEST, &mut output, |_| expected.clone()).unwrap();
        assert_eq!(returned, status);
        assert_eq!(decode_result(&output).unwrap(), expected);
        assert_eq!(output.iter().filter(|byte| **byte == b'\n').count(), 1);
        assert_eq!(output.last(), Some(&b'\n'));
    }
}
struct BrokenOutput;
impl Write for BrokenOutput {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed output"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn a_failed_result_write_is_reported_instead_of_claiming_a_receipt_was_written() {
    let error = run(&args(), REQUEST, BrokenOutput, |_| {
        receipt(Status::Accepted)
    })
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
}
#[test]
fn an_unencodable_result_is_reported_without_fabricating_or_truncating_delivery_facts() {
    let mut result = receipt(Status::Accepted);
    result
        .destinations
        .resize(MAX_ITEMS + 1, result.destinations[0].clone());
    let mut output = Vec::new();
    let error = run(&args(), REQUEST, &mut output, |_| result).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(output.is_empty());
}
