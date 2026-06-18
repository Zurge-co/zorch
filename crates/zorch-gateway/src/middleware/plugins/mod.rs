pub mod prompt_injector;
pub mod request_blocker;
pub mod sensitive_marker;
pub mod token_reducer;

use super::types::MiddlewarePlugin;
use std::sync::Arc;

pub fn built_in_plugins() -> Vec<Arc<dyn MiddlewarePlugin>> {
    vec![
        Arc::new(token_reducer::TokenReducerPlugin),
        Arc::new(sensitive_marker::SensitiveMarkerPlugin),
        Arc::new(request_blocker::RequestBlockerPlugin),
        Arc::new(prompt_injector::PromptInjectorPlugin),
    ]
}
