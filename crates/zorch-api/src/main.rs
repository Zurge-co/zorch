#[tokio::main]
async fn main() {
    let cfg = match zorch_shared::AppConfig::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = zorch_telemetry::init_telemetry(&cfg) {
        eprintln!("Failed to initialize telemetry: {}", e);
        std::process::exit(1);
    }

    if let Err(e) = zorch_api::run(cfg).await {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }
}
