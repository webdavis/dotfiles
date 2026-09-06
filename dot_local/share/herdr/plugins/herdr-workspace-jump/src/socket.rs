//! The fast path: one Unix socket connection, spoken to directly.
//!
//! herdr injects `HERDR_SOCKET_PATH` into every plugin command, and the socket
//! is the same control surface the CLI wraps. Talking to it directly costs one
//! connection for the whole jump instead of two `herdr` process spawns plus a
//! `jq` spawn, and the response is parsed in process.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use crate::jump::WorkspaceDirectory;
use crate::protocol::{self, JumpError, Workspace};

/// How long any single read or write may block.
///
/// A wedged herdr server accepts the connection and then never answers, which
/// would otherwise hang the keychord forever. The deadline turns that into a
/// `Transport` failure, which is what sends the composition root to the CLI.
pub const DEADLINE: Duration = Duration::from_secs(5);

pub struct SocketWorkspaceDirectory {
    connection: BufReader<UnixStream>,
    next_request_id: u32,
}

impl SocketWorkspaceDirectory {
    pub fn connect(path: &Path, deadline: Duration) -> Result<Self, JumpError> {
        let stream = UnixStream::connect(path)
            .map_err(|error| JumpError::Unreachable(format!("{}: {error}", path.display())))?;
        stream
            .set_read_timeout(Some(deadline))
            .and_then(|()| stream.set_write_timeout(Some(deadline)))
            .map_err(|error| JumpError::Transport(format!("could not set a deadline: {error}")))?;
        Ok(Self {
            connection: BufReader::new(stream),
            next_request_id: 1,
        })
    }

    fn take_request_id(&mut self) -> String {
        let request_id = self.next_request_id;
        self.next_request_id += 1;
        format!("req_{request_id}")
    }

    /// Write one request line and read the one response line that answers it.
    fn round_trip(&mut self, request: &str) -> Result<Value, JumpError> {
        self.connection
            .get_mut()
            .write_all(request.as_bytes())
            .map_err(|error| JumpError::Transport(format!("write failed: {error}")))?;
        let mut response = String::new();
        let read = self
            .connection
            .read_line(&mut response)
            .map_err(|error| JumpError::Transport(format!("read failed: {error}")))?;
        if read == 0 {
            return Err(JumpError::Transport(
                "herdr closed the connection without answering".to_string(),
            ));
        }
        protocol::result_of(&response)
    }
}

impl WorkspaceDirectory for SocketWorkspaceDirectory {
    fn list(&mut self) -> Result<Vec<Workspace>, JumpError> {
        let request_id = self.take_request_id();
        let result = self.round_trip(&protocol::list_request(&request_id))?;
        protocol::workspaces_of(&result)
    }

    fn focus(&mut self, workspace_id: &str) -> Result<(), JumpError> {
        let request_id = self.take_request_id();
        self.round_trip(&protocol::focus_request(&request_id, workspace_id))?;
        Ok(())
    }

    fn create(&mut self, label: &str, cwd: &str) -> Result<(), JumpError> {
        let request_id = self.take_request_id();
        self.round_trip(&protocol::create_request(&request_id, label, cwd))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jump::{Jump, jump};
    use std::os::unix::net::UnixListener;
    use std::sync::mpsc;
    use std::thread;

    /// A herdr stand-in: reads request lines, answers each with the next canned
    /// reply, and hands the requests it saw back over a channel.
    struct FakeHerdrServer {
        path: std::path::PathBuf,
        requests: mpsc::Receiver<String>,
        _directory: TempDirectory,
    }

    /// A scratch directory removed when the test ends. The socket has to live on
    /// a real path, and the tests must not leave one behind.
    struct TempDirectory(std::path::PathBuf);

    impl Drop for TempDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// What the fake does once its canned replies run out.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum AfterReplies {
        /// Close the connection, the shape of a herdr that died.
        HangUp,
        /// Hold it open and answer nothing, the shape of a wedged herdr.
        StaySilent,
    }

    fn start_fake_herdr(name: &str, replies: Vec<String>, after: AfterReplies) -> FakeHerdrServer {
        let directory = std::env::temp_dir().join(format!("hwj-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        if let Err(error) = std::fs::create_dir_all(&directory) {
            panic!("could not make the socket directory: {error}");
        }
        let path = directory.join("herdr.sock");
        let listener = match UnixListener::bind(&path) {
            Ok(listener) => listener,
            Err(error) => panic!("could not bind the fake socket: {error}"),
        };
        let (sender, requests) = mpsc::channel();
        thread::spawn(move || {
            let Ok((stream, _)) = listener.accept() else {
                return;
            };
            let mut reader = BufReader::new(stream);
            for reply in replies {
                let mut request = String::new();
                match reader.read_line(&mut request) {
                    Ok(0) | Err(_) => return,
                    Ok(_) => {}
                }
                let _ = sender.send(request);
                if reader
                    .get_mut()
                    .write_all(format!("{reply}\n").as_bytes())
                    .is_err()
                {
                    return;
                }
            }
            if after == AfterReplies::StaySilent {
                // Hold the connection OPEN and silent. Closing here would let a
                // client with no read deadline escape on the hangup instead of
                // on its timeout, which is the difference between a test that
                // pins the deadline and one that only looks like it does. This
                // read returns when the test drops its client, so the thread
                // does not outlive the test.
                // The loop matters: a single read would consume the next
                // request and END this thread, closing the socket, and the
                // client would again escape on the hangup. Reading until the
                // client goes away is what keeps it genuinely wedged.
                let mut ignored = String::new();
                while reader.read_line(&mut ignored).is_ok_and(|read| read > 0) {
                    ignored.clear();
                }
            }
        });
        FakeHerdrServer {
            path,
            requests,
            _directory: TempDirectory(directory),
        }
    }

    fn connected(server: &FakeHerdrServer) -> SocketWorkspaceDirectory {
        match SocketWorkspaceDirectory::connect(&server.path, DEADLINE) {
            Ok(directory) => directory,
            Err(error) => panic!("could not connect to the fake socket: {error}"),
        }
    }

    fn seen(server: &FakeHerdrServer) -> Value {
        let request = match server.requests.recv_timeout(Duration::from_secs(5)) {
            Ok(request) => request,
            Err(error) => panic!("the fake server saw no request: {error}"),
        };
        match serde_json::from_str(request.trim()) {
            Ok(value) => value,
            Err(error) => panic!("the request was not JSON: {error}"),
        }
    }

    const TWO_WORKSPACES: &str = r#"{"id":"req_1","result":{"workspaces":[{"workspace_id":"wA","label":"netpulse"},{"workspace_id":"w11","label":"dotfiles"}]}}"#;
    const OK: &str = r#"{"id":"req_2","result":{"type":"ok"}}"#;

    #[test]
    fn a_jump_that_focuses_sends_list_then_focus_over_one_connection() {
        let server = start_fake_herdr(
            "focus",
            vec![TWO_WORKSPACES.into(), OK.into()],
            AfterReplies::HangUp,
        );
        let mut directory = connected(&server);
        let outcome = jump(&mut directory, "dotfiles", "/home/me/dotfiles");

        assert_eq!(outcome, Ok(Jump::Focused("w11".into())));
        assert_eq!(seen(&server)["method"], "workspace.list");
        let focus = seen(&server);
        assert_eq!(focus["method"], "workspace.focus");
        assert_eq!(focus["params"]["workspace_id"], "w11");
    }

    #[test]
    fn a_jump_that_creates_sends_list_then_create_with_cwd_label_and_focus() {
        let server = start_fake_herdr(
            "create",
            vec![TWO_WORKSPACES.into(), OK.into()],
            AfterReplies::HangUp,
        );
        let mut directory = connected(&server);
        let outcome = jump(&mut directory, "homelab", "/home/me/homelab");

        assert_eq!(outcome, Ok(Jump::Created));
        assert_eq!(seen(&server)["method"], "workspace.list");
        let create = seen(&server);
        assert_eq!(create["method"], "workspace.create");
        assert_eq!(create["params"]["label"], "homelab");
        assert_eq!(create["params"]["cwd"], "/home/me/homelab");
        assert_eq!(create["params"]["focus"], true);
    }

    #[test]
    fn each_request_on_one_connection_carries_a_fresh_id() {
        let server = start_fake_herdr(
            "ids",
            vec![TWO_WORKSPACES.into(), OK.into()],
            AfterReplies::HangUp,
        );
        let mut directory = connected(&server);
        let _ = jump(&mut directory, "dotfiles", "/tmp");
        assert_eq!(seen(&server)["id"], "req_1");
        assert_eq!(seen(&server)["id"], "req_2");
    }

    #[test]
    fn an_error_envelope_from_the_server_fails_the_jump() {
        let refusal = r#"{"id":"req_1","error":{"code":"internal","message":"broken"}}"#;
        let server = start_fake_herdr("refuse", vec![refusal.into()], AfterReplies::HangUp);
        let mut directory = connected(&server);
        assert_eq!(
            jump(&mut directory, "dotfiles", "/tmp"),
            Err(JumpError::Server {
                code: "internal".to_string(),
                message: "broken".to_string(),
            })
        );
    }

    #[test]
    fn a_server_that_hangs_up_without_answering_is_a_transport_failure() {
        let server = start_fake_herdr("silent", vec![], AfterReplies::HangUp);
        let mut directory = connected(&server);
        assert!(matches!(
            jump(&mut directory, "dotfiles", "/tmp"),
            Err(JumpError::Transport(_))
        ));
    }

    #[test]
    fn a_missing_socket_is_unreachable_rather_than_a_transport_failure() {
        let absent = std::env::temp_dir().join("hwj-no-such-socket-ever");
        let _ = std::fs::remove_file(&absent);
        assert!(matches!(
            SocketWorkspaceDirectory::connect(&absent, DEADLINE),
            Err(JumpError::Unreachable(_))
        ));
    }

    #[test]
    fn a_server_that_never_answers_fails_on_the_deadline_rather_than_hanging() {
        // The reply list is non-empty, so the fake reads the request and then
        // blocks on the next one instead of closing: the client only escapes on
        // its read deadline. Kept short so the suite stays inside its budget.
        let server = start_fake_herdr(
            "wedged",
            vec![TWO_WORKSPACES.into(), OK.into()],
            AfterReplies::StaySilent,
        );
        let mut directory =
            match SocketWorkspaceDirectory::connect(&server.path, Duration::from_millis(150)) {
                Ok(directory) => directory,
                Err(error) => panic!("could not connect: {error}"),
            };
        // Drain the two canned replies so the third request is met with silence.
        let _ = directory.list();
        let _ = directory.focus("wA");
        assert!(matches!(
            directory.focus("wA"),
            Err(JumpError::Transport(_))
        ));
    }
}
