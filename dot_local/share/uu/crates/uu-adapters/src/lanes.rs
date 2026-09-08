//! Execute the adapter selected by configuration and composition.
//!
//! Continue on failure: one failed step does not stop later subjects, and one
//! failed lane does not stop later lanes. The next attempt is a week away,
//! so aborting at the first failure would discard the other subjects' work.
//! The record reports each outcome.

mod brew;
mod changes;
mod claude_plugins;
mod command;
mod herdr;
mod npm;
mod nvim;
mod skills;
mod spawn;
mod text;
mod uv;
pub use skills::{
    HermesRegistryEntry, SkillsBuildMode, SkillsCandidate, SkillsGenerationStore,
    SkillsPublication, SkillsRecovery, SkillsRoster, exchange_skills_directories,
};

pub use spawn::{CommandRunner, Ran, Verdict};
pub use text::failure_reason;
use uu_domain::LaneReport;

use crate::config::Config;
use uu_domain::RunFacts;

pub trait LaneAdapter: std::fmt::Debug {
    fn parse(label: &str, fields: toml::Table) -> Result<Self, crate::ConfigError>
    where
        Self: Sized;
    fn keys() -> &'static [&'static str]
    where
        Self: Sized;
    fn run(&self, name: &str, facts: &RunFacts, runner: &dyn CommandRunner) -> LaneReport;
    fn bootstrap_capability(&self) -> Option<&dyn crate::BootstrapLane> {
        None
    }
    fn diagnostic_program(&self) -> Option<&str> {
        None
    }
}

/// Run one named lane, or `None` when this config declares none by that name.
/// A name the parser accepted always carries a kind this build knows how to
/// run, because an unrecognized `type` was already refused at load.
pub fn run_lane(
    name: &str,
    config: &Config,
    facts: &RunFacts,
    runner: &dyn CommandRunner,
) -> Option<LaneReport> {
    Some(config.lanes.get(name)?.adapter.run(name, facts, runner))
}

#[cfg(test)]
pub(crate) mod stubs;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse_test_config as parse_config;
    use crate::lanes::stubs::{ScriptedRunner, stub_facts};

    /// The declared lanes, in the order a run reaches them.
    fn names(config: &Config) -> Vec<&str> {
        config.lanes.keys().map(String::as_str).collect()
    }

    // --- the registry ---------------------------------------------------------

    #[test]
    fn a_config_with_no_lane_block_enables_nothing() {
        let config = parse_config("").unwrap();
        assert!(config.lanes.is_empty());
        assert_eq!(
            run_lane("herdr", &config, &stub_facts(), &ScriptedRunner::new(&[])),
            None,
            "a lane with no block must not run just because it was named"
        );
    }

    #[test]
    fn a_lane_block_with_nothing_in_it_turns_the_lane_on() {
        let config = parse_config("[lanes.herdr]\n").unwrap();
        assert_eq!(names(&config), vec!["herdr"]);
        assert!(run_lane("herdr", &config, &stub_facts(), &ScriptedRunner::new(&[])).is_some());
    }

    #[test]
    fn a_lane_this_build_does_not_have_runs_nothing() {
        let config = parse_config("[lanes.herdr]\n").unwrap();
        assert_eq!(
            run_lane("brew", &config, &stub_facts(), &ScriptedRunner::new(&[])),
            None
        );
    }

    #[test]
    fn every_built_in_lane_type_can_be_selected_and_run_and_keeps_its_own_name() {
        // One minimal block per BUILT-IN TYPE (the WEEKDAY_NAMES pattern): a
        // registered type that dispatches to nothing, or that loses the
        // lane's own name along the way, would accept a lane it never truly
        // runs. `command` needs a `run` to be valid at all, so the block is
        // spelled out per fixture rather than derived from the name alone.
        let fixtures: &[(&str, &str, &str)] = &[
            ("brew", "[lanes.brew]\n", "brew"),
            ("command", "[lanes.command]\nrun = [\"x\"]\n", "command"),
            (
                "nvim-mason",
                "[lanes.nvim-mason]\nconfig = \"/fixture/nvim\"\n",
                "nvim-mason",
            ),
            (
                "nvim-parsers",
                "[lanes.nvim-parsers]\nconfig = \"/fixture/nvim\"\n",
                "nvim-parsers",
            ),
            (
                "nvim-plugins",
                "[lanes.nvim-plugins]\nconfig = \"/fixture/nvim\"\n",
                "nvim-plugins",
            ),
            (
                "nvim-smoke-test",
                "[lanes.nvim-smoke-test]\nconfig = \"/fixture/nvim\"\ncache = \"/fixture/cache\"\n",
                "nvim-smoke-test",
            ),
            ("herdr", "[lanes.herdr]\n", "herdr"),
            ("npm", "[lanes.npm]\nbinary = \"/n/npm\"\n", "npm"),
            ("uv", "[lanes.uv]\n", "uv"),
        ];
        for (kind, block, name) in fixtures {
            let config = parse_config(block).unwrap();
            let report = run_lane(name, &config, &stub_facts(), &ScriptedRunner::new(&[]))
                .unwrap_or_else(|| panic!("the roster names `{kind}` but nothing dispatches it"));
            assert_eq!(&report.name, name);
        }
    }

    #[test]
    fn a_run_lanes_report_carries_the_lanes_own_name_not_its_type() {
        // The fixture above names its herdr lane "herdr", which cannot tell a
        // report carrying the ACTUAL name apart from one hardcoding the type's
        // own literal. A lane named something else closes that gap.
        let config = parse_config("[lanes.mine]\ntype = \"herdr\"\n").unwrap();
        let report = run_lane("mine", &config, &stub_facts(), &ScriptedRunner::new(&[])).unwrap();
        assert_eq!(report.name, "mine");
    }

    #[test]
    fn lanes_run_in_name_order_whatever_the_file_order() {
        let config =
            parse_config("[lanes.zeta]\ntype = \"herdr\"\n\n[lanes.alpha]\ntype = \"herdr\"\n")
                .unwrap();
        assert_eq!(names(&config), vec!["alpha", "zeta"]);
    }
    // --- the run_with_input spy -------------------------------------------------

    #[test]
    fn the_scripted_runner_records_every_input_it_was_given() {
        // A command lane's own event only ever reaches a real `SystemRunner`
        // through `run_with_input`; this is what a lane test asserts against
        // instead of a real child process (BRIEF U8, kills args-dropped and
        // stdin-not-written at the lane level, above `SystemRunner` itself).
        let runner = ScriptedRunner::new(&[]);
        runner
            .run_with_input("cmd", &["a", "b"], "first\n")
            .unwrap();
        runner.run_with_input("cmd", &["a", "b"], "second\n").ok();
        assert_eq!(runner.inputs(), vec!["first\n", "second\n"]);
        assert_eq!(
            runner.calls(),
            vec![vec!["cmd", "a", "b"], vec!["cmd", "a", "b"]],
            "the program and its arguments are recorded beside the input"
        );
    }

    #[test]
    fn a_scripted_runner_answers_run_with_input_failure_from_its_failing_set() {
        let runner = ScriptedRunner::new(&[&["cmd", "a"]]);
        let ran = runner.run_with_input("cmd", &["a"], "in\n").unwrap();
        assert_eq!(ran.verdict, Verdict::Failed("exit 1".to_string()));
    }

    #[test]
    fn a_scripted_runner_answers_run_with_input_deferral_from_its_deferring_set() {
        let runner = ScriptedRunner::new(&[]).deferring(&["cmd", "a"]);
        let ran = runner.run_with_input("cmd", &["a"], "in\n").unwrap();
        assert_eq!(ran.verdict, Verdict::Deferred("exit 75".to_string()));
    }
}
