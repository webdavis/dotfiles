//! The real hosts file: reading it, and replacing it atomically as root.

use std::fs;
use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use tailnet_pin_application::HostsFile;

/// How far a symlinked hosts path is followed before a loop is assumed.
const MAXIMUM_SYMLINK_HOPS: usize = 8;

/// A hosts file on disk, addressed by the path a caller configured and the path
/// that configuration finally names.
#[derive(Debug, PartialEq, Eq)]
pub struct RealHostsFile {
    configured: PathBuf,
    resolved: PathBuf,
}

/// Why a path could not be opened for reconciling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathFault {
    /// The symlink chain loops, or a link could not be read.
    UnresolvedSymlink,
    /// The path resolves to something that is not a regular file, or to
    /// nothing.
    NotAFile,
}

impl RealHostsFile {
    /// Resolve `configured` through its symlink chain and check that it names a
    /// regular file.
    pub fn open(configured: &Path) -> Result<Self, PathFault> {
        let resolved = resolve_symlink_chain(configured).ok_or(PathFault::UnresolvedSymlink)?;
        if !resolved.is_file() {
            return Err(PathFault::NotAFile);
        }
        Ok(RealHostsFile {
            configured: configured.to_path_buf(),
            resolved,
        })
    }

    /// How a message names this file.
    ///
    /// BOTH PATHS, ALWAYS, when they differ. A refusal that names `/etc/hosts`
    /// and a success that names `/etc/hosts.real` are two reports about the same
    /// operation that read as two different files.
    pub fn description(&self) -> String {
        if self.configured == self.resolved {
            return self.configured.display().to_string();
        }
        format!(
            "{} (resolved through its symlink chain to {})",
            self.configured.display(),
            self.resolved.display()
        )
    }
}

impl HostsFile for RealHostsFile {
    fn read(&self) -> Option<Vec<u8>> {
        fs::read(&self.resolved).ok()
    }

    /// Write beside the target, take the target's own metadata, then rename.
    ///
    /// THE TEMPORARY FILE IS REMOVED BY `Drop`, which is what the shell this
    /// replaced needed six signal traps to approximate: a killed run left a
    /// mode-0600 `hosts.XXXXXXXX` beside the target. Rust unwinds the same way
    /// for a panic and for an early return, and a process killed outright leaves
    /// a file either way.
    ///
    /// METADATA FROM THE TARGET, not from a constant. A hosts file whose mode or
    /// owner an operator has changed keeps them, and a rename that carried
    /// 0600 root:wheel onto a file that was 0644 would leave the machine unable
    /// to read its own hosts file.
    fn install(&self, contents: &[u8]) -> Result<(), String> {
        let target = self.resolved.as_path();
        let metadata =
            fs::metadata(target).map_err(|error| explain("reading its metadata", &error))?;
        let scratch = Scratch::beside(target, contents)?;
        fs::set_permissions(
            scratch.path(),
            fs::Permissions::from_mode(metadata.permissions().mode()),
        )
        .map_err(|error| explain("setting the rebuild's mode", &error))?;
        chown(scratch.path(), metadata.uid(), metadata.gid())
            .map_err(|error| explain("setting the rebuild's owner", &error))?;
        scratch.rename_onto(target)
    }
}

/// A file written beside the target, removed unless it is renamed away.
struct Scratch {
    path: Option<PathBuf>,
}

impl Scratch {
    /// Write `contents` to a fresh path beside `target`.
    ///
    /// BESIDE THE TARGET, not in a temporary directory, because the rename that
    /// installs it has to be atomic and a rename is only atomic within one
    /// filesystem.
    ///
    /// `create_new` is the exclusive create that makes the name ours: a
    /// collision is retried rather than overwritten, and a symlink planted at
    /// the name is refused rather than followed.
    fn beside(target: &Path, contents: &[u8]) -> Result<Self, String> {
        for attempt in 0..MAXIMUM_SCRATCH_ATTEMPTS {
            let mut name = target.as_os_str().to_os_string();
            name.push(format!(".tailnet-pin.{}.{attempt}", std::process::id()));
            let path = PathBuf::from(name);
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    let scratch = Scratch { path: Some(path) };
                    io::Write::write_all(&mut file, contents)
                        .map_err(|error| explain("writing the rebuild", &error))?;
                    file.sync_all()
                        .map_err(|error| explain("flushing the rebuild", &error))?;
                    return Ok(scratch);
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(explain("creating a file beside the target", &error)),
            }
        }
        Err("no free name beside the target".to_string())
    }

    fn path(&self) -> &Path {
        self.path
            .as_deref()
            .expect("a scratch file has a path until it is renamed")
    }

    /// Install this file over `target`, giving up ownership of the path so the
    /// guard does not remove what is now the real file.
    fn rename_onto(mut self, target: &Path) -> Result<(), String> {
        let path = self.path.take().expect("a scratch file is renamed once");
        fs::rename(&path, target).map_err(|error| {
            // The rename failed, so the scratch file is still ours to clean up.
            self.path = Some(path);
            explain("renaming the rebuild over the target", &error)
        })
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = fs::remove_file(path);
        }
    }
}

/// How many names are tried beside the target before giving up.
const MAXIMUM_SCRATCH_ATTEMPTS: u32 = 16;

fn explain(doing: &str, error: &io::Error) -> String {
    format!("{doing} failed: {error}")
}

/// Follow a symlink chain to the file it finally names.
///
/// Relative link targets resolve against the directory of the link that holds
/// them, and the hop count is what makes a loop refuse rather than spin.
fn resolve_symlink_chain(path: &Path) -> Option<PathBuf> {
    let mut path = path.to_path_buf();
    for _ in 0..MAXIMUM_SYMLINK_HOPS {
        if !fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_symlink()) {
            return Some(path);
        }
        let target = fs::read_link(&path).ok()?;
        path = if target.is_absolute() {
            target
        } else {
            path.parent()?.join(target)
        };
    }
    None
}

/// `chown` through libc, because `std::fs` has no owner setter.
///
/// SAFETY: the call takes a NUL-terminated path and two integer ids and touches
/// no memory this owns. The `CString` lives across the call, and a path holding
/// an interior NUL cannot be built into one, which is refused rather than
/// truncated.
fn chown(path: &Path, uid: u32, gid: u32) -> io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::other("the path holds a NUL byte"))?;
    // SAFETY: see the function's own note.
    if unsafe { libc_chown(path.as_ptr(), uid, gid) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

unsafe extern "C" {
    #[link_name = "chown"]
    fn libc_chown(path: *const std::ffi::c_char, uid: u32, gid: u32) -> i32;
}

#[cfg(test)]
#[path = "lib/tests.rs"]
mod tests;
