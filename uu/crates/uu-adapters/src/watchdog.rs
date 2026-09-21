//! The watchdog that bounds one lane subject: the spawn, the deadline, and
//! the group kill that enforces it.
//!
//! THREE FILES, three questions. This one SPAWNS, under a bound the caller can
//! give up on; `wait` decides how long the child and its pipes may take and
//! stops the group when they take longer; `drain` collects what the pipes said
//! without ever blocking the watchdog on a read.

use std::fs::File;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

mod drain;
mod wait;

use crate::lanes::Environment;
use crate::runner::prefixed_path;
use drain::Drain;
use wait::{TERM_GRACE, wait_bounded};

pub use wait::Ended;

/// What one bounded spawn produced.
pub struct Finished {
    pub ended: Ended,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub(crate) stdout_error: Option<String>,
}

/// How long past its own budget a bounded spawn may take to answer before the
/// SPAWN itself is judged stuck. `bounded_output`'s own worst case is the
/// budget plus both kill graces, so anything past this means `Command::spawn`
/// never returned.
const SPAWN_SLACK: Duration = Duration::from_secs(8);

/// What came of trying to run one program under a budget.
pub enum Spawned {
    Ran(Finished),
    /// The program could not be started at all, already fit to print.
    NotRunnable(String),
    /// `Command::spawn` ITSELF never returned. No pid exists in that case, so
    /// nothing can be signalled and only the caller giving up bounds it.
    SpawnStuck,
}

/// Run `program` under `budget`, in a process group of its own, collecting
/// both of its pipes.
///
/// THE SPAWN IS INSIDE THE BOUND, on a thread the caller can give up on.
/// `Command::spawn` is synchronous and its exec can block on a filesystem that
/// stopped answering, and until it returns there is no pid to signal, so a
/// watchdog started afterwards would never run and uu's run lock would be held
/// for good. An abandoned thread finishes the job itself: by the time its
/// spawn returns the budget is spent, so the `bounded_output` it then enters
/// stops the group it just created.
///
/// ONLY WHILE UU LIVES, measured: `main` ends at `std::process::exit`, so a
/// spawn whose kernel-side child already exists and that returns after uu has
/// finished leaves it orphaned. Joining instead is the unbounded hang this
/// exists to end, so that orphan is the accepted price.
pub fn bounded_spawn(program: &str, args: &[&str], stdin: Stdio, budget: Duration) -> Spawned {
    spawn_with_environment(program, args, stdin, budget, None, None)
}
pub fn bounded_spawn_in(
    program: &str,
    args: &[&str],
    stdin: Stdio,
    budget: Duration,
    env: &Environment,
) -> Spawned {
    spawn_with_environment(program, args, stdin, budget, Some(env.clone()), None)
}
pub(crate) fn bounded_spawn_to_file(
    program: &str,
    args: &[&str],
    input: File,
    output: File,
    budget: Duration,
) -> Spawned {
    spawn_with_environment(program, args, input.into(), budget, None, Some(output))
}

fn spawn_with_environment(
    program: &str,
    args: &[&str],
    stdin: Stdio,
    budget: Duration,
    env: Option<Environment>,
    output_file: Option<File>,
) -> Spawned {
    if let Some(why) = crate::interruption::refusal() {
        return Spawned::NotRunnable(why);
    }
    let (send, receive) = std::sync::mpsc::channel();
    let owned_program = program.to_string();
    let owned_args: Vec<String> = args.iter().map(|word| (*word).to_string()).collect();
    std::thread::spawn(move || {
        if let Some(why) = crate::interruption::refusal() {
            let _ = send.send(Spawned::NotRunnable(why));
            return;
        }
        let started = Instant::now();
        let mut command = Command::new(&owned_program);
        if let Some(env) = env {
            if env.only_these {
                command.env_clear();
            }
            command.envs(&env.variables);
            if let Some(prefix) = &env.path_prefix {
                // AN ISOLATED ENVIRONMENT PREFIXES ITS OWN PATH, never uu's
                // inherited one: `only_these` exists to keep the child from
                // seeing what uu was started with, and joining against the
                // real environment here would hand it back regardless.
                let inherited = if env.only_these {
                    env.variables.get("PATH").map(std::ffi::OsString::from)
                } else {
                    std::env::var_os("PATH")
                };
                command.env("PATH", prefixed_path(prefix, inherited.as_deref()));
            }
        }
        let spawned = command
            .args(&owned_args)
            .stdin(stdin)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Keep producers in their own group. Cancellation reaches the whole
            // owned group through the watchdog, just like deadline enforcement.
            .process_group(0)
            .spawn();
        // WHAT THE SPAWN ITSELF COST comes off the budget, so a spawn that
        // took half an hour does not hand the child a fresh one.
        let _ = send.send(match spawned {
            Ok(mut child) => Spawned::Ran(bounded_output(
                &mut child,
                budget.saturating_sub(started.elapsed()),
                output_file,
            )),
            Err(error) => Spawned::NotRunnable(format!("could not run {owned_program}: {error}")),
        });
    });
    let mut deadline = Instant::now() + budget + SPAWN_SLACK;
    let mut cancelling = false;
    loop {
        if !cancelling && crate::interruption().is_some() {
            cancelling = true;
            deadline = deadline.min(Instant::now() + SPAWN_SLACK);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        match receive.recv_timeout(remaining.min(Duration::from_millis(25))) {
            Ok(result) => return result,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return Spawned::SpawnStuck,
            Err(_) if remaining.is_zero() => return Spawned::SpawnStuck,
            Err(_) => {}
        }
    }
}

/// Drive `child` to its end inside `budget`, draining both of its pipes.
///
/// THE CHILD MUST ALREADY BE IN A PROCESS GROUP OF ITS OWN (the caller spawns
/// it with `process_group(0)`), because the kill below is aimed at that group
/// and a shared one would take the caller with it.
fn bounded_output(child: &mut Child, budget: Duration, file: Option<File>) -> Finished {
    let output = match file {
        Some(file) => Drain::to_file(child.stdout.take(), file),
        None => Drain::new(child.stdout.take()),
    };
    let errors = Drain::new(child.stderr.take());
    let ended = wait_bounded(child, &output, &errors, budget, TERM_GRACE);
    Finished {
        ended,
        stdout: output.taken(),
        stderr: errors.taken(),
        stdout_error: output.error(),
    }
}

#[cfg(test)]
pub(crate) mod tests;
