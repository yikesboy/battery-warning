use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum PolicyError {
    #[error("thresholds must be finite and satisfy 0 <= critical < low < rearm <= 100")]
    InvalidThresholds,
    #[error("battery percentage must be finite and between 0 and 100, got {0}")]
    InvalidPercentage(f64),
}

type Result<T> = std::result::Result<T, PolicyError>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Thresholds {
    pub critical: f64,
    pub low: f64,
    pub rearm: f64,
}

impl Thresholds {
    pub fn validate(&self) -> Result<()> {
        if !(self.critical.is_finite()
            && self.low.is_finite()
            && self.rearm.is_finite()
            && 0.0 <= self.critical
            && self.critical < self.low
            && self.low < self.rearm
            && self.rearm <= 100.0)
        {
            return Err(PolicyError::InvalidThresholds);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Snapshot {
    pub on_battery: bool,
    pub present: bool,
    pub percentage: f64,
    pub state: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Low,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    None,
    Close,
    Show(Level),
}

#[derive(Debug)]
pub struct Policy {
    thresholds: Thresholds,
    issued: Option<Level>,
}

impl Policy {
    pub fn new(thresholds: Thresholds) -> Result<Self> {
        thresholds.validate()?;
        Ok(Self {
            thresholds,
            issued: None,
        })
    }

    pub fn evaluate(&mut self, snapshot: Snapshot) -> Result<Effect> {
        if !snapshot.on_battery || !snapshot.present {
            self.issued = None;
            return Ok(Effect::Close);
        }
        if !snapshot.percentage.is_finite() || !(0.0..=100.0).contains(&snapshot.percentage) {
            return Err(PolicyError::InvalidPercentage(snapshot.percentage));
        }
        if snapshot.percentage > self.thresholds.rearm {
            self.issued = None;
            return Ok(Effect::Close);
        }
        if !matches!(snapshot.state, 2 | 6) {
            return Ok(Effect::None);
        }

        let level = if snapshot.percentage <= self.thresholds.critical {
            Level::Critical
        } else if snapshot.percentage <= self.thresholds.low {
            Level::Low
        } else {
            return Ok(Effect::None);
        };
        Ok(if self.issued.is_none_or(|issued| level > issued) {
            Effect::Show(level)
        } else {
            Effect::None
        })
    }

    /// Call only after a notification has successfully spawned, not on dismissal.
    pub fn issued(&mut self, level: Level) {
        self.issued = Some(self.issued.map_or(level, |issued| issued.max(level)));
    }
}
