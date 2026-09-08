use posture_domain::ConvergeFile;
use std::ffi::{CString, OsString};
use std::fs;
use std::io;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct StagedTree {
    root: PathBuf,
}

impl StagedTree {
    pub(super) fn create(parent: &Path) -> io::Result<Self> {
        let path = parent.join("posture-converge.XXXXXX");
        let mut template = CString::new(path.as_os_str().as_bytes())?.into_bytes_with_nul();
        // mkdtemp edits this live NUL-terminated buffer and atomically creates a private directory.
        if unsafe { libc::mkdtemp(template.as_mut_ptr().cast()) }.is_null() {
            return Err(io::Error::last_os_error());
        }
        template.pop();
        let staged = Self {
            root: PathBuf::from(OsString::from_vec(template)),
        };
        fs::set_permissions(&staged.root, fs::Permissions::from_mode(0o700))?;
        fs::create_dir(staged.root.join("packs"))?;
        Ok(staged)
    }

    pub fn source(&self, file: ConvergeFile) -> PathBuf {
        self.root.join(file.relative_path())
    }
}

impl Drop for StagedTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
