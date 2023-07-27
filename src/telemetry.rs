use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub fn init() {
    init_with_filter(std::env::var("RUST_LOG").as_deref().unwrap_or("info"))
}

pub fn init_with_filter(filter: &str) {
    let fmt_layer = fmt::layer().with_target(false).compact();
    let filter_layer = EnvFilter::try_new(filter)
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
}
