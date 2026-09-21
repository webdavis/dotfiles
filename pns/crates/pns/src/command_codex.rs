//! `pns codex install-hooks`: pns's four hooks, wired into Codex.
//!
//! THE COMMAND IT INSTALLS NAMES THIS BINARY, taken from `current_exe` and
//! canonicalized, so the hook a run wires up calls the engine that wired it
//! rather than whatever later answers to the name `pns`.

use pns_adapters::CodexHooksInstall;

pub(crate) const CODEX_USAGE: &str = "pns: usage: pns codex install-hooks; merges pns's four \
hooks into ~/.codex/hooks.json and leaves every other handler alone";

pub(crate) fn codex_mode(verb: &str) -> i32 {
    match verb {
        "install-hooks" => install_hooks(),
        // UNKNOWN IS AN ERROR, never a silent fallthrough: an operator who
        // mistyped the verb believes the hooks are wired.
        _ => {
            eprintln!("{CODEX_USAGE}");
            2
        }
    }
}

fn install_hooks() -> i32 {
    if !crate::arguments_after_verb().is_empty() {
        eprintln!("{CODEX_USAGE}");
        return 2;
    }
    let home = match std::env::var("HOME") {
        Ok(home) if !home.is_empty() => home,
        _ => {
            eprintln!("pns: HOME is unset, so there is no Codex hooks file to merge into");
            return 2;
        }
    };
    let Some(binary) = running_binary() else {
        eprintln!("pns: cannot resolve this binary's own path, so there is no command to install");
        return 2;
    };
    match pns_adapters::install_codex_hooks(&home, &binary) {
        // Codex ignores a new or changed hook until the operator reviews it,
        // so a run that changed something says so once.
        Ok(CodexHooksInstall::Changed) => {
            eprintln!(
                "pns: added or changed Codex hooks in {}. Codex will IGNORE them until you review \
and trust them: open Codex and run /hooks to approve.",
                pns_adapters::codex_hooks_path(&home).display()
            );
            0
        }
        Ok(CodexHooksInstall::Unchanged) => 0,
        Err(refusal) => {
            eprintln!("{refusal}");
            2
        }
    }
}

fn running_binary() -> Option<String> {
    let path = std::env::current_exe().ok()?;
    let path = path.canonicalize().unwrap_or(path);
    path.into_os_string().into_string().ok()
}
