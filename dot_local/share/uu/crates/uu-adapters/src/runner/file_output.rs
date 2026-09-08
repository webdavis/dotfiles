use std::fs::File;

use super::{SystemRunner, overrun};
use crate::watchdog::{Ended, Spawned, bounded_spawn_to_file};

pub(super) fn run(
    runner: &SystemRunner,
    program: &str,
    args: &[&str],
    input: File,
    output: File,
) -> Result<(), String> {
    let budget = runner.remaining();
    if budget.is_zero() {
        return Err(runner.overrun(&Ended::Stopped, b""));
    }
    match bounded_spawn_to_file(program, args, input, output, budget) {
        Spawned::Ran(finished) => runner.clean_output(finished).map(|_| ()),
        Spawned::NotRunnable(why) => Err(why),
        Spawned::SpawnStuck => Err(overrun::spawn_stuck(
            &runner.lane,
            runner.budget,
            runner.declared,
            program,
        )),
    }
}
