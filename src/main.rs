use anyhow::Context;
use coodo_be::{settings::get_settings, startup::Application, telemetry};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    telemetry::init();

    let settings = get_settings().context("Failed to parse app settings")?;
    let app = Application::build(settings)
        .await
        .context("Failed to initialize app")?;

    app.run_until_stopped().await?;

    Ok(())
}
