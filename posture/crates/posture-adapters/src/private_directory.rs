use std::{
    ffi::{CString, OsString},
    fs, io,
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::PermissionsExt,
    },
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub(crate) struct PrivateDirectory(PathBuf);

impl PrivateDirectory {
    pub(crate) fn create(parent: &Path) -> io::Result<Self> {
        let path = parent.join("posture-converge.XXXXXX");
        let mut template = CString::new(path.as_os_str().as_bytes())?.into_bytes_with_nul();
        // mkdtemp edits this live NUL-terminated buffer and atomically creates a private directory.
        if unsafe { libc::mkdtemp(template.as_mut_ptr().cast()) }.is_null() {
            return Err(io::Error::last_os_error());
        }
        template.pop();
        let directory = Self(PathBuf::from(OsString::from_vec(template)));
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))?;
        Ok(directory)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for PrivateDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.path());
    }
}
