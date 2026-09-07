use std::io::{self, Read};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub(super) struct CapturedChild {
    pub(super) child: Child,
    stdout: Receiver<io::Result<Vec<u8>>>,
    stderr: Receiver<io::Result<Vec<u8>>>,
    readers: Vec<JoinHandle<()>>,
    reaped: bool,
}

impl CapturedChild {
    pub(super) fn spawn(command: &mut Command) -> io::Result<Self> {
        let mut child = command
            .process_group(0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let (stdout, out_reader) = drain(child.stdout.take().expect("piped stdout"));
        let (stderr, err_reader) = drain(child.stderr.take().expect("piped stderr"));
        Ok(Self {
            child,
            stdout,
            stderr,
            readers: vec![out_reader, err_reader],
            reaped: false,
        })
    }

    pub(super) fn output_within(mut self, limit: Duration) -> io::Result<Output> {
        let deadline = Instant::now() + limit;
        // Both streams must close before reaping: an inherited pipe may still
        // belong to a submission descendant after the hook has exited.
        let stdout = receive(&self.stdout, deadline)?;
        let stderr = receive(&self.stderr, deadline)?;
        loop {
            if let Some(status) = self.child.try_wait()? {
                self.reaped = true;
                return Ok(Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "child did not exit",
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for CapturedChild {
    fn drop(&mut self) {
        if !self.reaped {
            // The unreaped child's ID still owns this private group. Kill the
            // inherited pipe holders before joining readers blocked on EOF.
            if let Ok(pid) = libc::pid_t::try_from(self.child.id())
                && pid > 1
            {
                // SAFETY: spawn created this group; the child is unreaped.
                unsafe { libc::kill(-pid, libc::SIGKILL) };
            }
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        for reader in self.readers.drain(..) {
            let _ = reader.join();
        }
    }
}

fn drain(reader: impl Read + Send + 'static) -> (Receiver<io::Result<Vec<u8>>>, JoinHandle<()>) {
    let (sender, receiver) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        let mut reader = reader;
        let mut bytes = Vec::new();
        let result = reader.read_to_end(&mut bytes).map(|_| bytes);
        let _ = sender.send(result);
    });
    (receiver, handle)
}

fn receive(receiver: &Receiver<io::Result<Vec<u8>>>, deadline: Instant) -> io::Result<Vec<u8>> {
    receiver
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|error| match error {
            mpsc::RecvTimeoutError::Timeout => {
                io::Error::new(io::ErrorKind::TimedOut, "pipe stayed open")
            }
            mpsc::RecvTimeoutError::Disconnected => io::Error::other("pipe reader stopped"),
        })?
}

#[cfg(test)]
#[path = "captured_child/tests.rs"]
mod tests;
