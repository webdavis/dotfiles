use std::path::{Path, PathBuf};

/// Where an existing config is kept when `--force` replaces it: a sibling of
/// the config, stamped with the instant it was moved aside.
///
/// A SIBLING because the config's own directory is the one place this wizard
/// already knows it can write, and STAMPED so a second forced run cannot land
/// on the first one's backup. The stamp is UTC and carries no colons: it is a
/// discriminator in a file name rather than a clock anybody reads, and the
/// caller prints the whole path.
///
/// NO CLOCK, NO NAME, and the caller turns that into a refusal: replacing a
/// config whose copy cannot be named is the one outcome that loses the file.
pub fn backup_path(config: &Path, epoch_secs: u64) -> Option<PathBuf> {
    let stamp = crate::utc_timestamp(epoch_secs)?
        .replace(':', "-")
        .replace('Z', "");
    let name = config.file_name()?.to_str()?;
    Some(config.with_file_name(format!("{name}.{stamp}.backup")))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_backup_sits_beside_the_config_stamped_with_the_instant_it_was_moved() {
        // A SIBLING, because the directory is the one place the wizard already
        // knows it can write, and a stamp rather than a `.bak` so a second
        // forced run cannot land on the first one's name.
        assert_eq!(
            backup_path(Path::new("/home/x/.config/pns/config.toml"), 1_800_000_000),
            Some(PathBuf::from(
                "/home/x/.config/pns/config.toml.2027-01-15T08-00-00.backup"
            ))
        );
    }

    #[test]
    fn a_clock_that_cannot_be_read_names_no_backup_at_all() {
        // NO NAME IS THE REFUSAL the caller turns into "nothing was written":
        // replacing a config whose copy cannot be named is the one outcome
        // that loses the file.
        assert_eq!(backup_path(Path::new("/x/config.toml"), u64::MAX), None);
    }
}
