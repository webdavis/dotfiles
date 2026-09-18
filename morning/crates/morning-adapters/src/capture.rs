//! Running a command the config named, under a deadline.
//!
//! `gh` and `td` are spawned rather than linked, so morning never carries their
//! credentials, their schema or their release cadence. Both can also hang, on a
//! network call or on a prompt, which is why nothing here waits forever.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Why a command produced no output.
#[derive(Debug)]
pub enum CaptureError {
    /// The command could not be started, typically because it is not installed.
    NotStarted(std::io::Error),
    /// The command ran past its deadline and was killed.
    TimedOut(Duration),
    /// The command failed, with whatever it said about it.
    Failed { status: String, message: String },
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotStarted(error) => write!(formatter, "could not run it: {error}"),
            Self::TimedOut(limit) => write!(
                formatter,
                "no answer within {} seconds, so it was stopped",
                limit.as_secs_f32()
            ),
            Self::Failed { status, message } => match message.is_empty() {
                true => write!(formatter, "it {status}"),
                false => write!(formatter, "it {status}: {message}"),
            },
        }
    }
}

/// How often a running child is checked against its deadline.
const POLL: Duration = Duration::from_millis(10);

/// Runs `argv` and returns its standard output, killing it at `limit`.
///
/// Both pipes are drained by their own threads while the deadline is polled,
/// because a child that fills a pipe nobody is reading blocks forever and would
/// outlast the deadline it was given.
pub fn capture(argv: &[String], limit: Duration) -> Result<String, CaptureError> {
    let (program, arguments) = argv.split_first().ok_or_else(|| CaptureError::Failed {
        status: "was configured with no command to run".to_string(),
        message: String::new(),
    })?;
    let mut child = Command::new(program)
        .args(arguments)
        // The page is plain text wherever it is read, and a child that decides
        // on color by itself has no terminal to ask about here.
        .env("NO_COLOR", "1")
        .env("CLICOLOR", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(CaptureError::NotStarted)?;
    let out = drain(child.stdout.take());
    let err = drain(child.stderr.take());
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Err(error) => return Err(CaptureError::NotStarted(error)),
            Ok(Some(status)) => {
                let stdout = out.join().unwrap_or_default();
                return match status.success() {
                    true => Ok(stdout),
                    false => Err(CaptureError::Failed {
                        status: describe(status),
                        message: first_line(&err.join().unwrap_or_default()),
                    }),
                };
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(CaptureError::TimedOut(limit));
            }
            Ok(None) => std::thread::sleep(POLL),
        }
    }
}

fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_string(&mut text);
        }
        text
    })
}

fn describe(status: std::process::ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("exited {code}"),
        None => "was killed by a signal".to_string(),
    }
}

fn first_line(message: &str) -> String {
    message
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests;
