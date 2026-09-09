#[derive(Debug, PartialEq, Eq)]
pub struct WatchdogPage {
    pub title: String,
    pub body: String,
    pub sound: &'static str,
}
pub fn watchdog_page(problems: &[String]) -> Option<WatchdogPage> {
    if problems.is_empty() {
        return None;
    }
    let mut title = "🔴 **CRITICAL**".to_owned();
    if problems.len() > 1 {
        title.push_str(&format!(" ({} issues)", problems.len()));
    }
    let mut body = "**Monitoring is DOWN**".to_owned();
    for problem in problems {
        body.push_str("\n- ");
        body.push_str(problem);
    }
    body.push_str("\n- **Diagnose:** `launchctl list | grep -i osquery`\n- Restart the down component, then re-check.");
    Some(WatchdogPage {
        title,
        body,
        sound: "Sosumi",
    })
}
