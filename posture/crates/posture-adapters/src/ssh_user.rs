use std::ffi::CStr;

pub fn current_user_name() -> Option<String> {
    let mut bytes = vec![0_u8; 1024];
    loop {
        let mut entry = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut result = std::ptr::null_mut();
        // The real uid matches id -un. A successful lookup initializes entry and its live buffer.
        let status = unsafe {
            libc::getpwuid_r(
                libc::getuid(),
                entry.as_mut_ptr(),
                bytes.as_mut_ptr().cast(),
                bytes.len(),
                &mut result,
            )
        };
        if status == libc::ERANGE && bytes.len() < 1_048_576 {
            bytes.resize(bytes.len() * 2, 0);
            continue;
        }
        if status != 0 || result.is_null() {
            return None;
        }
        let name = unsafe { (*result).pw_name };
        if name.is_null() {
            return None;
        }
        let name = unsafe { CStr::from_ptr(name) }.to_str().ok()?;
        return (!name.is_empty()).then(|| name.to_owned());
    }
}

/// The calling process's own user identity, which is the launchd domain its
/// per-user jobs live in (`gui/<uid>`).
pub fn current_uid() -> u32 {
    // getuid takes no pointers and always returns the calling process identity.
    unsafe { libc::getuid() }
}
