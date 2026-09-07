use super::*;

pub struct Guard(Fixture);
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.event(Event::Release);
        self.0.0.borrow_mut().held = false;
    }
}

impl RunState for Fixture {
    type Guard = Guard;
    fn acquire(&self) -> Result<Guard, LockFailure> {
        let mut data = self.0.borrow_mut();
        assert!(!data.held);
        data.events.push(Event::Acquire);
        if let Some(failure) = data.lock_failure.take() {
            return Err(failure);
        }
        data.held = true;
        Ok(Guard(self.clone()))
    }
    fn prune_removed_lanes(&self, declared: &[&str]) {
        self.event(Event::Prune(
            declared.iter().map(|name| name.to_string()).collect(),
        ));
    }
    fn marker(&self) -> MarkerSnapshot {
        self.event(Event::MarkerRead);
        MarkerSnapshot {
            value: Marker::Recorded {
                epoch: 7,
                iso: "previous".into(),
            },
            location: "/fixture/marker".into(),
        }
    }
    fn write_marker(&self, epoch: i64) -> Result<(), StateWriteFailure> {
        self.event(Event::MarkerWrite(epoch));
        Ok(())
    }
    fn streak(&self, lane: &str, kind: StreakKind) -> StreakSnapshot {
        self.event(Event::StreakRead(kind, lane.into()));
        let data = self.0.borrow();
        let value = if kind == StreakKind::Pending && data.unreadable_pending {
            Streak::Unreadable("fixture damaged count".into())
        } else {
            let counts = match kind {
                StreakKind::NonSuccess => &data.streaks,
                StreakKind::Pending => &data.pending,
            };
            counts
                .get(lane)
                .copied()
                .map_or(Streak::Absent, Streak::Value)
        };
        StreakSnapshot {
            value,
            location: format!("/fixture/{lane}/{kind:?}"),
        }
    }
    fn write_streak(
        &self,
        lane: &str,
        kind: StreakKind,
        value: u32,
    ) -> Result<(), StateWriteFailure> {
        self.event(Event::StreakWrite(kind, lane.into(), value));
        let mut data = self.0.borrow_mut();
        if kind == StreakKind::Pending && data.fail_pending_write {
            return Err(StateWriteFailure {
                location: format!("/fixture/{lane}/pending"),
                cause: "fixture write refused".into(),
            });
        }
        match kind {
            StreakKind::NonSuccess => &mut data.streaks,
            StreakKind::Pending => &mut data.pending,
        }
        .insert(lane.into(), value);
        Ok(())
    }
}
