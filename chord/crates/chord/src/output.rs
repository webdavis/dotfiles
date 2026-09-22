//! Writing a rendering to the file the table names.
//!
//! The write is atomic: the text lands in a working file beside the target
//! and is renamed over it, so a reader either sees the previous rendering or
//! the new one and never a half-written file.

/// Write `text` to `path`. The working file is removed when the rename it
/// was written for cannot happen, so a refused write leaves neither a
/// truncated target nor a stray file beside it.
pub fn write(path: &str, text: &str) -> Result<(), String> {
    let working = format!("{path}.chord-new");
    std::fs::write(&working, text).map_err(|fault| format!("cannot write {working}: {fault}"))?;
    std::fs::rename(&working, path).map_err(|fault| {
        let _ = std::fs::remove_file(&working);
        format!("cannot write {path}: {fault}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory of this test's own under the system temporary directory,
    /// named for the test so two of them cannot collide.
    fn scratch(name: &str) -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!("chord-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("a scratch directory");
        directory
    }

    #[test]
    fn the_text_reaches_the_named_file() {
        let path = scratch("writes").join("bindings.sh");
        write(&path.to_string_lossy(), "one\ntwo\n").expect("the write must land");
        assert_eq!(
            std::fs::read_to_string(&path).expect("the file must exist"),
            "one\ntwo\n"
        );
    }

    #[test]
    fn a_second_write_replaces_the_first_and_leaves_no_working_file_behind() {
        let directory = scratch("replaces");
        let path = directory.join("bindings.sh");
        write(&path.to_string_lossy(), "old\n").expect("the first write must land");
        write(&path.to_string_lossy(), "new\n").expect("the second write must land");
        assert_eq!(
            std::fs::read_to_string(&path).expect("the file must exist"),
            "new\n"
        );
        let left: Vec<String> = std::fs::read_dir(&directory)
            .expect("the directory must be readable")
            .map(|entry| {
                entry
                    .expect("an entry")
                    .file_name()
                    .to_string_lossy()
                    .into()
            })
            .collect();
        assert_eq!(left, ["bindings.sh"]);
    }

    #[test]
    fn a_write_into_a_directory_that_is_not_there_is_refused_by_path() {
        let path = scratch("missing").join("absent").join("bindings.sh");
        let refusal = write(&path.to_string_lossy(), "text\n")
            .expect_err("a write into a missing directory must be refused");
        assert!(refusal.contains("absent"), "{refusal}");
    }

    #[test]
    fn a_failed_write_leaves_the_file_that_was_there_untouched() {
        let directory = scratch("survives");
        let path = directory.join("bindings.sh");
        write(&path.to_string_lossy(), "good\n").expect("the first write must land");
        let _ = write(&directory.to_string_lossy(), "text\n");
        assert_eq!(
            std::fs::read_to_string(&path).expect("the file must still exist"),
            "good\n"
        );
    }
}
