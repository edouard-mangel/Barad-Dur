use anyhow::{bail, Result};

/// The three modes the harness understands. Parsed once, up front — the
/// enum is closed, so a mode-dispatch match against it is exhaustive at the
/// type level: there is no wildcard arm left for an unrecognized mode to
/// silently fall into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Run,
    Accept,
    Audit,
}

/// Parse the raw mode argument. Pure and separate from the driver's `main`
/// so an unrecognized mode can be tested without invoking the binary.
pub fn parse_mode(raw: &str) -> Result<Mode> {
    match raw {
        "run" => Ok(Mode::Run),
        "accept" => Ok(Mode::Accept),
        "audit" => Ok(Mode::Audit),
        other => bail!("unrecognized mode {other:?} — expected one of: run, accept, audit"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mode_accepts_the_three_known_modes() {
        assert_eq!(parse_mode("run").unwrap(), Mode::Run);
        assert_eq!(parse_mode("accept").unwrap(), Mode::Accept);
        assert_eq!(parse_mode("audit").unwrap(), Mode::Audit);
    }

    #[test]
    fn parse_mode_rejects_an_unrecognized_mode() {
        let err = parse_mode("acept").expect_err("typo must not silently pass through");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("acept"),
            "error must name the offending argument, got: {msg}"
        );
        for valid in ["run", "accept", "audit"] {
            assert!(
                msg.contains(valid),
                "error must list valid mode {valid}, got: {msg}"
            );
        }
    }
}
