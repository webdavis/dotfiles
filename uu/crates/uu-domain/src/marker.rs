/// What the last-successful-run marker says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Marker {
    /// No marker at all: this machine has never recorded a successful run.
    NeverRecorded,
    /// A marker that is there and says nothing usable.
    Unreadable,
    Recorded {
        epoch: i64,
        iso: String,
    },
}
