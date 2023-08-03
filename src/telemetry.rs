use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub fn init_with_filter(filter: &str) {
    let fmt_layer = fmt::layer().with_target(false).compact();
    let filter_layer = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter));

    //LogTracer::init_with_filter(tracing::log::LevelFilter::Info).expect("Failed to set logger");

    let subscriber = tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer);

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set global subscriber");
}
