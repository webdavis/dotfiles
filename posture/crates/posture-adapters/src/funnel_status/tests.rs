use super::*;
use crate::CommandOutput;
use crate::test_sandbox::Sandbox;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

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
    let sandbox = Sandbox::new("funnel-status");
    let root = sandbox.path();
    std::fs::create_dir(root.join("tailscale.d")).unwrap();
    let others_only = root.join("tailscale");
    std::fs::write(&others_only, "#!/bin/sh\n").unwrap();
    // Execute for group and other but never for the identity that would run it,
    // the way a root-owned 0700 binary reads to an unprivileged poller. The mode
    // bits are set, so only an access check refuses it.
    std::fs::set_permissions(&others_only, std::fs::Permissions::from_mode(0o011)).unwrap();
    let mut paths = vec![root.join("tailscale.d"), root.join("absent")];
    // Root is exempt: faccessat grants X_OK on any execute bit at all. The file
    // was just created, so its owner is this process's effective identity.
    if std::fs::metadata(&others_only).unwrap().uid() != 0 {
        paths.push(others_only);
    }
    for path in paths {
        assert_eq!(
            read_funnel(&mut Never, &path),
            Err(FunnelReadFailure::MissingBinary(
                path.to_string_lossy().into_owned()
            ))
        );
    }
}
