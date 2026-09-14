use posture_domain::{IncludePattern, SshTreeRefusal};
use std::{
    ffi::{CStr, CString, OsStr},
    fs, io,
    os::unix::ffi::OsStrExt,
    path::Path,
};

type Roots = (Option<Vec<u8>>, Vec<Vec<u8>>);
pub(super) fn roots(main: &Path, directory: &Path) -> Result<Roots, SshTreeRefusal> {
    let main = regular(main.as_os_str().as_bytes())?.then(|| main.as_os_str().as_bytes().to_vec());
    let mut paths = Vec::new();
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok((main, paths)),
        Err(_) => {
            return Err(SshTreeRefusal::Unreadable(
                directory.as_os_str().as_bytes().to_vec(),
            ));
        }
    };
    for entry in entries {
        let entry = entry
            .map_err(|_| SshTreeRefusal::Unreadable(directory.as_os_str().as_bytes().to_vec()))?;
        if entry.file_name().as_bytes().starts_with(b".") {
            continue;
        }
        let path = entry.path().as_os_str().as_bytes().to_vec();
        if regular(&path)? {
            paths.push(path);
            if paths.len() > 512 {
                return Err(SshTreeRefusal::Visits);
            }
        }
    }
    Ok((main, paths))
}

pub(super) fn resolve(pattern: &IncludePattern) -> Result<Vec<Vec<u8>>, SshTreeRefusal> {
    if !pattern.has_glob {
        return Ok(if regular(&pattern.literal)? {
            vec![pattern.literal.clone()]
        } else {
            Vec::new()
        });
    }
    let text = CString::new(pattern.pattern.clone())
        .map_err(|_| SshTreeRefusal::Path(pattern.pattern.clone()))?;
    // glob initializes the zeroed result. Its allocated paths remain owned until globfree in Drop.
    let mut result = Glob(unsafe { std::mem::zeroed() });
    let status = unsafe {
        libc::glob(
            text.as_ptr(),
            libc::GLOB_NOSORT | libc::GLOB_NOCHECK,
            None,
            &mut result.0,
        )
    };
    if status != 0 {
        return Err(SshTreeRefusal::Unreadable(pattern.pattern.clone()));
    }
    let mut paths = Vec::new();
    for index in 0..result.0.gl_pathc {
        // Successful glob returned gl_pathc live C strings; none is retained past globfree.
        let path = unsafe { CStr::from_ptr(*result.0.gl_pathv.add(index)) }.to_bytes();
        if regular(path)? {
            paths.push(path.to_vec());
            if paths.len() > 512 {
                return Err(SshTreeRefusal::Visits);
            }
        }
    }
    paths.sort();
    Ok(paths)
}

fn regular(path: &[u8]) -> Result<bool, SshTreeRefusal> {
    match fs::metadata(OsStr::from_bytes(path)) {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(SshTreeRefusal::Unreadable(path.to_vec())),
    }
}

struct Glob(libc::glob_t);
impl Drop for Glob {
    fn drop(&mut self) {
        // This object owns the result initialized by the one glob call above.
        unsafe {
            libc::globfree(&mut self.0);
        }
    }
}
