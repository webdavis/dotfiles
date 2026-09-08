//! Which channels an event reaches, driven through the real binary.
//!
//! `PNS_CHANNELS_DIR` points the engine at stub executables that record the
//! event they were handed, which is what lets these pin routing, the
//! rendered event, the pane scrub and the exit-0 edge without a network, a
//! key or a sleep. The native plugins are the other half, in native.rs.

#[path = "dispatch/captured_events.rs"]
mod captured_events;
#[path = "dispatch/quiet_records.rs"]
mod quiet_records;
#[path = "support/stored_records.rs"]
mod stored_records;
#[path = "dispatch/submit_json.rs"]
mod submit_json;
mod support;

use captured_events::events;
use stored_records::database;

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use support::{
    KEYS_DISAGREE, RouterStub, Sandbox, poll_until, router_table, run, stderr, stdout, write_script,
};

#[path = "dispatch/activity.rs"]
mod activity;
#[path = "dispatch/channel_roster.rs"]
mod channel_roster;
#[path = "dispatch/channel_settings.rs"]
mod channel_settings;
#[path = "dispatch/doctor_channels.rs"]
mod doctor_channels;
#[path = "dispatch/doctor_delivery.rs"]
mod doctor_delivery;
#[path = "dispatch/doctor_fixture.rs"]
mod doctor_fixture;
use doctor_fixture::*;
#[path = "dispatch/doctor_focus.rs"]
mod doctor_focus;
#[path = "dispatch/doctor_journal.rs"]
mod doctor_journal;
#[path = "dispatch/doctor_pairing.rs"]
mod doctor_pairing;
#[path = "dispatch/doctor_presence.rs"]
mod doctor_presence;
#[path = "dispatch/doctor_records.rs"]
mod doctor_records;
#[path = "dispatch/event_fixture.rs"]
mod event_fixture;
use event_fixture::*;
#[path = "dispatch/focus.rs"]
mod focus;
#[path = "dispatch/hermes_lines.rs"]
mod hermes_lines;
#[path = "dispatch/home_alerts.rs"]
mod home_alerts;
#[path = "dispatch/home_diagnostic.rs"]
mod home_diagnostic;
#[path = "dispatch/home_fixture.rs"]
mod home_fixture;
use home_fixture::*;
#[path = "dispatch/home_setup.rs"]
mod home_setup;
#[path = "dispatch/journal.rs"]
mod journal;
#[path = "dispatch/lamp_fixture.rs"]
mod lamp_fixture;
use lamp_fixture::*;
#[path = "dispatch/lights_fixture.rs"]
mod lights_fixture;
use lights_fixture::*;
#[path = "dispatch/lights_records.rs"]
mod lights_records;
#[path = "dispatch/lights_routes.rs"]
mod lights_routes;
#[path = "dispatch/lights_tick.rs"]
mod lights_tick;
#[path = "dispatch/loop_lease.rs"]
mod loop_lease;
#[path = "dispatch/mute_refusals.rs"]
mod mute_refusals;
#[path = "dispatch/mutes.rs"]
mod mutes;
#[path = "dispatch/overrides.rs"]
mod overrides;
#[path = "dispatch/phone_surface.rs"]
mod phone_surface;
#[path = "dispatch/plan_rows.rs"]
mod plan_rows;
#[path = "dispatch/producer_argv.rs"]
mod producer_argv;
#[path = "dispatch/producer_events.rs"]
mod producer_events;
#[path = "dispatch/quiet_fixture.rs"]
mod quiet_fixture;
use quiet_fixture::*;
#[path = "dispatch/quiet_window.rs"]
mod quiet_window;
#[path = "dispatch/recap_card.rs"]
mod recap_card;
#[path = "dispatch/recap_fixture.rs"]
mod recap_fixture;
use recap_fixture::*;
#[path = "dispatch/recap_merges.rs"]
mod recap_merges;
#[path = "dispatch/recap_notes.rs"]
mod recap_notes;
#[path = "dispatch/recap_process.rs"]
mod recap_process;
#[path = "dispatch/recap_routes.rs"]
mod recap_routes;
#[path = "dispatch/record_fixture.rs"]
mod record_fixture;
use record_fixture::*;
#[path = "dispatch/records.rs"]
mod records;
#[path = "dispatch/replay.rs"]
mod replay;
#[path = "dispatch/replay_claims.rs"]
mod replay_claims;
#[path = "dispatch/replay_fixture.rs"]
mod replay_fixture;
use replay_fixture::*;
#[path = "dispatch/replay_races.rs"]
mod replay_races;
#[path = "dispatch/replay_refusals.rs"]
mod replay_refusals;
#[path = "dispatch/return_fixture.rs"]
mod return_fixture;
use return_fixture::*;
#[path = "dispatch/return_moment.rs"]
mod return_moment;
#[path = "dispatch/return_window.rs"]
mod return_window;
#[path = "dispatch/setup_argv.rs"]
mod setup_argv;
#[path = "dispatch/summarizer.rs"]
mod summarizer;
#[path = "dispatch/summarizer_failures.rs"]
mod summarizer_failures;
#[path = "dispatch/tick_fixture.rs"]
mod tick_fixture;
use tick_fixture::*;
