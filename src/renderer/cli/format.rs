use colored::Colorize;

pub(crate) fn colorize_by_score(s: &str, score: u32) -> colored::ColoredString {
    if score >= 71 {
        s.green()
    } else if score >= 41 {
        s.yellow()
    } else {
        s.red()
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
