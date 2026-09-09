use super::{SqliteStore, StoreError, scalar::Scalar};
impl SqliteStore {
    pub fn quiet_expiry(&self) -> Result<Option<u64>, StoreError> {
        let connection = self.connect()?;
        // Retained text keeps the original parse complaint. A missing row
        // with a failed import remains unknown rather than an absent mute.
        let expiry = Scalar::Quiet
            .stored(&connection)?
            .map(|body| {
                pns_domain::quiet::expiry_from_state(&body).map_err(StoreError::InvalidState)
            })
            .transpose()?;
        super::import::readable(&connection, "quiet-until").map_err(|_| {
            StoreError::InvalidState(
                "pns: state error (quiet-until could not be read: legacy import failed); \
                 nothing is muted, clear it with pns quiet off"
                    .into(),
            )
        })?;
        Ok(expiry)
    }
    pub fn set_quiet_expiry(&self, expiry: Option<u64>) -> Result<(), StoreError> {
        self.transaction(|transaction| {
            Scalar::Quiet.write(transaction, expiry.map(|at| at.to_string()).as_deref())
        })
    }
    pub fn staleness(&self) -> Result<Option<String>, StoreError> {
        Ok(Scalar::Staleness
            .read(&self.connect()?)?
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()))
    }
    pub fn remember_staleness(&self, episode: Option<&str>) -> Result<(), StoreError> {
        self.transaction(|transaction| Scalar::Staleness.write(transaction, episode))
    }
    pub fn lights_complaint(&self) -> Result<Option<String>, StoreError> {
        Scalar::LightsComplaint.read(&self.connect()?)
    }
    pub fn remember_lights_complaint(&self, said: Option<&str>) -> Result<(), StoreError> {
        self.transaction(|transaction| Scalar::LightsComplaint.write(transaction, said))
    }
    pub fn quiet_complaint(&self) -> Result<Option<String>, StoreError> {
        Scalar::QuietComplaint.read(&self.connect()?)
    }
    pub fn remember_quiet_complaint(&self, said: Option<&str>) -> Result<(), StoreError> {
        self.transaction(|transaction| Scalar::QuietComplaint.write(transaction, said))
    }
}
