// --- parse_screen_locked ------------------------------------------------

/// The Root dictionary as `/usr/sbin/ioreg -n Root -d1` prints it, trimmed
/// to the neighbourhood of the key.
///
/// The `"IOConsoleLocked"` LINES ARE LIVE SHAPES, each captured on dresden
/// (Darwin 25.2.0) in the state it describes: `= Yes` while the screen was
/// genuinely locked, `= No` while it was not, six leading spaces and all.
/// The `"IOConsoleUsers"` line beside it is the captured one with the
/// nested `CGSSessionScreenIsLocked` WRITTEN IN. That key is the decoy
/// this fixture aims at rather than a claim about what the kernel prints
/// next to a locked console, and what it pins is the parser reading the
/// aggregate off its own line instead of anything in that per-session
/// array.
pub(super) const ROOT_LOCKED: &str = r#"+-o Root  <class IORegistryEntry, id 0x100000100, retain 28>
    {
      "OS Build Version" = "25C56"
      "IOConsoleLocked" = Yes
      "IOConsoleUsers" = ({"kCGSSessionOnConsoleKey"=Yes,"CGSSessionScreenIsLocked"=Yes,"kCGSSessionUserNameKey"="stephen"})
    }
"#;

/// The unlocked shape carrying the same decoy: a parser reaching into the
/// per-session array reads Yes here and reports a locked screen at a desk
/// the operator is sitting at.
///
/// THE DECOY LINE COMES FIRST ON PURPOSE. The parser answers from the
/// FIRST line carrying the key, so a search string loose enough to match
/// the per-session flag has to meet that flag before the aggregate for
/// this fixture to catch it. Put the aggregate back on top and the test
/// below passes for any search string at all.
pub(super) const ROOT_UNLOCKED_WITH_DECOY: &str = r#"+-o Root  <class IORegistryEntry, id 0x100000100, retain 28>
    {
      "OS Build Version" = "25C56"
      "IOConsoleUsers" = ({"kCGSSessionOnConsoleKey"=Yes,"CGSSessionScreenIsLocked"=Yes,"kCGSSessionUserNameKey"="stephen"})
      "IOConsoleLocked" = No
    }
"#;

/// The live chain, recorded on dresden 2026-08-15: one detached
/// `mosh-server`, the herdr client it forked, and that client's pty.
pub(super) const DISCOVERY: [(&str, &str); 3] = [
    ("/usr/bin/pgrep -x mosh-server", "14362\n"),
    ("/usr/bin/pgrep -P 14362", "14363\n"),
    // ps pads its column to a fixed width, which the parser trims.
    ("/bin/ps -o tty= -p 14363", "ttys000 \n"),
];

/// Two instants far enough apart that nothing but the freshest can win.
pub(super) const PUT_DOWN_ATIME: u64 = 1_577_836_800;
pub(super) const IN_HAND_ATIME: u64 = 1_609_459_200;

// --- the session view, against herdr's real answers ---------------------

/// Recorded from a live herdr on 2026-08-13, trimmed to the fields the
/// view needs. A shape change upstream fails these rather than silently
/// reading Unknown forever.
///
/// A workspace's `focused` flag and the `active_tab_id` beside it are the
/// only SESSION-GLOBAL statement of what is on screen. Every `pane`
/// answer is relative to the process that asked.
pub(super) const WORKSPACE_LIST: &str = r#"{"id":"cli:workspace:list","result":{"type":"workspace_list","workspaces":[{"active_tab_id":"wW:t9","focused":true,"label":"dotfiles modernization","workspace_id":"wW"}]}}"#;
/// The same recorded shape with a second workspace ahead of the focused
/// one, which is where the operator is looking.
pub(super) const WORKSPACE_LIST_SECOND_FOCUSED: &str = r#"{"id":"cli:workspace:list","result":{"type":"workspace_list","workspaces":[{"active_tab_id":"wV:t1","focused":false,"label":"other","workspace_id":"wV"},{"active_tab_id":"wW:t9","focused":true,"label":"dotfiles modernization","workspace_id":"wW"}]}}"#;
pub(super) const WORKSPACE_LIST_NONE_FOCUSED: &str = r#"{"id":"cli:workspace:list","result":{"type":"workspace_list","workspaces":[{"active_tab_id":"wW:t9","focused":false,"label":"dotfiles modernization","workspace_id":"wW"}]}}"#;

/// The D4 live capture: tab wW:t9 zoomed onto wW:p3R, taken while the
/// operator held that zoom and the hook fired from wW:p3K.
pub(super) const LAYOUT_ZOOMED_ON_SIBLING: &str = r#"{"id":"cli:pane:layout","result":{"layout":{"focused_pane_id":"wW:p3R","panes":[{"focused":false,"pane_id":"wW:p3K"},{"focused":true,"pane_id":"wW:p3R"}],"tab_id":"wW:t9","zoomed":true},"type":"pane_layout"}}"#;
/// The same tab zoomed onto wW:p3K instead.
pub(super) const LAYOUT_ZOOMED_ON_ORIGIN: &str = r#"{"id":"cli:pane:layout","result":{"layout":{"focused_pane_id":"wW:p3K","panes":[{"focused":true,"pane_id":"wW:p3K"},{"focused":false,"pane_id":"wW:p3R"}],"tab_id":"wW:t9","zoomed":true},"type":"pane_layout"}}"#;
pub(super) const LAYOUT_UNZOOMED: &str = r#"{"id":"cli:pane:layout","result":{"layout":{"focused_pane_id":"wW:p3K","panes":[{"focused":true,"pane_id":"wW:p3K"},{"focused":false,"pane_id":"wW:p3R"}],"tab_id":"wW:t9","zoomed":false},"type":"pane_layout"}}"#;
/// A pane sitting in one of the workspace's other tabs.
pub(super) const LAYOUT_OTHER_TAB: &str = r#"{"id":"cli:pane:layout","result":{"layout":{"focused_pane_id":"wW:p10","panes":[{"focused":true,"pane_id":"wW:p10"}],"tab_id":"wW:tF","zoomed":false},"type":"pane_layout"}}"#;
/// A pane in the OTHER workspace's active tab.
pub(super) const LAYOUT_OTHER_WORKSPACE: &str = r#"{"id":"cli:pane:layout","result":{"layout":{"focused_pane_id":"wV:p1","panes":[{"focused":true,"pane_id":"wV:p1"}],"tab_id":"wV:t1","zoomed":false},"type":"pane_layout"}}"#;

/// WHAT `herdr pane current` ACTUALLY ANSWERS A HOOK, recorded live on
/// 2026-08-13. It resolves "current" from the CALLER'S `HERDR_PANE_ID`,
/// so a hook running inside wW:p3K is told wW:p3K, `focused` flag and
/// all, while the session was really zoomed onto wW:p3R.
pub(super) const PANE_CURRENT_CALLER_RELATIVE: &str = r#"{"id":"cli:pane:current","result":{"pane":{"focused":false,"pane_id":"wW:p3K","tab_id":"wW:t9","workspace_id":"wW"},"type":"pane_current"}}"#;
