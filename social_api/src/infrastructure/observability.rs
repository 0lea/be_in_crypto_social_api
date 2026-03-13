use tracing_subscriber::{
    EnvFilter,
    fmt::{self, time::UtcTime},
    prelude::*,
};

pub fn init_observability() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,social_api=debug"));

    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_timer(UtcTime::rfc_3339())
        .pretty();

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .init();
}
