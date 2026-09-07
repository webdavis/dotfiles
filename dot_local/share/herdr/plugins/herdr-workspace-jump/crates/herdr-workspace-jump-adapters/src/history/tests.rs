use super::*;

fn mru(current: &str, previous: &str) -> Mru {
    Mru {
        current: current.to_string(),
        previous: previous.to_string(),
    }
}

struct TempState(PathBuf);

impl TempState {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("hwj-mru-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }

    fn path(&self) -> PathBuf {
        self.0.join("mru")
    }
}

impl Drop for TempState {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn state_file_prefers_the_injected_directory_over_the_home_default() {
    assert_eq!(
        state_file(Some("/tmp/state-override"), Some("/home/ignored")),
        PathBuf::from("/tmp/state-override/mru")
    );
    assert_eq!(
        state_file(None, Some("/home/me")),
        PathBuf::from("/home/me/.local/state/herdr/plugins/herdr-workspace-jump/mru")
    );
    assert_eq!(
        state_file(Some(""), Some("/home/me")),
        PathBuf::from("/home/me/.local/state/herdr/plugins/herdr-workspace-jump/mru")
    );
}

#[test]
fn read_at_treats_a_missing_or_short_file_as_a_cold_start() {
    let state = TempState::new("cold");
    assert_eq!(read_at(&state.path()), Mru::default());
    write_at(&state.path(), &mru("wA", ""));
    assert_eq!(read_at(&state.path()), mru("wA", ""));
}
