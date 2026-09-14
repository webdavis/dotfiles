use crate::legacy_json::{ProjectionFields, ProjectionInput, projected_field};
use posture_application::{FunnelGap, FunnelStateFailure, FunnelStore};
use posture_domain::{FunnelBaseline, FunnelState, funnel_baseline};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::{
    fs,
    fs::OpenOptions,
    io::{self, Read, Write},
    path::PathBuf,
};

pub struct FunnelStateFile {
    baseline: PathBuf,
}
impl FunnelStateFile {
    pub fn new(baseline: PathBuf) -> Self {
        Self { baseline }
    }
    fn sibling(&self, suffix: &str) -> PathBuf {
        let mut path = self.baseline.as_os_str().to_owned();
        path.push(suffix);
        path.into()
    }
    fn marker(&self, gap: FunnelGap) -> PathBuf {
        self.sibling(match gap {
            FunnelGap::Readings => ".gap",
            FunnelGap::Persistence => ".persist-gap",
        })
    }
    pub fn read(&self) -> FunnelBaseline {
        if !self.baseline.is_file() {
            return FunnelBaseline::Absent;
        }
        funnel_baseline(
            true,
            self.value().as_deref(),
            self.covered(FunnelGap::Persistence),
        )
    }
    fn value(&self) -> Option<String> {
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&self.baseline)
            .ok()?;
        if !file.metadata().ok()?.is_file() {
            return None;
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).ok()?;
        let input = ProjectionInput::new(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes))?;
        let fields: ProjectionFields<'_> = serde_json::from_str(&input.text).ok()?;
        projected_field(&input, &fields, "funnel")
    }
    fn write(&self, state: FunnelState) -> io::Result<()> {
        let value = match state {
            FunnelState::Active => "active",
            FunnelState::Inactive => "inactive",
        };
        let temporary = self.sibling(".tmp");
        {
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .mode(0o600)
                .open(&temporary)?;
            writeln!(file, "{{\"funnel\":\"{value}\"}}")?;
        }
        fs::rename(temporary, &self.baseline)?;
        // The Bash chmod follows replacement; a mode failure cannot undo the published bytes.
        fs::set_permissions(&self.baseline, fs::Permissions::from_mode(0o600))
    }
}
impl FunnelStore for FunnelStateFile {
    fn covered(&self, gap: FunnelGap) -> bool {
        self.marker(gap).is_file()
    }
    fn remember(&self, gap: FunnelGap) -> Result<(), FunnelStateFailure> {
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(self.marker(gap))
            .map(|_| ())
            .map_err(|_| FunnelStateFailure)
    }
    fn clear(&self, gap: FunnelGap) -> Result<(), FunnelStateFailure> {
        match fs::remove_file(self.marker(gap)) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result.map_err(|_| FunnelStateFailure),
        }
    }
    fn publish(&self, state: FunnelState) -> Result<(), FunnelStateFailure> {
        self.write(state).map_err(|_| FunnelStateFailure)
    }
}
impl Drop for FunnelStateFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.sibling(".tmp"));
    }
}
