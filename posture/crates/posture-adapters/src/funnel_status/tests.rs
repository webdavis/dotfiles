use super::*;
use crate::CommandOutput;
use std::os::unix::fs::PermissionsExt;

struct Never;
impl CommandRunner for Never {
    fn run_completed(
        &mut self,
        _program: &Path,
        _args: &[&OsStr],
        _io: CommandIo<'_>,
    ) -> Result<CommandOutput, InspectionFailure> {
        panic!("an unexecutable path is refused before anything is spawned")
    }
}

#[test]
fn a_directory_or_a_path_this_identity_cannot_execute_reads_as_a_missing_binary() {
    let root = std::env::temp_dir().join(format!("posture-funnel-status-{}", std::process::id()));
    std::fs::create_dir_all(root.join("tailscale.d")).unwrap();
    let others_only = root.join("tailscale");
    std::fs::write(&others_only, "#!/bin/sh\n").unwrap();
    // Execute for group and other but never for the identity that would run it,
    // the way a root-owned 0700 binary reads to an unprivileged poller. The mode
    // bits are set, so only an access check refuses it.
    std::fs::set_permissions(&others_only, std::fs::Permissions::from_mode(0o011)).unwrap();
    for path in [root.join("tailscale.d"), others_only, root.join("absent")] {
        assert_eq!(
            read_funnel(&mut Never, &path),
            Err(FunnelReadFailure::MissingBinary(
                path.to_string_lossy().into_owned()
            ))
        );
    }
}
