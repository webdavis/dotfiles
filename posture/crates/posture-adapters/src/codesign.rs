use crate::property_list::PropertyList;
use crate::{CommandIo, CommandRunner};
use posture_application::{EnrichmentInspection, InspectionFailure};
use std::ffi::{CString, OsStr};
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// The three Mach-O magics and their byte-swapped twins, read big-endian from
/// the first four bytes of a file.
const MACH_O_MAGICS: [u32; 6] = [
    0xfeed_face,
    0xfeed_facf,
    0xcafe_babe,
    0xcefa_edfe,
    0xcffa_edfe,
    0xbeba_feca,
];
const QUARANTINE: &[u8] = b"com.apple.quarantine\0";

pub struct SystemInspection<R> {
    runner: R,
    diagnostics: Vec<u8>,
}
impl<R: CommandRunner> SystemInspection<R> {
    pub fn new(runner: R) -> Self {
        Self {
            runner,
            diagnostics: Vec::new(),
        }
    }
    pub fn diagnostics(&self) -> &[u8] {
        &self.diagnostics
    }

    fn read(
        &mut self,
        program: &str,
        args: &[&OsStr],
        merged: bool,
    ) -> Result<Vec<u8>, InspectionFailure> {
        self.runner
            .run(
                Path::new(program),
                args,
                CommandIo::Inspection {
                    merge_stderr: merged,
                },
            )
            .map(|bytes| self.sanitize(bytes))
    }

    fn sanitize(&mut self, mut bytes: Vec<u8>) -> Vec<u8> {
        if bytes.contains(&0) {
            self.diagnostics
                .extend_from_slice(b"posture: warning: ignored null byte in inspection output\n");
            bytes.retain(|byte| *byte != 0);
        }
        // Trailing newlines are never part of an inspection value.
        while bytes.last() == Some(&b'\n') {
            bytes.pop();
        }
        bytes
    }
}
impl<R: CommandRunner> EnrichmentInspection for SystemInspection<R> {
    fn plist_value(&mut self, path: &Path, key: &str) -> Result<Vec<u8>, InspectionFailure> {
        let value = PropertyList::read(path)
            .and_then(|list| list.raw(key))
            .ok_or(InspectionFailure::Failed)?;
        Ok(self.sanitize(value))
    }
    fn signing(&mut self, path: &Path) -> Result<Vec<u8>, InspectionFailure> {
        self.read(
            "/usr/bin/codesign",
            &[
                OsStr::new("-dv"),
                OsStr::new("--verbose=2"),
                path.as_os_str(),
            ],
            true,
        )
    }
    fn quarantined(&mut self, path: &Path) -> bool {
        quarantine_attribute(path).is_some_and(|value| !self.sanitize(value).is_empty())
    }
    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }
    fn is_mach_o(&mut self, path: &Path) -> bool {
        self.is_file(path) && magic(path).is_some_and(|magic| MACH_O_MAGICS.contains(&magic))
    }
    fn metadata(&mut self, path: &Path) -> Option<Vec<u8>> {
        crate::metadata::metadata(path)
    }
}

/// The first four bytes of a file, big-endian. A file too short to hold them,
/// or one that cannot be opened, has no magic.
fn magic(path: &Path) -> Option<u32> {
    let mut bytes = [0_u8; 4];
    std::fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut bytes))
        .ok()?;
    Some(u32::from_be_bytes(bytes))
}

/// The quarantine attribute's bytes, or None when the path carries no such
/// attribute and when the read fails for any other reason.
fn quarantine_attribute(path: &Path) -> Option<Vec<u8>> {
    let path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let name = QUARANTINE.as_ptr().cast::<libc::c_char>();
    // getxattr with a null buffer reports the attribute's size, leaving the
    // path and name pointers untouched; -1 means no such attribute.
    let size = unsafe { libc::getxattr(path.as_ptr(), name, std::ptr::null_mut(), 0, 0, 0) };
    let size = usize::try_from(size).ok()?;
    let mut value = vec![0_u8; size];
    // The second call fills exactly the buffer measured above.
    let read = unsafe {
        libc::getxattr(
            path.as_ptr(),
            name,
            value.as_mut_ptr().cast::<libc::c_void>(),
            size,
            0,
            0,
        )
    };
    value.truncate(usize::try_from(read).ok()?);
    Some(value)
}

#[cfg(test)]
mod tests;
