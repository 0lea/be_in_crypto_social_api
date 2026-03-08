use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub fn init_observability() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,social_api=debug"));

    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .pretty();

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .init();
}
