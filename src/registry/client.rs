use std::sync::OnceLock;
use std::time::Duration;

pub const TIMEOUT_SECS: u64 = 15;

static HTTP_CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();

/// Shared HTTP client with a 15-second timeout. Initialised once and reused
/// across all registry calls so that connections can be pooled between requests.
pub fn http() -> &'static reqwest::blocking::Client {
    HTTP_CLIENT.get_or_init(|| http_with_timeout(Duration::from_secs(TIMEOUT_SECS)))
}

/// Build a fresh client with a custom timeout. Used in tests only.
pub fn http_with_timeout(timeout: Duration) -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .expect("failed to build HTTP client")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::time::Instant;

    #[test]
    fn client_times_out_on_unresponsive_server() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let _ = listener.accept();
        });

        let start = Instant::now();
        let result = http_with_timeout(Duration::from_millis(200))
            .get(format!("http://{}/", addr))
            .send();

        assert!(result.is_err(), "expected timeout error, got success");
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "client took too long to time out: {:?}",
            start.elapsed()
        );
    }
}
