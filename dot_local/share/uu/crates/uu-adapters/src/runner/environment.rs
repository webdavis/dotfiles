use super::SystemRunner;
use crate::watchdog::{Ended, Spawned, bounded_spawn_in};
use std::collections::BTreeMap;
use std::process::Stdio;

pub(super) fn run(
    runner: &SystemRunner,
    program: &str,
    args: &[&str],
    env: &BTreeMap<String, String>,
) -> Result<String, String> {
    let budget = runner.remaining();
    if budget.is_zero() {
        return Err(runner.overrun(&Ended::Stopped, b""));
    }
    match bounded_spawn_in(program, args, Stdio::null(), budget, env) {
        Spawned::Ran(finished) => runner.clean_output(finished),
        Spawned::NotRunnable(why) => Err(why),
        Spawned::SpawnStuck => Err(super::overrun::spawn_stuck(
            &runner.lane,
            runner.budget,
            runner.declared,
            program,
        )),
    }
}
