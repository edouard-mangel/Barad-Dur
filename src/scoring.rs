use serde::Serialize;

/// Minimum score (inclusive) for the "good" band.
pub const SCORE_GOOD_MIN: u32 = 71;
/// Minimum score (inclusive) for the "warn" band; below is "danger".
pub const SCORE_WARN_MIN: u32 = 41;

/// Qualitative band for a 0–100 score. Single source of truth for every
/// renderer (CLI colors, HTML report, dashboard) — renderers must not
/// re-derive thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreBand {
    Good,
    Warn,
    Danger,
}

pub fn score_band(score: u32) -> ScoreBand {
    match score {
        s if s >= SCORE_GOOD_MIN => ScoreBand::Good,
        s if s >= SCORE_WARN_MIN => ScoreBand::Warn,
        _ => ScoreBand::Danger,
    }
}

/// Band thresholds serialized into every report so JS/TS consumers read the
/// verdict boundaries instead of hardcoding them.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct ScoreThresholds {
    pub good_min: u32,
    pub warn_min: u32,
}

impl Default for ScoreThresholds {
    fn default() -> Self {
        Self {
            good_min: SCORE_GOOD_MIN,
            warn_min: SCORE_WARN_MIN,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn band_boundaries_match_documented_thresholds() {
        assert_eq!(score_band(100), ScoreBand::Good);
        assert_eq!(score_band(SCORE_GOOD_MIN), ScoreBand::Good);
        assert_eq!(score_band(SCORE_GOOD_MIN - 1), ScoreBand::Warn);
        assert_eq!(score_band(SCORE_WARN_MIN), ScoreBand::Warn);
        assert_eq!(score_band(SCORE_WARN_MIN - 1), ScoreBand::Danger);
        assert_eq!(score_band(0), ScoreBand::Danger);
    }
    #[test]
    fn default_thresholds_serialize_for_consumers() {
        assert_eq!(
            serde_json::to_string(&ScoreThresholds::default()).unwrap(),
            r#"{"good_min":71,"warn_min":41}"#
        );
    }
}
