use crate::domain::errors::DomainError;
use std::sync::atomic::{AtomicI64, AtomicU8, AtomicU32, Ordering};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum State {
    Closed,
    Open,
    HalfOpen,
}

impl From<u8> for State {
    fn from(v: u8) -> Self {
        match v {
            0 => State::Closed,
            1 => State::Open,
            _ => State::HalfOpen,
        }
    }
}

pub struct CircuitBreaker {
    threshold: u32,
    timeout: i64,
    failures: AtomicU32,
    last_failure_time: AtomicI64,
    state: AtomicU8,
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
            state: AtomicU8::new(0),
        }
    }

    pub fn check_state(&self) -> State {
        let old_state_raw = self.state.load(Ordering::SeqCst);
        let old_state = State::from(old_state_raw); // Helper pe
        let last_error = self.last_failure_time.load(Ordering::SeqCst);
        let now = chrono::Utc::now().timestamp();

        let fails = self.failures.load(Ordering::SeqCst);

        let final_state = match old_state {
            _ if fails < self.threshold => State::Closed,

            _ if now - last_error >= self.timeout => State::HalfOpen,

            _ => State::Open,
        };

        if old_state != final_state {
            tracing::warn!(
                service = "external_api",
                old_state = ?old_state,
                new_state = ?final_state,
                "Circuit breaker state transition"
            );

            self.state.store(final_state as u8, Ordering::SeqCst);

            if final_state == State::Closed {
                self.failures.store(0, Ordering::SeqCst);
            }
        }

        final_state
    }

    pub async fn call<F, T>(&self, f: F) -> Result<T, DomainError>
    where
        F: std::future::Future<Output = Result<T, DomainError>>,
    {
        match self.check_state() {
            State::Open => Err(DomainError::DependencyUnavailable {
                service: "todo".into(),
            }),
            State::Closed | State::HalfOpen => match f.await {
                Ok(data) => {
                    self.reset();
                    Ok(data)
                }
                Err(err) => match err {
                    DomainError::DependencyUnavailable { service: _ } => {
                        self.record_failure();
                        Err(err)
                    }
                    _ => Err(err),
                },
            },
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
