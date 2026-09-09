use super::*;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

impl PollStateFiles {
    pub fn publish(
        &self,
        update: &posture_domain::BaselineUpdate,
        current: &crate::PostureTrio,
    ) -> io::Result<()> {
        let encoded = super::encoding::encode(update, current, self.prior_json.as_deref())
            .ok_or(io::ErrorKind::InvalidData)?;
        self.write_with(&encoded, |path| {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        })
    }
    pub(super) fn write_with(
        &self,
        encoded: &str,
        mode: impl FnOnce(&Path) -> io::Result<()>,
    ) -> io::Result<()> {
        let temporary = self.sibling(".tmp");
        {
            // Creation is private. An existing sibling retains its old mode until chmod.
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&temporary)?;
            writeln!(file, "{encoded}")?;
        }
        fs::rename(temporary, &self.baseline)?;
        // Failure here reports refusal but cannot undo the already-completed replacement.
        mode(&self.baseline)
    }
}
