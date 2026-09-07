use crate::*;

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

/// The `setup` mode: the first-run walk, and the only writer of the config.
///
/// A THIN EDGE OVER A PURE COMPOSER. Everything about what lands in the file
/// is `pns::setup`; this asks, reads a line, and publishes. It EXITS NON-ZERO
/// on every refusal, which the always-exit-0 contract permits for the same
/// reason `quiet` does: that contract covers the hook and notification paths,
/// where a non-zero exit fails the turn being reported on, and this is hand
/// typed and is never a hook.
///
/// IT REFUSES A WALK NOBODY CAN ANSWER. Without a terminal there is no walk,
/// and guessing every answer would write a config the operator never agreed
/// to, over one they may already have.
pub(crate) fn setup_mode() -> i32 {
    let arguments: Vec<String> = std::env::args_os()
        .skip(2)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let force = match arguments.as_slice() {
        [] => false,
        [word] if word == "--force" => true,
        // ANY OTHER WORD IS A REFUSAL, never a silent fallthrough to the walk:
        // a mistyped `--force` that walked anyway would ask ten questions and
        // then refuse at the end, over a config it was told to replace.
        _ => {
            eprintln!("{SETUP_USAGE}");
            return 2;
        }
    };
    // AN EMPTY HOME IS REFUSED BY NAME, before the config is even located: an
    // unset or empty HOME would otherwise compose a config path relative to
    // the current directory, which is not the operator's own machine-wide
    // config no matter where this happened to be run from.
    let Some(home) = std::env::var("HOME").ok().filter(|home| !home.is_empty()) else {
        eprintln!("pns setup: HOME is unset or empty; nothing was written");
        return 2;
    };
    // THE CONFIG IS CHECKED BEFORE THE TERMINAL IS, because it is the more
    // specific answer: an operator who already has one is told that, whether
    // or not they are sitting in front of the questions.
    let path = config_path(&home);
    // `symlink_metadata`, NOT `exists`: `exists` follows a symlink and asks
    // what it resolves to, so a dangling one at the config name reads as
    // nothing at all here and the whole walk runs before the publish refuses
    // it with a claim that it "appeared while the questions were being
    // answered", which would not be true.
    match path.symlink_metadata() {
        Ok(_) if !force => {
            eprintln!(
                "pns setup: {} already exists; pass --force to replace it, \
                 which keeps the old file beside it",
                path.display()
            );
            return 2;
        }
        Ok(_) => {}
        // NOTHING AT THE NAME IS NOT YET NOTHING IN THE WAY: a dangling link
        // above the config reports `NotFound` here too, and it refuses
        // REGARDLESS OF `--force`, because what `--force` agrees to replace
        // is a config, not a path that leads nowhere.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Some((ancestor, cause)) = unresolvable_ancestor(&path) {
                eprintln!(
                    "pns setup: {} could not be checked: {} does not resolve ({cause}); \
                     nothing was written",
                    path.display(),
                    ancestor.display()
                );
                return 2;
            }
        }
        // ANY OTHER ERROR REFUSES REGARDLESS OF --force: the comment above
        // only holds for NotFound, and a directory this walk cannot even
        // stat is not one it can safely publish into either.
        Err(error) => {
            eprintln!(
                "pns setup: {} could not be checked: {error}; nothing was written",
                path.display()
            );
            return 2;
        }
    }
    if !std::io::stdin().is_terminal() {
        eprintln!(
            "pns setup: this is a walk through questions and stdin is not a terminal; \
             nothing was written"
        );
        return 2;
    }
    let answers = match walk() {
        Ok(answers) => answers,
        Err(reason) => {
            eprintln!("pns setup: {reason}; nothing was written");
            return 2;
        }
    };
    let composed = pns::setup::compose_config(&answers);
    // THROUGH THE ENGINE'S OWN PARSER BEFORE IT IS PUBLISHED. A wizard that
    // writes a config pns then refuses is worse than no wizard: it leaves a
    // machine falling back to the core with a complaint nobody is standing in
    // front of, and it does it while the operator is being told it worked.
    if let Err(error) = pns::config::parse_config(&composed) {
        eprintln!(
            "pns setup: what it composed does not load ({}); nothing was written",
            error.detail()
        );
        return 2;
    }
    match publish_config(&path, &composed, force) {
        Ok(backup) => {
            if let Some(backup) = backup {
                println!("pns setup: kept the old config at {}", backup.display());
            }
            println!("pns setup: wrote {}", path.display());
            0
        }
        Err(refusal) => {
            eprintln!("pns setup: {refusal}");
            1
        }
    }
}
/// What a setup typed wrong is told.
const SETUP_USAGE: &str =
    "pns: usage: pns setup [--force]; --force replaces an existing config, keeping it beside";
