use crate::*;
pub(crate) use pns_adapters::{now_secs, state_dir};

pub(crate) use pns_adapters::env_deadline;
/// Every override the engine reads, out of the process environment.
pub(crate) fn overrides_from_env() -> Overrides {
    Overrides::from_env(
        &std::env::vars_os()
            .map(|(name, value)| {
                (
                    name.to_string_lossy().into_owned(),
                    value.to_string_lossy().into_owned(),
                )
            })
            .collect::<BTreeMap<_, _>>(),
    )
}
/// The first executable of that name on PATH, absolute, or None. The click
/// string bakes it in because the click runs in a bare launchd context whose
/// PATH cannot find `~/.local/bin`.
pub(crate) fn executable_in_path(name: &str) -> Option<String> {
    use std::os::unix::fs::PermissionsExt;
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|directory| directory.join(name))
        .find(|candidate| {
            std::fs::metadata(candidate)
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        })
        .map(|path| path.to_string_lossy().into_owned())
}

pub(crate) use pns_adapters::resolve_path;

#[cfg(test)]
#[path = "runtime_environment/tests.rs"]
mod runtime_environment_tests;
