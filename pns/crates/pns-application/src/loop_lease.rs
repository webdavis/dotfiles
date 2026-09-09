use crate::LoopLeases;

pub struct AcquireLoopLease<'a, L> {
    pub leases: &'a L,
}
impl<L: LoopLeases> AcquireLoopLease<'_, L> {
    pub fn begin(
        &self,
        pane: &str,
        now: Option<u64>,
        register: impl FnOnce(u64),
    ) -> Result<(), String> {
        // NO CLOCK IS NO LEASE, never a lease at epoch zero: the timeout is
        // measured against this number, and a zero would be expired the
        // moment it was written.
        let now = now
            .filter(|_| pns_domain::safety::pane_file_is_safe(pane))
            .ok_or("pns: loop: the clock cannot be read; the lease was not taken")?;
        self.leases
            .begin(pane, now)
            .map_err(|error| format!("pns: loop: the lease could not be written: {error}"))?;
        // AND THE TICK IS REGISTERED FOR THE WHOLE LEASE, because nothing
        // else will register it in time. The tick's own lease is refreshed
        // by EVENT traffic, so a lease taken by hand in a pane that then
        // goes quiet, which is exactly the overnight run this verb exists
        // for, would be read by a tick that expired minutes into it.
        register(now);
        Ok(())
    }
    pub fn end(&self, pane: &str) -> Result<(), String> {
        self.leases.end(pane).map_err(|error| {
            format!(
                "pns: loop: the lease could not be given back ({error}); the loop lamp \
             keeps breathing until it times out"
            )
        })
    }
}

#[cfg(test)]
mod tests;
