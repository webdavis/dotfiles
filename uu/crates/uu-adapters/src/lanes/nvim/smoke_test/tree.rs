use crate::config::NvimSmokeTestLane;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub(super) struct Candidate {
    pub root: PathBuf,
    pub config: PathBuf,
}

impl Candidate {
    pub fn prepare(lane: &NvimSmokeTestLane, data: &Path) -> io::Result<Self> {
        let root = PathBuf::from(&lane.cache);
        private_dir(&root)?;
        let root = root.canonicalize()?;
        let config = Path::new(&lane.host.config).canonicalize()?;
        let mason = data.join("nvim/mason").canonicalize()?;
        for source in [&config, &mason] {
            if root.starts_with(source) || source.starts_with(&root) {
                return Err(io::Error::other(
                    "cache must be separate from the source config and Mason tree",
                ));
            }
        }
        for leaf in ["c", "d", "d/nvim", "s", "k", "h", "h/.claude"] {
            private_dir(&root.join(leaf))?;
        }
        replace_copy(&config, &root.join("c/nvim"))?;
        replace_copy(&mason, &root.join("d/nvim/mason"))?;
        Ok(Self {
            config: root.join("c/nvim"),
            root,
        })
    }
    pub fn environment(&self) -> Vec<String> {
        [
            ("HOME", "h"),
            ("CLAUDE_CONFIG_DIR", "h/.claude"),
            ("XDG_CONFIG_HOME", "c"),
            ("XDG_DATA_HOME", "d"),
            ("XDG_STATE_HOME", "s"),
            ("XDG_CACHE_HOME", "k"),
        ]
        .iter()
        .map(|(key, leaf)| format!("{key}={}", self.root.join(leaf).display()))
        .collect()
    }
}

fn private_dir(path: &Path) -> io::Result<()> {
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(io::Error::other("candidate directory is a symlink"));
    }
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

fn replace_copy(source: &Path, target: &Path) -> io::Result<()> {
    match fs::symlink_metadata(target) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(target)?,
        Ok(_) => return Err(io::Error::other("candidate copy target is not a directory")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return Err(error),
    }
    copy(source, target, &mut Vec::new())
}

fn copy(source: &Path, target: &Path, ancestors: &mut Vec<PathBuf>) -> io::Result<()> {
    let source = source.canonicalize()?;
    let metadata = fs::metadata(&source)?;
    if metadata.is_file() {
        fs::copy(source, target)?;
        return Ok(());
    }
    if !metadata.is_dir() || ancestors.contains(&source) {
        return Err(io::Error::other(
            "source copy contains a cycle or special file",
        ));
    }
    fs::create_dir(target)?;
    ancestors.push(source.clone());
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        copy(&entry.path(), &target.join(entry.file_name()), ancestors)?;
    }
    ancestors.pop();
    fs::set_permissions(target, metadata.permissions())
}
