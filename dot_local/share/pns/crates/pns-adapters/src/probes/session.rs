use super::*;
use crate::herdr::view::{parse_focused_tab, parse_layout};

impl<R: CommandRunner> pns_application::SessionViewProbe for SystemProbes<R> {
    /// Two reads, and NEITHER may be caller-relative.
    ///
    /// `herdr pane current` is the trap this builder exists to avoid: it
    /// resolves "current" from the CALLER'S `HERDR_PANE_ID`, and the caller
    /// is always the pane the event fired from, so a view built on it makes
    /// the origin its own focused pane and every desk event self-suppresses.
    /// Measured live on 2026-08-13 (drill D4): with the session zoomed onto
    /// wW:p3R, a hook in wW:p3K was answered wW:p3K.
    ///
    /// So what is on screen comes from `workspace list`, the one session-
    /// global answer herdr gives, and the arrangement comes from the ORIGIN
    /// tab's own layout, addressed by the pane id the event carried. That
    /// layout names the origin's tab as well, so the third call is gone.
    /// Either call failing yields None, which the model reads as Unknown
    /// rather than as "not visible".
    ///
    /// NO CELL, UNLIKE THE OTHER FOUR PROBES ON THIS STRUCT: this has exactly
    /// one production reader (`pns_application::decide`), so "one probe
    /// set is one reading" already holds by call site alone, with nothing to
    /// memoize against. A second production reader would need the same
    /// `OnceCell` the other four carry, to keep that property true once it is
    /// no longer free.
    fn session_view(&self, origin_pane: &str) -> Option<pns_domain::surface::SessionView> {
        let focused_tab = parse_focused_tab(&self.herdr("workspace", &["list"])?)?;
        let layout = parse_layout(&self.herdr("pane", &["layout", "--pane", origin_pane])?)?;
        Some(pns_domain::surface::SessionView {
            origin_tab: layout.tab_id,
            focused_tab,
            focused_pane: layout.focused_pane,
            zoomed: layout.zoomed,
        })
    }
}

impl<R: CommandRunner> SystemProbes<R> {
    /// Resolved through PATH, unlike the system binaries above: the
    /// multiplexer is not at a fixed location, and a context whose PATH does
    /// not carry it reads as unknown, which fails OPEN into a notification.
    fn herdr(&self, subcommand: &str, args: &[&str]) -> Option<String> {
        let mut argv = vec![subcommand];
        argv.extend_from_slice(args);
        self.runner.run("herdr", &argv)
    }
}
