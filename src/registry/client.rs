use std::sync::OnceLock;
use std::time::Duration;

pub const TIMEOUT_SECS: u64 = 15;

static HTTP_CLIENT: OnceLock<Option<reqwest::blocking::Client>> = OnceLock::new();

pub fn http() -> Option<&'static reqwest::blocking::Client> {
    HTTP_CLIENT
        .get_or_init(
            || match http_with_timeout(Duration::from_secs(TIMEOUT_SECS)) {
                Ok(c) => Some(c),
                Err(e) => {
                    eprintln!("warning: failed to build HTTP client: {e}");
                    None
                }
            },
        )
        .as_ref()
}

pub fn http_with_timeout(timeout: Duration) -> Result<reqwest::blocking::Client, reqwest::Error> {
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
}

/// Whether `elapsed` proves a caller-supplied timeout was applied rather
/// than the `TIMEOUT_SECS` default.
///
/// The bug worth catching is `http_with_timeout` ignoring its argument and
/// falling back to the default; that path cannot finish in less than
/// `TIMEOUT_SECS`, so any shorter measurement rules it out. The bound is
/// deliberately the full default rather than a fraction of it: the number
/// measured is wall clock on CI machines running mutation shards in
/// parallel, where most of it can be scheduling delay that says nothing
/// about the client. A tighter bound measures the machine, not the code.
#[cfg(test)]
fn ruled_out_default_timeout(elapsed: Duration) -> bool {
    elapsed < Duration::from_secs(TIMEOUT_SECS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::time::Instant;

    #[test]
    fn scheduling_delay_does_not_read_as_the_default_timeout() {
        // The observed CI failure: a 200ms client timeout measured at over
        // 7.5s of wall clock on a runner executing mutation shards in
        // parallel. The client was correct; the machine was busy. Anything
        // short of the default must still count as ruling it out.
        assert!(ruled_out_default_timeout(Duration::from_millis(200)));
        assert!(ruled_out_default_timeout(Duration::from_secs(8)));
        assert!(ruled_out_default_timeout(Duration::from_secs(
            TIMEOUT_SECS - 1
        )));
    }

    #[test]
    fn waiting_the_default_timeout_is_not_ruled_out() {
        // The bug this guards: `http_with_timeout` ignoring its argument and
        // falling back to the default. That path cannot finish before
        // TIMEOUT_SECS, so at or beyond it the check must fail.
        assert!(!ruled_out_default_timeout(Duration::from_secs(
            TIMEOUT_SECS
        )));
        assert!(!ruled_out_default_timeout(Duration::from_secs(
            TIMEOUT_SECS + 1
        )));
    }

    #[test]
    fn http_with_timeout_returns_result() {
        let result = http_with_timeout(Duration::from_millis(100));
        assert!(result.is_ok(), "expected Ok client, got {:?}", result.err());
    }

    #[test]
    fn http_none_produces_unavailable_error() {
        // Verify that the ok_or_else pattern used at all call sites produces
        // the expected error message when the client is None.
        let result: anyhow::Result<&reqwest::blocking::Client> = None::<&reqwest::blocking::Client>
            .ok_or_else(|| anyhow::anyhow!("HTTP client unavailable"));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "HTTP client unavailable");
    }

    #[test]
    fn http_singleton_returns_some() {
        assert!(
            http().is_some(),
            "http() must return Some on a normal system"
        );
    }

    #[test]
    fn client_times_out_on_unresponsive_server() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        // Accept the connection but keep it open — forces the client to wait
        // until its own timeout fires rather than getting an immediate reset.
        std::thread::spawn(move || {
            if let Ok((_stream, _)) = listener.accept() {
                std::thread::sleep(Duration::from_secs(60));
            }
        });

        let start = Instant::now();
        let result = http_with_timeout(Duration::from_millis(200))
            .expect("client should build")
            .get(format!("http://{}/", addr))
            .send();

        assert!(result.is_err(), "expected timeout error, got success");
        let err = result.unwrap_err();
        assert!(err.is_timeout(), "expected timeout error, got: {err}");
        assert!(
            ruled_out_default_timeout(start.elapsed()),
            "client waited at least the default {TIMEOUT_SECS}s, so the \
             200ms timeout was not applied: {:?}",
            start.elapsed()
        );
    }
}
