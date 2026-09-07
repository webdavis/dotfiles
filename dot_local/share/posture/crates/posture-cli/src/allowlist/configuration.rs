use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

pub(super) struct Configuration {
    pub home: PathBuf,
    pub deployed: PathBuf,
    pub osqueryi: PathBuf,
    pub chezmoi: PathBuf,
    pub manifest: Option<PathBuf>,
}
impl Configuration {
    pub(super) fn read(mut variable: impl FnMut(&str) -> Option<OsString>) -> Self {
        let home = PathBuf::from(variable("HOME").unwrap_or_default());
        let path = variable("PATH").unwrap_or_default();
        let mut nonempty = |name| variable(name).filter(|value| !value.is_empty());
        Self {
            deployed: nonempty("OSQUERY_LAUNCHD_ALLOWLIST")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    let mut target = home.as_os_str().to_os_string();
                    target.push("/.config/osquery/page-launchd-allowlist.txt");
                    target.into()
                }),
            osqueryi: nonempty("OSQUERYI").map(PathBuf::from).unwrap_or_else(|| {
                executable("osqueryi", &path).unwrap_or_else(|| "/usr/local/bin/osqueryi".into())
            }),
            chezmoi: nonempty("CHEZMOI").map(PathBuf::from).unwrap_or_else(|| {
                executable("chezmoi", &path).unwrap_or_else(|| "chezmoi".into())
            }),
            manifest: nonempty("OSQUERY_PIPELINE_MANIFEST_RUNNER").map(PathBuf::from),
            home,
        }
    }
}
fn executable(name: &str, path: &std::ffi::OsStr) -> Option<PathBuf> {
    std::env::split_paths(path)
        .map(|directory| directory.join(name))
        .find(|file| {
            file.metadata()
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        })
}

#[cfg(test)]
mod tests;
