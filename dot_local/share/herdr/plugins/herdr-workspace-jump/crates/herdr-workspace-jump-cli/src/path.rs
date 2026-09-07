use std::path::PathBuf;

/// Expand a leading `~`, which herdr's argv-only action commands cannot do.
pub(crate) fn expand_tilde(raw: &str, home: Option<&str>) -> Result<PathBuf, String> {
    let tail = match raw.strip_prefix('~') {
        None => return Ok(PathBuf::from(raw)),
        Some(tail) => tail,
    };
    if !tail.is_empty() && !tail.starts_with('/') {
        return Err(format!("cannot expand another user's home in {raw}"));
    }
    let home = home
        .filter(|home| !home.is_empty())
        .ok_or_else(|| format!("cannot expand {raw}: HOME is unset"))?;
    Ok(PathBuf::from(home).join(tail.trim_start_matches('/')))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_tilde_joins_a_leading_tilde_onto_home() {
        assert_eq!(
            expand_tilde("~/workspaces/Ivy", Some("/Users/me")),
            Ok(PathBuf::from("/Users/me/workspaces/Ivy"))
        );
        assert_eq!(
            expand_tilde("~", Some("/Users/me")),
            Ok(PathBuf::from("/Users/me"))
        );
    }
    #[test]
    fn expand_tilde_leaves_an_absolute_or_relative_path_alone() {
        assert_eq!(
            expand_tilde("/opt/project", Some("/Users/me")),
            Ok(PathBuf::from("/opt/project"))
        );
        assert_eq!(
            expand_tilde("relative/path", Some("/Users/me")),
            Ok(PathBuf::from("relative/path"))
        );
        // A tilde anywhere but the front is an ordinary character.
        assert_eq!(
            expand_tilde("/opt/~backup", Some("/Users/me")),
            Ok(PathBuf::from("/opt/~backup"))
        );
    }
    #[test]
    fn expand_tilde_refuses_another_users_home_and_a_missing_home() {
        assert!(expand_tilde("~other/project", Some("/Users/me")).is_err());
        assert!(expand_tilde("~/project", None).is_err());
        assert!(expand_tilde("~/project", Some("")).is_err());
    }
}
