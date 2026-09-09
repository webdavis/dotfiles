use super::{AuditFile, AuditKind};
use crate::{KnownGoodTuple, ManifestDigest};

pub(super) fn kinds(
    want: KnownGoodTuple<'_>,
    file: AuditFile<'_>,
    max_bytes: u64,
) -> Vec<AuditKind> {
    let (size, mode, uid, digest) = match file {
        AuditFile::Missing => return vec![AuditKind::Missing],
        AuditFile::Symlink | AuditFile::Irregular => return vec![AuditKind::Irregular],
        AuditFile::Regular {
            size,
            mode,
            uid,
            digest,
        } => (size, mode, uid, digest),
    };
    let ManifestDigest::Built(expected) = want.digest else {
        return vec![AuditKind::Content];
    };
    let (Some(size), Some(mode), Some(uid)) = (size, mode, uid) else {
        return vec![AuditKind::Unreadable];
    };
    if mode.len() != 4
        || !mode.bytes().all(|byte| (b'0'..=b'7').contains(&byte))
        || !(1..=10).contains(&uid.len())
        || !uid.bytes().all(|byte| byte.is_ascii_digit())
    {
        return vec![AuditKind::Unreadable];
    }
    let mut kinds = Vec::new();
    if size > max_bytes {
        kinds.push(AuditKind::Oversize);
    } else if let Some(actual) = digest
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        if !expected.eq_ignore_ascii_case(actual) {
            kinds.push(AuditKind::Content);
        }
    } else {
        kinds.push(AuditKind::Unreadable);
    }
    // Oversize or an unreadable hash does not excuse attribute drift. A bad
    // attribute read above instead produces one unreadable finding for the path.
    if mode != want.mode {
        kinds.push(AuditKind::Mode);
    }
    if uid != want.uid {
        kinds.push(AuditKind::Owner);
    }
    kinds
}
