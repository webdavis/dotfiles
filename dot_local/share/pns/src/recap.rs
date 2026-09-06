//! The return recap's body: one window of activity, said in one message.
//!
//! POLICY ONLY, in `missed_notifications`'s style: every function here is a
//! total function of its arguments, with no config, no clock, no environment,
//! no file and no printing. The composition root reads the window off the
//! activity ring, resolves the local wall clock, and posts what comes back.
//!
//! THE COUNT NEVER LIES, which is the rule the whole module is arranged
//! around. The header's count is the length of the window that was READ, and
//! it is composed before anything is cut, so a body that ran out of room still
//! names a total it can back. The budget then cuts LINES and the LENGTH OF A
//! LINE, never a count, and never a line that says something needs the
//! operator.
//!
//! TWO BUDGETS, BOTH ENFORCED. Twenty-five lines is the locked one, and a
//! character ceiling sits beside it because the locked property is ONE Discord
//! message and a line has a length: twenty-five full-width timeline lines
//! MEASURED at 2,859 characters, and the gateway splits at 1,900. `fit` owns
//! both.
//!
//! THE PRIVACY RULE IS THE JOURNAL'S, INHERITED. These lines carry the
//! operator's own text, so nothing here prints: the caller posts the body to
//! the same durable route the live events already reached, and no pns command
//! renders it to a terminal.

// THE RECAP COMPOSITION moved to `pns-domain`, one file per part of the body.
// Nothing stays here but the re-exports its callers read and the tests.
pub use pns_domain::recap::budget::{MAX_CHARS, fit};
pub use pns_domain::recap::external::{
    External, Externals, Found, Sourced, merged, noted, unreadable,
};
pub use pns_domain::recap::night::NOTHING_HAPPENED;
pub use pns_domain::recap::prompt::{MAX_ANSWER_BYTES, answer, merge_prompt, note_prompt, prompt};
pub use pns_domain::recap::sanitize::is_invisible;
pub use pns_domain::recap::sections::{Timeline, body};
