use posture_domain::{SshAttributes, SshRecord, SshTreeRefusal, SshWalkBudget};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsStr,
    fs::OpenOptions,
    io::Read,
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt},
    },
    path::Path,
};

pub(super) fn read(
    path: &[u8],
    budget: &mut SshWalkBudget,
) -> Result<(SshRecord, Vec<u8>), SshTreeRefusal> {
    let unreadable = || SshTreeRefusal::Unreadable(path.to_vec());
    // Follow Include symlinks, but open nonblocking and inspect the opened descriptor before reading.
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(Path::new(OsStr::from_bytes(path)))
        .map_err(|_| unreadable())?;
    let metadata = file.metadata().map_err(|_| unreadable())?;
    if !metadata.is_file() {
        return Err(SshTreeRefusal::NonRegular(path.to_vec()));
    }
    let mut bytes = Vec::new();
    file.take(262145)
        .read_to_end(&mut bytes)
        .map_err(|_| unreadable())?;
    let charged = bytes.len() + usize::from(!bytes.is_empty() && !bytes.ends_with(b"\n"));
    budget.read(charged)?;
    let record = SshRecord {
        path: path.to_vec(),
        attributes: SshAttributes {
            mode: metadata.mode() & 0o7777,
            uid: metadata.uid(),
            gid: metadata.gid(),
        },
        checksum: Sha256::digest(&bytes).into(),
    };
    Ok((record, bytes))
}
