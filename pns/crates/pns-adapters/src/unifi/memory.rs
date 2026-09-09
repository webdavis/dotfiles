pub struct HomeStaleness;

impl pns_application::StalenessMemory for HomeStaleness {
    fn remembered(&self) -> Option<String> {
        crate::remembered_staleness()
    }

    fn remember(&self, episode: Option<&str>) {
        crate::remember_staleness(episode);
    }
}
