use crate::{CommandIo, CommandRunner};
use posture_application::{EnrichmentInspection, InspectionFailure};
use std::ffi::OsStr;
use std::path::Path;

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
            .map(|mut bytes| {
                if bytes.contains(&0) {
                    self.diagnostics.extend_from_slice(
                        b"posture: warning: ignored null byte in inspection output\n",
                    );
                    bytes.retain(|byte| *byte != 0);
                }
                // Command substitution strips all trailing newlines, including an empty raw plist value.
                while bytes.last() == Some(&b'\n') {
                    bytes.pop();
                }
                bytes
            })
    }
}
impl<R: CommandRunner> EnrichmentInspection for SystemInspection<R> {
    fn plist_value(&mut self, path: &Path, key: &str) -> Result<Vec<u8>, InspectionFailure> {
        self.read(
            "/usr/bin/plutil",
            &[
                OsStr::new("-extract"),
                OsStr::new(key),
                OsStr::new("raw"),
                OsStr::new("-o"),
                OsStr::new("-"),
                path.as_os_str(),
            ],
            false,
        )
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
        self.read(
            "/usr/bin/xattr",
            &[
                OsStr::new("-p"),
                OsStr::new("com.apple.quarantine"),
                path.as_os_str(),
            ],
            false,
        )
        .is_ok_and(|value| !value.is_empty())
    }
    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }
    fn is_mach_o(&mut self, path: &Path) -> bool {
        self.is_file(path)
            && self
                .read("/usr/bin/file", &[path.as_os_str()], false)
                .is_ok_and(|bytes| {
                    bytes
                        .windows(6)
                        .any(|value| value.eq_ignore_ascii_case(b"mach-o"))
                })
    }
    fn metadata(&mut self, path: &Path) -> Option<Vec<u8>> {
        crate::metadata::metadata(path)
    }
}

#[cfg(test)]
mod tests;
