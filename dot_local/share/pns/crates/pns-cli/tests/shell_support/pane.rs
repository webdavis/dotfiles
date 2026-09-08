use super::Fixture;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub struct Pane {
    child: Child,
    pub pid: u32,
}
impl Pane {
    pub fn begin(fixture: &Fixture) -> Self {
        let inherited = fixture.command();
        let mut shell = Command::new("/bin/sh");
        shell.env_clear();
        for (key, value) in inherited.get_envs() {
            if let Some(value) = value {
                shell.env(key, value);
            }
        }
        let script = r#""$1" shell begin --pid "$$" --command build || exit 3
printf '%s\n' "$$"
read ignored
"#;
        let child = shell
            .args(["-c", script, "fixture"])
            .arg(inherited.get_program())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut pane = Self {
            pid: child.id(),
            child,
        };
        let output = pane.child.stdout.take().unwrap();
        let (send, recv) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let _ = BufReader::new(output).read_line(&mut line);
            let _ = send.send(line);
        });
        let pid = recv.recv_timeout(Duration::from_millis(400)).unwrap();
        assert_eq!(pid.trim().parse::<u32>().unwrap(), pane.pid);
        pane
    }
}
impl Drop for Pane {
    fn drop(&mut self) {
        drop(self.child.stdin.take());
        let deadline = Instant::now() + Duration::from_millis(100);
        while self.child.try_wait().unwrap().is_none() {
            if Instant::now() >= deadline {
                let _ = self.child.kill();
                let _ = self.child.wait();
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
