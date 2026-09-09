use crate::{ConfigError, lanes::LaneAdapter};

type ParseLane = fn(&str, toml::Table) -> Result<Box<dyn LaneAdapter>, ConfigError>;

#[derive(Clone, Copy)]
pub struct LaneRegistration {
    name: &'static str,
    parse: ParseLane,
    keys: fn() -> &'static [&'static str],
}

impl LaneRegistration {
    pub const fn new<T: LaneAdapter + 'static>(name: &'static str) -> Self {
        Self {
            name,
            parse: parse_adapter::<T>,
            keys: T::keys,
        }
    }

    pub fn type_name(&self) -> &'static str {
        self.name
    }

    pub fn keys(&self) -> &'static [&'static str] {
        (self.keys)()
    }

    pub(crate) fn parse(
        &self,
        label: &str,
        fields: toml::Table,
    ) -> Result<Box<dyn LaneAdapter>, ConfigError> {
        (self.parse)(label, fields)
    }
}

fn parse_adapter<T: LaneAdapter + 'static>(
    label: &str,
    fields: toml::Table,
) -> Result<Box<dyn LaneAdapter>, ConfigError> {
    Ok(Box::new(T::parse(label, fields)?))
}
