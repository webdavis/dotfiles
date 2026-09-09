use crate::InspectionFailure;
use posture_domain::{ParentPid, RestartBounds};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VendorPlist {
    Regular,
    Symlink,
    Missing,
}

pub trait OsqueryControl {
    fn vendor_plist(&mut self) -> VendorPlist;
    fn config_check(&mut self) -> Result<(), InspectionFailure>;
    fn stop(&mut self) -> Result<(), InspectionFailure>;
    fn start(&mut self) -> Result<(), InspectionFailure>;
}

pub trait ProcessTable {
    fn daemon_parent(&mut self) -> Result<Option<ParentPid>, InspectionFailure>;
}

pub trait RestartClock {
    fn elapsed(&self) -> Duration;
    fn sleep(&mut self, duration: Duration);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartFailure {
    VendorPlist(VendorPlist),
    Configuration(InspectionFailure),
    Start(InspectionFailure),
    ParentProbe(InspectionFailure),
    UnchangedParent(ParentPid),
    Deadline,
    UnstableParent(ParentPid),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Restarted {
    pub parent: ParentPid,
    pub settled_for: Duration,
}

pub fn restart_daemon(
    control: &mut impl OsqueryControl,
    processes: &mut impl ProcessTable,
    clock: &mut impl RestartClock,
    bounds: RestartBounds,
) -> Result<Restarted, RestartFailure> {
    let vendor = control.vendor_plist();
    if vendor != VendorPlist::Regular {
        return Err(RestartFailure::VendorPlist(vendor));
    }
    control
        .config_check()
        .map_err(RestartFailure::Configuration)?;
    let previous = processes
        .daemon_parent()
        .map_err(RestartFailure::ParentProbe)?;
    // A fresh host may have nothing to unload. Its exit is not restart evidence.
    let _ = control.stop();
    control.start().map_err(RestartFailure::Start)?;
    let parent = wait_for_parent(processes, clock, bounds.deadline(), previous)?;
    let started = clock.elapsed();
    while clock.elapsed().saturating_sub(started) < bounds.settle() {
        clock.sleep(RestartBounds::POLL_INTERVAL);
        let current = processes
            .daemon_parent()
            .map_err(RestartFailure::ParentProbe)?;
        if current != Some(parent) {
            return Err(RestartFailure::UnstableParent(parent));
        }
    }
    Ok(Restarted {
        parent,
        settled_for: bounds.settle(),
    })
}

fn wait_for_parent(
    processes: &mut impl ProcessTable,
    clock: &mut impl RestartClock,
    deadline: Duration,
    previous: Option<ParentPid>,
) -> Result<ParentPid, RestartFailure> {
    let started = clock.elapsed();
    loop {
        let current = processes
            .daemon_parent()
            .map_err(RestartFailure::ParentProbe)?;
        if let Some(parent) = current.filter(|pid| Some(*pid) != previous) {
            return Ok(parent);
        }
        if clock.elapsed().saturating_sub(started) >= deadline {
            break;
        }
        clock.sleep(RestartBounds::POLL_INTERVAL);
    }
    if let Some(parent) = previous
        && processes
            .daemon_parent()
            .map_err(RestartFailure::ParentProbe)?
            == Some(parent)
    {
        return Err(RestartFailure::UnchangedParent(parent));
    }
    Err(RestartFailure::Deadline)
}

#[cfg(test)]
mod tests;
