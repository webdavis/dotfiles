use std::path::{Path, PathBuf};

/// The first ancestor of `path` that exists in its own right but resolves to
/// nothing, and why it does not resolve.
///
/// WHAT THIS IS FOR: `NotFound` at the config path is not proof the config is
/// absent. A dangling link ANYWHERE ABOVE it (`~/.config/pns` naming a
/// directory that was moved or never created) fails the leaf's own stat with
/// ENOENT, exactly as a genuinely missing config does. Told apart nowhere,
/// that reading walks the whole questionnaire and only fails at publication,
/// with every answer already typed and every secret already handed over.
///
/// IT CLIMBS ONLY AS FAR AS THE FIRST COMPONENT THAT EXISTS. Above that
/// everything resolves by definition, and below it the components really are
/// missing, which is the ordinary first run this must not refuse.
fn unresolvable_ancestor(path: &Path) -> Option<(PathBuf, std::io::Error)> {
    // `skip(1)`: `path` ITSELF has already been stated by the caller, and it
    // is the leaf's own `NotFound` that brought us here.
    for ancestor in path.ancestors().skip(1) {
        match ancestor.symlink_metadata() {
            // NOT THERE AS A NAME AT ALL: keep climbing. The component under
            // it is genuinely missing rather than broken.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            // UNREADABLE, NOT ABSENT: refuse by the same rule the leaf's own
            // non-NotFound arm refuses under.
            Err(error) => return Some((ancestor.to_path_buf(), error)),
            // A NAME IS STANDING HERE. Whether it LEADS anywhere is the whole
            // question: `metadata` follows the link `symlink_metadata` did
            // not, so a dangling one (or a loop, or a file where a directory
            // belongs) answers with its own cause here.
            Ok(_) => {
                return match ancestor.metadata() {
                    Ok(_) => None,
                    Err(error) => Some((ancestor.to_path_buf(), error)),
                };
            }
        }
    }
    None
}

pub fn check_config_path(path: &Path, force: bool) -> Result<(), String> {
    match path.symlink_metadata() {
        Ok(entry) if entry.is_dir() => {
            return Err(format!(
                "pns setup: {} is a directory; nothing was written",
                path.display()
            ));
        }
        Ok(_) if !force => {
            return Err(format!(
                "pns setup: {} already exists; pass --force to replace it, \
                 which keeps the old file beside it",
                path.display()
            ));
        }
        Ok(_) => {}
        // NOTHING AT THE NAME IS NOT YET NOTHING IN THE WAY: a dangling link
        // above the config reports `NotFound` here too, and it refuses
        // REGARDLESS OF `--force`, because what `--force` agrees to replace
        // is a config, not a path that leads nowhere.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Some((ancestor, cause)) = unresolvable_ancestor(path) {
                return Err(format!(
                    "pns setup: {} could not be checked: {} does not resolve ({cause}); \
                     nothing was written",
                    path.display(),
                    ancestor.display()
                ));
            }
        }
        // ANY OTHER ERROR REFUSES REGARDLESS OF --force: the comment above
        // only holds for NotFound, and a directory this walk cannot even
        // stat is not one it can safely publish into either.
        Err(error) => {
            return Err(format!(
                "pns setup: {} could not be checked: {error}; nothing was written",
                path.display()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
