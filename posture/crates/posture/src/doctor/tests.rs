use super::*;
use posture_adapters::NotifyMode;

const PATH: &str = "/private/fixture/home/.config/posture/config.toml";

fn reported(notify: &Notify) -> (u8, String) {
    let mut out = Vec::new();
    let status = report(notify, Path::new(PATH), &mut out);
    (status, String::from_utf8(out).expect("utf-8 output"))
}

#[test]
fn a_delivery_config_this_build_cannot_use_is_named_and_fails_the_doctor() {
    let notify = Notify {
        refusal: Some("unknown variant `banner`".to_string()),
        ..Notify::default()
    };
    let (status, output) = reported(&notify);
    assert_eq!(status, 1, "{output}");
    assert!(output.contains("no page can be delivered"), "{output}");
    assert!(output.contains("unknown variant `banner`"), "{output}");
    assert!(output.contains(PATH), "{output}");
}

#[test]
fn a_usable_config_reports_the_ignored_keys_and_still_passes() {
    let notify = Notify {
        mode: NotifyMode::Off,
        refusal: None,
        warnings: vec!["`jobs.uptime` is not a key this build reads".to_string()],
    };
    let (status, output) = reported(&notify);
    assert_eq!(status, 0, "{output}");
    assert!(output.contains("warning: `jobs.uptime`"), "{output}");
    assert!(output.contains("the delivery config is usable"), "{output}");
}
