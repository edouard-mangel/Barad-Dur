use std::time::Duration;

pub const TIMEOUT_SECS: u64 = 15;

pub fn http() -> reqwest::blocking::Client {
    http_with_timeout(Duration::from_secs(TIMEOUT_SECS))
}

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
        // A server that accepts TCP connections but never sends any bytes.
        // Without a timeout on the client, GET would block forever.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            // Accept the connection, hold it open, but never write a response.
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
