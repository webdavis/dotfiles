use crate::ValueError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Brightness(u8);
impl Brightness {
    pub fn new(value: u64) -> Self {
        Self(value.clamp(1, 100) as u8)
    }
    pub fn percent(self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReportedBrightness(f64);
impl ReportedBrightness {
    pub fn new(value: f64) -> Result<Self, ValueError> {
        if value.is_finite() && (0.0..=100.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ValueError("invalid reported brightness"))
        }
    }
    pub fn percent(self) -> f64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absolute_brightness_clamps_0_1_2_99_100_101() {
        for (input, expected) in [
            (0, 1),
            (1, 1),
            (2, 2),
            (99, 99),
            (100, 100),
            (101, 100),
            (u64::MAX, 100),
        ] {
            assert_eq!(Brightness::new(input).percent(), expected);
        }
    }
    #[test]
    fn reported_brightness_preserves_zero_and_fraction() {
        for n in [0.0, 0.5, 1.0, 42.75, 100.0] {
            assert_eq!(ReportedBrightness::new(n).unwrap().percent(), n);
        }
    }
    #[test]
    fn invalid_reported_brightness_is_rejected() {
        for n in [-0.01, 100.01, f64::NAN, f64::INFINITY] {
            assert!(ReportedBrightness::new(n).is_err());
        }
    }
}
