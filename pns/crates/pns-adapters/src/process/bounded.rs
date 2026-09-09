use super::group::Group;
use super::wait::wait_until;
use pns_application::CommandRunner;
use std::process::Command;
use std::time::Duration;

/// The production runner: spawns the command under a deadline and keeps its
/// stdout.
///
/// EVERY PROBE IS BOUNDED. A wedged herdr, ioreg, pgrep or ps would otherwise
/// hold a notification open indefinitely, and the readings all have a
/// fail-direction already: no answer reads as unknown, which never suppresses.
pub struct SystemCommandRunner;

/// One window for every probe. All of them answer in milliseconds, so this is
/// generous and still far short of a hang.
const PROBE_DEADLINE: Duration = Duration::from_secs(5);

/// One ceiling for every probe's OUTPUT. A registry dump, a process list and a
/// herdr layout are kilobytes, so a mebibyte is generous by three orders of
/// magnitude and still a bound: a probe that answered in gigabytes is a probe
/// that is not answering, and the callers all read no answer as unknown.
pub const PROBE_READ_MAX: u64 = 1024 * 1024;

impl CommandRunner for SystemCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Option<String> {
        let mut command = Command::new(program);
        command.args(args);
        run_bounded(command, None, PROBE_DEADLINE, PROBE_READ_MAX)
    }
}

/// Run a command with a deadline, returning its stdout on success.
///
/// There is no wait-with-timeout in the standard library and macOS ships no
/// `timeout(1)`, so a cleanup child owns the command's process group until
/// completion, deadline, or producer death. Every spawn on a notification path is bounded: the
/// notification is worth less than the turn it reports on.
///
/// BOUNDED IN BYTES AS WELL AS IN TIME, which it was not. The read was a
/// `read_to_end` into a growing `Vec`, so "bounded" meant a child could hand
/// back as much as it managed to write inside the window, and the caller only
/// found out how much AFTER it was all in memory. That was academic while every
/// caller was a probe answering in kilobytes and stopped being academic the
/// moment one of them became an operator-named command running a model for
/// minutes. `max_bytes` is the reader's own ceiling: past it the pipe is closed
/// under the child, which is also what stops it writing.
///
/// AND PAST THE CEILING IS NO ANSWER, which is the DEADLINE'S OWN DIRECTION
/// rather than a second one. A truncated answer is the dangerous shape here:
/// a process list cut at the ceiling has lost its last rows and a JSON listing
/// has stopped mid-object, and both arrive at a caller looking exactly like a
/// complete short answer, so the caller acts on a reading that is missing the
/// part that mattered. Every caller reads no answer as unknown, and unknown
/// never suppresses. The reader is asked for one byte PAST the ceiling, which
/// is what keeps "over the cap" and "exactly at the cap" two different
/// answers, so the bound stays inclusive like every other bound in this crate.
pub fn run_bounded(
    mut command: Command,
    stdin_text: Option<&str>,
    deadline: Duration,
    max_bytes: u64,
) -> Option<String> {
    let expires_at = std::time::Instant::now() + deadline;
    command
        .stdin(if stdin_text.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());

    finish_bounded(&mut command, stdin_text, expires_at, max_bytes)
        .ok()
        .flatten()
}

pub fn finish_bounded(
    command: &mut Command,
    stdin_text: Option<&str>,
    expires_at: std::time::Instant,
    max_bytes: u64,
) -> std::io::Result<Option<String>> {
    let group = Group::start(expires_at)?;
    let child = group.spawn(command)?;
    Ok(collect(child, stdin_text, expires_at, max_bytes))
}

fn collect(
    mut child: std::process::Child,
    stdin_text: Option<&str>,
    expires_at: std::time::Instant,
    max_bytes: u64,
) -> Option<String> {
    // The WRITE is inside the window too: a child that never reads its stdin
    // blocks the writer, and doing it before the clock started meant the
    // deadline never covered the case.
    let stdin_text = stdin_text.map(String::from);
    let mut stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if let (Some(text), Some(mut pipe)) = (stdin_text, stdin.take()) {
            let _ = std::io::Write::write_all(&mut pipe, text.as_bytes());
        }
        // Dropping stdin closes it, which is what tells the child to stop
        // reading; without it a child waiting on EOF never exits.
        drop(stdin);
        // THE READ IS CAPPED: `take` stops at the ceiling instead of growing
        // the buffer to whatever the child felt like writing, and the pipe
        // closing under it is what stops the child writing more. One byte past
        // the ceiling is asked for so the reader downstream can tell a child
        // that went over from one that stopped exactly on it.
        let mut output = Vec::new();
        if let Some(stdout) = stdout {
            let mut capped = std::io::Read::take(stdout, max_bytes.saturating_add(1));
            let _ = std::io::Read::read_to_end(&mut capped, &mut output);
        }
        // The BYTES travel, not a string: the size that matters is the size on
        // the wire, and a lossy conversion grows an invalid byte into three.
        let _ = sender.send(output);
    });

    // Over the ceiling is the same no-answer a blown deadline is; see above.
    let output = receiver
        .recv_timeout(expires_at.saturating_duration_since(std::time::Instant::now()))
        .ok()
        .filter(|bytes: &Vec<u8>| bytes.len() as u64 <= max_bytes)
        .filter(|_| std::time::Instant::now() < expires_at);
    // Closed stdout is not an exited process: a child can close it and sleep,
    // so the wait is polled against the SAME deadline rather than blocking.
    let status = match output.is_some() {
        true => wait_until(&mut child, expires_at, std::thread::sleep),
        false => None,
    };
    let Some(status) = status else {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    };
    // A command that failed has no reading to give: every caller here treats
    // no answer as unknown, which is the honest report.
    //
    // LOSSY, and only now that the size has been judged: every reading here is
    // read line by line downstream, so one invalid byte must cost its own line
    // rather than the whole answer, and `read_to_string` would refuse the lot.
    status
        .success()
        .then_some(output)
        .flatten()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests;
