use super::*;
impl<R: CommandRunner> SystemProbes<R> {
    pub fn new(runner: R, marker_path: String) -> Self {
        Self {
            runner: Arc::new(runner),
            marker_path,
            tty_dir: TTY_DIR.to_string(),
            idle: std::cell::OnceCell::new(),
            marker_mtime: std::cell::OnceCell::new(),
            phone_atime: std::cell::OnceCell::new(),
            screen_locked: std::cell::OnceCell::new(),
            now: std::cell::OnceCell::new(),
            presence_path: String::new(),
            presence_line: std::cell::OnceCell::new(),
            desk_handle: std::cell::Cell::new(None),
            phone_handle: std::cell::Cell::new(None),
        }
    }

    /// The wall clock, taken once and remembered like the four probes beside
    /// it: see the struct doc. THE FIFTH MEMOIZED READING. Of the four beside
    /// it, only the phone atime and the marker mtime are epochs aged against
    /// this clock; idle is already an age and screen lock is a boolean. Which
    /// is why forwarding to the phone and deciding what to deliver must read
    /// this same cell rather than each taking their own: a second boundary
    /// between two wall-clock reads is what drifted a phone reading and a
    /// desk reading apart in R4-1. AN UNREADABLE CLOCK IS REMEMBERED TOO: the
    /// first reader's `None` is the second reader's `None`, so the two can
    /// never disagree about whether there was a clock at all.
    pub fn now_secs(&self) -> Option<u64> {
        *self.now.get_or_init(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|since_epoch| since_epoch.as_secs())
        })
    }

    /// THE SIXTH MEMOIZED READING: the presence state file's one line, RAW.
    ///
    /// NOTHING IS JUDGED HERE. No clock, no staleness bound, no room list:
    /// this module says what the machine reported and `presence::classify`
    /// says what it means, which is the split the module doc opens with. It
    /// is memoized like the five beside it, so the doctor and the eventual
    /// routing cannot read two different lines inside one event.
    ///
    /// READ THROUGH THE BOUNDED READER, so a FIFO at that path is refused on
    /// its metadata rather than opened, and a file some other hand grew is
    /// refused rather than allocated.
    pub fn presence_line(&self) -> Option<String> {
        self.presence_line
            .get_or_init(|| {
                readable_state_file(
                    std::path::Path::new(&self.presence_path),
                    crate::PRESENCE_READ_MAX,
                )
                .ok()
            })
            .clone()
    }

    /// Points this probe set at the presence state file, AND TAKES THE READING
    /// THERE AND THEN.
    ///
    /// A BUILDER RATHER THAN A THIRD CONSTRUCTOR ARGUMENT, and the default is
    /// no path at all. The composition root sets it; anything that does not
    /// reads no line, which is the same Unknown a machine with the daemon
    /// switched off already gets, so a caller that never wires it is degraded
    /// rather than wrong.
    ///
    /// EAGER BECAUSE THE CLOCK IS THE OTHER HALF OF THE READING, and the two
    /// are compared. Read lazily, the line was taken whenever something first
    /// asked, which was always AFTER `now_secs` froze: the event path asks
    /// once its delivery plan is decided, the blocked path freezes the clock
    /// before it even forwards to moshi, and the doctor's own reading asks a
    /// line after taking the clock a statement earlier. The daemon polls every
    /// few seconds, so a line published inside any of those windows carried an
    /// epoch NEWER than the frozen clock, `classify` answered `Future`, and a
    /// room the operator was standing in became the whole house again.
    ///
    /// TAKEN HERE, THE LINE CANNOT BE NEWER THAN THE CLOCK, because this is
    /// the only way to point a probe set at the file at all: every caller
    /// reaches it through this builder before it holds a clock. The reading
    /// may instead be up to one window OLD, which is the safe direction and
    /// the one the staleness bound already exists to judge.
    /// AND POINTING IT AGAIN RE-READS, because the composition root points
    /// every set at the daemon's own file and a caller that then points one
    /// somewhere else meant the second path. Left sealed by the first read,
    /// the second call would silently keep answering the first file's line.
    pub fn with_presence_path(mut self, path: String) -> Self {
        self.presence_path = path;
        self.presence_line.take();
        let _ = self.presence_line();
        self
    }
}

impl<R: CommandRunner + Send + Sync + 'static> pns_application::IdleProbe for SystemProbes<R> {
    fn idle_secs(&self) -> Option<u64> {
        self.join_desk();
        *self.idle.get_or_init(|| idle_reading(&*self.runner))
    }
}

impl<R: CommandRunner + Send + Sync + 'static> pns_application::ScreenLockProbe
    for SystemProbes<R>
{
    fn screen_locked(&self) -> Option<bool> {
        self.join_desk();
        *self
            .screen_locked
            .get_or_init(|| lock_reading(&*self.runner))
    }
}

impl<R: CommandRunner> pns_application::PhoneMarkerProbe for SystemProbes<R> {
    fn marker_mtime_secs(&self) -> Option<u64> {
        *self.marker_mtime.get_or_init(|| {
            // The LINK itself, never its target, matching BSD `stat -f %m`: the
            // Back Tap touch lands on this path, so a dangling link still
            // carries the reading and following it would erase one.
            let modified = std::fs::symlink_metadata(&self.marker_path)
                .ok()?
                .modified()
                .ok()?;
            Some(
                modified
                    .duration_since(std::time::UNIX_EPOCH)
                    .ok()?
                    .as_secs(),
            )
        })
    }
}

impl<R: CommandRunner + Send + Sync + 'static> pns_application::PhoneInputProbe
    for SystemProbes<R>
{
    fn phone_input_atime_secs(&self) -> Option<u64> {
        self.join_phone();
        *self
            .phone_atime
            .get_or_init(|| phone_reading(&*self.runner, &self.tty_dir))
    }
}

impl<R: CommandRunner> pns_application::Clock for SystemProbes<R> {
    fn now_secs(&self) -> Option<u64> {
        SystemProbes::now_secs(self)
    }
}
