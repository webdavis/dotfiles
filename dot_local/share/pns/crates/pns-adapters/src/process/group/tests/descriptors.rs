use super::*;
use std::fs::File;
use std::os::fd::{FromRawFd, OwnedFd};

fn isolated(name: &str, action: impl FnOnce()) {
    const FIXTURE: &str = "PNS_DESCRIPTOR_FIXTURE";
    if std::env::var(FIXTURE).as_deref() == Ok(name) {
        let mut limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: this is the private re-executed fixture, never the test host.
        unsafe {
            assert_eq!(libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit), 0);
            limit.rlim_cur = 1_048_576;
            assert_eq!(libc::setrlimit(libc::RLIMIT_NOFILE, &limit), 0);
        }
        action();
        return;
    }
    let test = format!("{}::{name}", module_path!().split_once("::").unwrap().1);
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &test, "--nocapture"])
        .env(FIXTURE, name)
        .stdin(Stdio::null())
        .spawn()
        .unwrap();
    let status = super::super::super::wait::wait_until(
        &mut child,
        Instant::now() + Duration::from_millis(700),
        std::thread::sleep,
    );
    if status.is_none() {
        child.kill().unwrap();
    }
    let _ = child.wait();
    assert!(
        status.is_some_and(|status| status.success()),
        "owned descriptor fixture failed"
    );
}

#[test]
fn cleanup_readiness_does_not_scan_the_descriptor_limit() {
    isolated(
        "cleanup_readiness_does_not_scan_the_descriptor_limit",
        || {
            let group = Group::start(Instant::now() + Duration::from_millis(80)).unwrap();
            let mut command = Command::new("/bin/sh");
            command.args(["-c", "exit 42"]);
            assert_eq!(
                group.spawn(&mut command).unwrap().wait().unwrap().code(),
                Some(42)
            );
        },
    );
}

#[test]
fn cleanup_closes_sparse_high_and_batched_descriptors_before_readiness() {
    isolated(
        "cleanup_closes_sparse_high_and_batched_descriptors_before_readiness",
        || {
            let holes: Vec<_> = (0..4).map(|_| File::open("/dev/null").unwrap()).collect();
            let files: Vec<_> = (0..513).map(|_| File::open("/dev/null").unwrap()).collect();
            let directory = File::open(std::env::temp_dir()).unwrap();
            let (reader, writer) = io::pipe().unwrap();
            // SAFETY: duplicate this fixture's pipe into its own sparse descriptor table.
            let high = unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 8_000) };
            assert!(high >= 8_000);
            let high = unsafe { OwnedFd::from_raw_fd(high) };
            // The four freed low slots become the guardian's two pipes. Both
            // retained ends therefore occur in every full descriptor batch.
            drop(holes);
            let group = Group::start(Instant::now() + Duration::from_millis(300)).unwrap();
            assert!(group.owner.as_ref().unwrap().as_raw_fd() < files[0].as_raw_fd());
            let mut descriptors = [libc::proc_fdinfo {
                proc_fd: 0,
                proc_fdtype: 0,
            }; 1024];
            // SAFETY: observe only our unreaped cleanup child's descriptor table.
            let bytes = unsafe {
                libc::proc_pidinfo(
                    group.pid,
                    libc::PROC_PIDLISTFDS,
                    0,
                    descriptors.as_mut_ptr().cast(),
                    std::mem::size_of_val(&descriptors) as i32,
                )
            };
            assert!(bytes > 0);
            let count = bytes as usize / std::mem::size_of::<libc::proc_fdinfo>();
            let forbidden = [
                directory.as_raw_fd(),
                reader.as_raw_fd(),
                writer.as_raw_fd(),
                high.as_raw_fd(),
            ];
            for record in &descriptors[..count] {
                assert!(
                    !forbidden.contains(&record.proc_fd),
                    "guardian retained {:?}",
                    record.proc_fd
                );
                assert!(!files.iter().any(|file| file.as_raw_fd() == record.proc_fd));
            }
            drop(group);
            drop((files, directory, reader, writer, high));
        },
    );
}
