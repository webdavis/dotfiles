use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
const FAKE_BOUND: Duration = Duration::from_millis(400);

pub(crate) enum Reply {
    Line(String),
    Chunks(Vec<(Duration, String)>),
    Silent,
    HangUp,
}

pub(crate) struct SocketServer {
    pub(crate) path: PathBuf,
    requests: Receiver<String>,
    stop: Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl SocketServer {
    pub(crate) fn start(replies: Vec<Reply>) -> Self {
        let number = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!("hwj-{}-{number}", std::process::id()));
        std::fs::create_dir(&directory).expect("create owned socket directory");
        let path = directory.join("herdr.sock");
        let listener = UnixListener::bind(&path).expect("bind fake socket");
        listener.set_nonblocking(true).expect("bound fake accept");
        let (request_sender, requests) = mpsc::channel();
        let (stop, stopped) = mpsc::channel();
        let thread = thread::spawn(move || {
            for reply in replies {
                let stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                        Err(_) => return,
                    }
                    if stopped.recv_timeout(Duration::from_millis(1)).is_ok() {
                        return;
                    }
                };
                stream
                    .set_nonblocking(false)
                    .expect("blocking accepted fake stream");
                stream
                    .set_read_timeout(Some(FAKE_BOUND))
                    .expect("bound fake read");
                stream
                    .set_write_timeout(Some(FAKE_BOUND))
                    .expect("bound fake write");
                let mut reader = BufReader::new(stream);
                let mut request = String::new();
                if !reader.read_line(&mut request).is_ok_and(|count| count > 0) {
                    return;
                }
                if request_sender.send(request).is_err() {
                    return;
                }
                match reply {
                    Reply::Line(line) => {
                        let _ = writeln!(reader.get_mut(), "{line}");
                    }
                    Reply::Chunks(chunks) => {
                        for (delay, bytes) in chunks {
                            if stopped.recv_timeout(delay).is_ok() {
                                return;
                            }
                            if reader.get_mut().write_all(bytes.as_bytes()).is_err() {
                                return;
                            }
                        }
                    }
                    Reply::HangUp => {}
                    Reply::Silent => {
                        // The fake's independent bound releases a client whose
                        // own deadline is removed by a mutation.
                        let _ = stopped.recv_timeout(FAKE_BOUND);
                    }
                }
                // herdr 0.8.2 closes each ordinary connection after one reply.
            }
        });
        Self {
            path,
            requests,
            stop,
            thread: Some(thread),
        }
    }

    pub(crate) fn seen(&self) -> serde_json::Value {
        let request = self
            .requests
            .recv_timeout(FAKE_BOUND)
            .expect("fake saw request");
        serde_json::from_str(request.trim()).expect("request is JSON")
    }
}

impl Drop for SocketServer {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(thread) = self.thread.take() {
            thread.join().expect("fake server thread stopped");
        }
        std::fs::remove_dir_all(self.path.parent().expect("owned directory"))
            .expect("remove owned socket fixture");
    }
}
