use super::*;
use posture_application::{AlarmFailed, AlertSignal};

#[derive(Default)]
struct Alarm {
    calls: Vec<(String, String)>,
    fail: bool,
}
impl IndependentAlarm for Alarm {
    fn alarm(&mut self, title: &str, detail: &str) -> Result<(), AlarmFailed> {
        self.calls.push((title.into(), detail.into()));
        if self.fail { Err(AlarmFailed) } else { Ok(()) }
    }
}

fn alert() -> Alert {
    Alert {
        occurrence_id: Some("occurrence-7".into()),
        event: "page",
        signal: AlertSignal::NeedsAttention,
        severity: None,
        occurred_at: Some(1730000000),
        title: "Security finding".into(),
        detail: "line one\nline two".into(),
    }
}

#[test]
fn the_page_reaches_the_banner_whole_and_counts_as_delivered() {
    let mut sut = BannerOnly::new(Alarm::default());
    assert_eq!(sut.submit(&alert()), Submission::Accepted);
    assert_eq!(
        sut.alarm.calls,
        vec![(
            "Security finding".to_string(),
            "line one\nline two".to_string()
        )]
    );
}

#[test]
fn a_banner_that_did_not_fire_leaves_the_finding_unacknowledged() {
    let mut sut = BannerOnly::new(Alarm {
        calls: vec![],
        fail: true,
    });
    assert_eq!(
        sut.submit(&alert()),
        Submission::NotAccepted(SubmissionFailure::Failed)
    );
    assert_eq!(sut.alarm.calls.len(), 1);
}
