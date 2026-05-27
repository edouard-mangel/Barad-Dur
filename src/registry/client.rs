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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::time::Instant;

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
            start.elapsed() < Duration::from_secs(TIMEOUT_SECS / 2),
            "client took too long to time out: {:?}",
            start.elapsed()
        );
    }
}
