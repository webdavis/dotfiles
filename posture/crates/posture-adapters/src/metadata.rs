use std::ffi::CStr;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

pub(super) fn metadata(path: &Path) -> Option<Vec<u8>> {
    // Bash tests existence through a link, then BSD stat describes the link itself.
    if !path.exists() {
        return None;
    }
    let metadata = std::fs::symlink_metadata(path).ok()?;
    let modified = modified_time(metadata.mtime())?;
    let mut fact = b"owner ".to_vec();
    fact.extend_from_slice(&owner_name(metadata.uid()));
    fact.extend_from_slice(b", mode ");
    fact.extend_from_slice(&mode_text(metadata.mode()));
    fact.extend_from_slice(b", modified ");
    fact.extend_from_slice(&modified);
    Some(fact)
}

fn mode_text(mode: u32) -> [u8; 10] {
    let mut text = *b"----------";
    text[0] = match mode & libc::S_IFMT as u32 {
        value if value == libc::S_IFDIR as u32 => b'd',
        value if value == libc::S_IFLNK as u32 => b'l',
        value if value == libc::S_IFCHR as u32 => b'c',
        value if value == libc::S_IFBLK as u32 => b'b',
        value if value == libc::S_IFIFO as u32 => b'p',
        value if value == libc::S_IFSOCK as u32 => b's',
        _ => b'-',
    };
    for (index, bit) in [
        0o400, 0o200, 0o100, 0o040, 0o020, 0o010, 0o004, 0o002, 0o001,
    ]
    .into_iter()
    .enumerate()
    {
        if mode & bit != 0 {
            text[index + 1] = b"rwx"[index % 3];
        }
    }
    for (bit, index, lower, upper) in [
        (0o4000, 3, b's', b'S'),
        (0o2000, 6, b's', b'S'),
        (0o1000, 9, b't', b'T'),
    ] {
        if mode & bit != 0 {
            text[index] = if text[index] == b'x' { lower } else { upper };
        }
    }
    text
}

fn owner_name(uid: u32) -> Vec<u8> {
    let mut bytes = vec![0_u8; 1024];
    loop {
        let mut entry = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut result = std::ptr::null_mut();
        // getpwuid_r initializes entry and a NUL-terminated name within bytes on success.
        let status = unsafe {
            libc::getpwuid_r(
                uid,
                entry.as_mut_ptr(),
                bytes.as_mut_ptr().cast(),
                bytes.len(),
                &mut result,
            )
        };
        if status == libc::ERANGE {
            let Some(size) = bytes.len().checked_mul(2) else {
                break;
            };
            bytes.resize(size, 0);
            continue;
        }
        if status == 0 && !result.is_null() {
            // A successful result points to the initialized entry and its live backing buffer.
            return unsafe { CStr::from_ptr((*result).pw_name) }
                .to_bytes()
                .to_vec();
        }
        break;
    }
    uid.to_string().into_bytes()
}

fn modified_time(seconds: i64) -> Option<Vec<u8>> {
    let mut calendar = std::mem::MaybeUninit::<libc::tm>::uninit();
    // BSD stat uses local time even when its caller's format ends with the literal Z.
    // Both pointers remain valid for the entire libc call.
    if unsafe { libc::localtime_r(&seconds, calendar.as_mut_ptr()) }.is_null() {
        return None;
    }
    // localtime_r initialized every calendar member after the successful return above.
    format_time(&unsafe { calendar.assume_init() })
}

fn format_time(calendar: &libc::tm) -> Option<Vec<u8>> {
    let mut bytes = [0_u8; 64];
    // strftime writes at most the supplied buffer size; the format is NUL-terminated.
    let size = unsafe {
        libc::strftime(
            bytes.as_mut_ptr().cast(),
            bytes.len(),
            c"%Y-%m-%dT%H:%M:%SZ".as_ptr(),
            calendar,
        )
    };
    (size != 0).then(|| bytes[..size].to_vec())
}

#[cfg(test)]
mod tests;
