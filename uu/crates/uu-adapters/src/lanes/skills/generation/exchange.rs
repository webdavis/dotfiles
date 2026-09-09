use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

pub fn exchange_skills_directories(a: &Path, b: &Path) -> Result<(), String> {
    let a = CString::new(a.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
    let b = CString::new(b.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
    // SAFETY: both C strings are owned, NUL-terminated and alive for this call.
    // RENAME_SWAP exchanges existing paths in one filesystem operation.
    let result = unsafe {
        libc::renameatx_np(
            libc::AT_FDCWD,
            a.as_ptr(),
            libc::AT_FDCWD,
            b.as_ptr(),
            libc::RENAME_SWAP,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(())
}
