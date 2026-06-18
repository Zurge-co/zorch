use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use zorch_shared::AppError;

pub(crate) fn init_tracing_subscriber(filter: &str) -> Result<(), AppError> {
    let fmt_layer = fmt::layer();
    let env_filter = EnvFilter::try_new(filter).unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .try_init()
        .ok();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_tracing_subscriber_default() {
        let result = init_tracing_subscriber("info");
        assert!(result.is_ok());
    }
}
