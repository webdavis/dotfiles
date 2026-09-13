#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunnelReadFailure {
    MissingBinary(String),
    Status(i32),
    Empty,
    InvalidJson,
    UnexpectedShape,
}
pub const FUNNEL_CRITICAL_TITLE: &str = "🔴 **CRITICAL**";

pub fn funnel_read_gap(failure: &FunnelReadFailure) -> String {
    let reason = match failure {
        FunnelReadFailure::MissingBinary(path) => format!("- No tailscale binary found at [{path}]."),
        FunnelReadFailure::Status(code) => format!("- `tailscale funnel status --json` exited {code}, so the funnel state is unreadable."),
        FunnelReadFailure::Empty => "- `tailscale funnel status --json` returned no output, so the funnel state is unreadable.".into(),
        FunnelReadFailure::InvalidJson => "- `tailscale funnel status --json` returned output that is not valid JSON, so the funnel state is unreadable.".into(),
        FunnelReadFailure::UnexpectedShape => "- the funnel status JSON has an unexpected AllowFunnel shape, so the funnel state is unclassifiable and unreadable.".into(),
    };
    blind(&reason)
}
pub fn funnel_corruption_gap() -> String {
    blind(
        "- The funnel monitor state file was CORRUPT (an unreadable baseline); it is being reset.",
    )
}
fn blind(reason: &str) -> String {
    format!(
        "**Tailscale funnel monitoring is BLIND - public-exposure paging is not running.**\n{reason}\n- A funnel could be opened to the PUBLIC internet without a page while this is blind. **Fix now.**"
    )
}
pub fn funnel_persistence_gap() -> String {
    "**Tailscale funnel monitor degraded.**\n- The funnel monitor could not persist its baseline: it cannot advance state, so a stale baseline could mask the next real public exposure and blind the monitor.\n- Check the state directory free space and permissions. **Check now.**".into()
}
