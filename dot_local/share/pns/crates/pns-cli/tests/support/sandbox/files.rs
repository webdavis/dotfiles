use super::Sandbox;

impl Sandbox {
    pub fn fired(&self, channel: &str) -> bool {
        self.path(&format!("{channel}.event")).exists()
    }

    /// The event one stub channel recorded, parsed.
    pub fn event(&self, channel: &str) -> serde_json::Value {
        let raw = std::fs::read_to_string(self.path(&format!("{channel}.event")))
            .unwrap_or_else(|_| panic!("{channel} recorded no event"));
        serde_json::from_str(&raw).unwrap_or_else(|error| panic!("{channel}: {error}: {raw}"))
    }

    /// The engine's config, written where its HOME will find it. The secrets
    /// live in this file now, so this is also how a native test arms a
    /// channel.
    pub fn write_config(&self, contents: &str) {
        let dir = self.path(".config/pns");
        std::fs::create_dir_all(&dir).expect("config dir");
        std::fs::write(dir.join("config.toml"), contents).expect("config file");
    }

    /// The macOS Focus store with `mode` asserted, written where the engine's
    /// own HOME will find it: one live assertion, and a catalog naming that
    /// mode `name`.
    ///
    /// THE SANDBOX'S HOME IS THE WHOLE SEAM. No variable names this path, for
    /// the reason no variable can set the operator's mute: one that could
    /// would let any producer force the answer in either direction. NOTHING
    /// HERE READS THE OPERATOR'S OWN STORE, which would answer differently on
    /// every run and on every machine.
    ///
    /// The shapes are the live ones off this machine, trimmed to the keys a
    /// reader navigates; `focus.rs`'s own tests carry them at full fidelity,
    /// duplicate assertion record and all.
    pub fn write_focus_store(&self, mode: &str, name: &str) {
        let dir = self.path("Library/DoNotDisturb/DB");
        std::fs::create_dir_all(&dir).expect("focus db dir");
        std::fs::write(
            dir.join("Assertions.json"),
            format!(
                r#"{{"data":[{{"storeInvalidationRecords":[],"storeInvalidationRequestRecords":[],
                "storeAssertionRecords":[{{"assertionUUID":"3CC0682F-2B5C-4C9D-95EB-93E0B5B2677A",
                "assertionStartDateTimestamp":809713980.03135,
                "assertionDetails":{{"assertionDetailsIdentifier":"com.apple.focus.activity-manager",
                "assertionDetailsModeIdentifier":"{mode}","assertionDetailsReason":"user-action"}}}}]}}],
                "header":{{"version":8,"timestamp":809744069.273127}}}}"#
            ),
        )
        .expect("the assertion store");
        std::fs::write(
            dir.join("ModeConfigurations.json"),
            format!(
                r#"{{"data":[{{"modeConfigurations":{{"{mode}":{{"dimsLockScreen":false,
                "mode":{{"name":"{name}","identifier":"586E30E1-1C59-45D9-B531-838B7759C1E2",
                "modeIdentifier":"{mode}","visibility":0}}}}}}}}],
                "header":{{"version":3,"timestamp":809128539.244755}}}}"#
            ),
        )
        .expect("the mode catalog");
    }
}
