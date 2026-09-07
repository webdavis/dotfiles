use super::{Clock, Duration, Fixture, Observed, RunOutcome, execute};
use uu_domain::LaneVerdict;

#[test]
fn a_registered_nvim_plugins_lane_preserves_a_pending_exit_and_its_output() {
    let fixture = Fixture::new("nvim-plugins-pending");
    let nvim = fixture.stub("printf 'finder: updates available\\n'\nexit 100\n");
    let config = fixture.load(&format!("[lanes.editor]\ntype = \"nvim-plugins\"\nnvim = {nvim:?}\nconfig = \"/fixture/config\"\n"), crate::registrations::LANES).expect("registered Neovim plugin parser");
    let observed = Observed::default();
    assert_eq!(
        execute(
            fixture.home(),
            &config,
            None,
            Clock::new(Duration::ZERO),
            &observed
        ),
        RunOutcome::Completed
    );
    let reports = observed.reports.borrow();
    assert_eq!(reports[0].name, "editor");
    assert_eq!(reports[0].verdict(), LaneVerdict::Pending);
    assert_eq!(reports[0].lines[0], "finder: updates available");
    assert!(fixture.marker().exists());
}

#[test]
fn a_registered_mason_lane_records_the_real_child_failure() {
    let fixture = Fixture::new("nvim-mason-failed");
    let nvim = fixture.stub("printf 'owned Mason install failed\\n'\nexit 1\n");
    let config = fixture.load(&format!("[lanes.editor]\ntype = \"nvim-mason\"\nnvim = {nvim:?}\nconfig = \"/fixture/config\"\n"), crate::registrations::LANES).expect("registered Mason parser");
    let observed = Observed::default();
    assert_eq!(
        execute(
            fixture.home(),
            &config,
            None,
            Clock::new(Duration::ZERO),
            &observed
        ),
        RunOutcome::Completed
    );
    let reports = observed.reports.borrow();
    assert_eq!(reports[0].name, "editor");
    assert_eq!(reports[0].verdict(), LaneVerdict::Failed);
    assert_eq!(reports[0].lines[0], "owned Mason install failed");
    assert!(!fixture.marker().exists());
}

#[test]
fn a_registered_parsers_lane_records_the_real_child_failure() {
    let fixture = Fixture::new("nvim-parsers-failed");
    let nvim = fixture.stub("printf 'owned parser compiler tail\\n'\nexit 1\n");
    let config = fixture.load(&format!("[lanes.editor]\ntype = \"nvim-parsers\"\nnvim = {nvim:?}\nconfig = \"/fixture/config\"\n"), crate::registrations::LANES).expect("registered parsers parser");
    let observed = Observed::default();
    assert_eq!(
        execute(
            fixture.home(),
            &config,
            None,
            Clock::new(Duration::ZERO),
            &observed
        ),
        RunOutcome::Completed
    );
    let reports = observed.reports.borrow();
    assert_eq!(reports[0].name, "editor");
    assert_eq!(reports[0].verdict(), LaneVerdict::Failed);
    assert_eq!(reports[0].lines[0], "owned parser compiler tail");
    assert!(!fixture.marker().exists());
}
