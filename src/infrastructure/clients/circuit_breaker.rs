use crate::domain::errors::DomainError;
use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};
use std::time::Duration;

#[derive(Debug, PartialEq)]
pub enum State {
    Closed,
    Open,
    HalfOpen,
}

pub struct CircuitBreaker {
    threshold: u32,
    timeout: i64,
    failures: AtomicU32,
    last_failure_time: AtomicI64,
}

impl CircuitBreaker {
    pub fn from_env() -> Self {
        let threshold = std::env::var("CB_THRESHOLD")
            .unwrap_or_else(|_| "5".into())
            .parse()
            .unwrap();
        let timeout = std::env::var("CB_TIMEOUT_SECS")
            .unwrap_or_else(|_| "30".into())
            .parse()
            .unwrap();

        Self {
            threshold,
            timeout,
            failures: AtomicU32::new(0),
            last_failure_time: AtomicI64::new(0),
        }
    }

    pub fn check_state(&self) -> State {
        let fails = self.failures.load(Ordering::SeqCst);

        if fails < self.threshold {
            return State::Closed;
        }

        let last_error = self.last_failure_time.load(Ordering::SeqCst);
        let now = chrono::Utc::now().timestamp();

        if now - last_error >= self.timeout {
            State::HalfOpen
        } else {
            State::Open
        }
    }

    pub async fn call<F, T, E>(&self, f: F) -> Result<T, DomainError>
    where
        F: std::future::Future<Output = Result<T, E>>,
    {
        match self.check_state() {
            State::Open => {
                // L'interruttore è saltato. Non carichiamo il sistema.
                return Err(DomainError::ExternalServiceUnavailable);
            }
            State::Closed | State::HalfOpen => {
                // In Closed passano tutte, in HalfOpen ne passa una di "test"
                match f.await {
                    Ok(data) => {
                        self.reset();
                        Ok(data)
                    }
                    Err(_) => {
                        self.record_failure();
                        Err(DomainError::ExternalServiceUnavailable)
                    }
                }
            }
        }
    }

    fn reset(&self) {
        self.failures.store(0, Ordering::SeqCst);
    }

    fn record_failure(&self) {
        self.failures.fetch_add(1, Ordering::SeqCst);
        let now = chrono::Utc::now().timestamp();
        self.last_failure_time.store(now, Ordering::SeqCst);
    }
}
