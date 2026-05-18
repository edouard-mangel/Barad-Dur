use std::time::Duration;

const TIMEOUT_SECS: u64 = 15;

pub fn http() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build()
        .expect("failed to build HTTP client")
}
