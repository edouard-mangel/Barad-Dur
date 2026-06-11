use crate::scorer::{score_band, ScoreBand};
use colored::Colorize;

pub(crate) fn colorize_by_score(s: &str, score: u32) -> colored::ColoredString {
    match score_band(score) {
        ScoreBand::Good => s.green(),
        ScoreBand::Warn => s.yellow(),
        ScoreBand::Danger => s.red(),
    }
}

pub(crate) fn format_score_bar(score: u32, width: usize) -> String {
    let filled = (score as usize * width) / 100;
    let empty = width - filled;
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));
    colorize_by_score(&bar, score).to_string()
}

pub(crate) fn format_score_number(score: u32) -> String {
    colorize_by_score(&format!("{}/100", score), score)
        .bold()
        .to_string()
}

pub(crate) fn format_score_dot(score: u32) -> String {
    colorize_by_score("●", score).to_string()
}

pub(crate) fn direction_arrow(direction: &crate::trend::VelocityDirection) -> &'static str {
    use crate::trend::VelocityDirection;
    match direction {
        VelocityDirection::Improving => "↑",
        VelocityDirection::Declining => "↓",
        VelocityDirection::Stable => "→",
    }
}

pub(crate) fn direction_word(direction: &crate::trend::VelocityDirection) -> &'static str {
    use crate::trend::VelocityDirection;
    match direction {
        VelocityDirection::Improving => "improving",
        VelocityDirection::Declining => "declining",
        VelocityDirection::Stable => "stable",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scorer::{SCORE_GOOD_MIN, SCORE_WARN_MIN};
    use colored::Color;

    #[test]
    fn cli_colors_follow_the_shared_band_thresholds() {
        assert_eq!(
            colorize_by_score("x", SCORE_GOOD_MIN).fgcolor,
            Some(Color::Green)
        );
        assert_eq!(
            colorize_by_score("x", SCORE_GOOD_MIN - 1).fgcolor,
            Some(Color::Yellow)
        );
        assert_eq!(
            colorize_by_score("x", SCORE_WARN_MIN).fgcolor,
            Some(Color::Yellow)
        );
        assert_eq!(
            colorize_by_score("x", SCORE_WARN_MIN - 1).fgcolor,
            Some(Color::Red)
        );
    }
}
